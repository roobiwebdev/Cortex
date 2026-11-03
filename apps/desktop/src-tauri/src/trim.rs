//! SOURCE OF TRUTH KEYWORDS: Library, list_recordings, rename_recording, recording_thumbnails, recording_waveform, trim_recording, cancel_trim, cortex-media, recordings://changed, trim://progress
//! WHAT:  The recordings library and Quick trim in the shell: the scanned list of recordings in
//!        the output folder, rename, timeline thumbnails and waveform (cached), trim export with
//!        progress and cancel, and the `cortex-media://` scheme the Trim window plays files from.
//! WHY:   The webview never sees a path it can choose: every command takes a recording id that
//!        must be in the latest scan of the output folder, and the media scheme serves only
//!        those files, only to the Trim window (engineering plan §6, NEW-TR-02). Thumbnails come
//!        back as small JPEG data URLs, so no file access is granted for them at all.
//! WHERE: `lib.rs` (setup, the media scheme, rescans after each recording); `ipc.rs` registers
//!        the commands and events; `surfaces/trim.tsx` and `lib/engine.ts` call them.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rec_core::api::{EngineError, RecorderEngine};
use rec_core::library::{
    self, MAX_RECORDINGS, MAX_THUMBNAILS, MAX_WAVEFORM_BUCKETS, TrimOutput, TrimProgress,
    TrimRequest,
};
use rec_core::state::Recording;
use rec_engine::trim::{self as media, ExportOptions, MediaInfo};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{AppHandle, Manager, Runtime, State, UriSchemeContext, UriSchemeResponder};
use tauri_specta::Event;

use crate::shell::{Emit, Outbound, Shell};

/// The recordings in the output folder, newest first. Sent after every rescan that changed it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RecordingsChangedEvent(pub Vec<Recording>);
impl Event for RecordingsChangedEvent {
    const NAME: &'static str = "recordings://changed";
}

/// Trim export progress, about ten times a second.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TrimProgressEvent(pub TrimProgress);
impl Event for TrimProgressEvent {
    const NAME: &'static str = "trim://progress";
}

/// The only window that may load recordings through `cortex-media://`.
const MEDIA_WINDOWS: [&str; 1] = ["trim"];
/// Thumbnail width in px (the filmstrip tiles are about 92 CSS px wide on a 2x display).
const THUMB_WIDTH: f64 = 192.0;
/// A media response carries at most this many bytes; the player asks for the rest.
const MAX_CHUNK: u64 = 4 << 20;
/// While recording, a file written to this recently is the one being recorded.
const IN_PROGRESS: Duration = Duration::from_secs(4);

#[derive(Clone)]
struct Entry {
    rec: Recording,
    path: PathBuf,
    /// Size, modification time and inode: what the caches are keyed by (survives a rename).
    key: String,
    info: MediaInfo,
}

/// The library: the latest scan, running exports and the cache folder.
pub struct Library {
    cache_dir: PathBuf,
    emit: Emit,
    entries: Mutex<Vec<Entry>>,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// One scan at a time; a second caller waits and then reuses the fresh result.
    scanning: Mutex<()>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn failed(e: impl std::fmt::Display) -> EngineError {
    EngineError::failed(e.to_string())
}

fn not_found() -> EngineError {
    EngineError::failed("That recording is no longer in the recordings folder")
}

fn millis(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

impl Library {
    pub fn new(cache_dir: PathBuf, emit: Emit) -> Library {
        Library {
            cache_dir,
            emit,
            entries: Mutex::new(vec![]),
            jobs: Mutex::new(HashMap::new()),
            scanning: Mutex::new(()),
        }
    }

    /// Rescan `dir` and return the list. Files already seen with the same size, time and
    /// inode are not opened again. Unreadable files are left out. `skip_recent`: leave out
    /// files written in the last few seconds (the recording in progress).
    pub fn scan(&self, dir: &Path, skip_recent: bool) -> Vec<Recording> {
        let _one = lock(&self.scanning);
        let known: HashMap<String, Entry> = lock(&self.entries)
            .iter()
            .map(|e| (e.key.clone(), e.clone()))
            .collect();
        let mut files: Vec<(PathBuf, std::fs::Metadata, SystemTime)> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|d| d.path())
            .filter(|p| library::is_recording_file(p))
            .filter_map(|p| {
                let m = std::fs::metadata(&p).ok().filter(|m| m.is_file())?;
                let t = m.created().or_else(|_| m.modified()).ok()?;
                Some((p, m, t))
            })
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.2));
        files.truncate(MAX_RECORDINGS);
        let now = SystemTime::now();
        let mut out: Vec<Entry> = Vec::with_capacity(files.len());
        for (path, meta, created) in files {
            let modified = meta.modified().unwrap_or(created);
            if skip_recent && now.duration_since(modified).is_ok_and(|d| d < IN_PROGRESS) {
                continue;
            }
            let key = format!("{}-{}-{}", meta.ino(), meta.len(), millis(modified) as u64);
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let info = match known.get(&key) {
                Some(e) => e.info.clone(),
                None => match media::inspect(&path) {
                    Ok(i) if i.duration_secs > 0.0 => i,
                    _ => continue,
                },
            };
            let rec = Recording {
                id: stem.clone(),
                name: stem,
                path: path.display().to_string(),
                duration_secs: info.duration_secs,
                bytes: meta.len(),
                created_at: millis(created),
                width: info.width,
                height: info.height,
                has_camera_track: info.video_tracks > 1,
                fps: info.fps.round().max(0.0) as u32,
                audio_tracks: info.audio.len() as u32,
            };
            out.push(Entry {
                rec,
                path,
                key,
                info,
            });
        }
        // Ids are file stems, like the engine's; a stem shared by two files uses the file name.
        let mut count: HashMap<String, u32> = HashMap::new();
        for e in &out {
            *count.entry(e.rec.id.clone()).or_default() += 1;
        }
        for e in &mut out {
            if count.get(&e.rec.id).is_some_and(|n| *n > 1)
                && let Some(n) = e.path.file_name()
            {
                e.rec.id = n.to_string_lossy().into_owned();
            }
        }
        let list = out.iter().map(|e| e.rec.clone()).collect();
        *lock(&self.entries) = out;
        list
    }

    fn find(&self, id: &str) -> Result<Entry, EngineError> {
        lock(&self.entries)
            .iter()
            .find(|e| e.rec.id == id)
            .cloned()
            .ok_or_else(not_found)
    }

    /// The file for `id`, if it is in the latest scan and still a file.
    pub fn media_path(&self, id: &str) -> Option<PathBuf> {
        self.find(id).ok().map(|e| e.path).filter(|p| p.is_file())
    }

    fn cache_file(&self, kind: &str, name: String) -> PathBuf {
        self.cache_dir.join(kind).join(name)
    }
}

/// Scan, then tell every window the list.
fn rescan<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Recording>, EngineError> {
    let (Some(lib), Some(shell)) = (
        app.try_state::<Arc<Library>>(),
        app.try_state::<Arc<Shell>>(),
    ) else {
        return Ok(vec![]);
    };
    let capturing = shell.engine().state().status.is_capturing();
    let list = lib.scan(&shell.output_dir(), capturing);
    let _ = RecordingsChangedEvent(list.clone()).emit(app);
    Ok(list)
}

/// Rescan in the background (at launch and after each recording). Never blocks the caller.
pub fn rescan_later<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = rescan(&app) {
            eprintln!("cortex: recordings scan failed: {e}");
        }
    });
}

/// The `emit` hook: a finished recording means the folder changed.
pub fn observer<R: Runtime>(app: AppHandle<R>) -> impl Fn(&Outbound) + Send + Sync + 'static {
    move |o| {
        if matches!(o, Outbound::RecordingFinished(_)) {
            rescan_later(&app);
        }
    }
}

type Lib<'a> = State<'a, Arc<Library>>;

async fn off_thread<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, EngineError> + Send + 'static,
) -> Result<T, EngineError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| failed(format!("The recorder stopped responding: {e}")))?
}

/// Every recording in the output folder (.mov/.mp4), newest first, at most 200.
#[tauri::command]
#[specta::specta]
pub async fn list_recordings(app: AppHandle) -> Result<Vec<Recording>, EngineError> {
    off_thread(move || rescan(&app)).await
}

/// Rename a recording's file (same folder, same extension). Returns it under its new name.
#[tauri::command]
#[specta::specta]
pub async fn rename_recording(
    app: AppHandle,
    lib: Lib<'_>,
    id: String,
    name: String,
) -> Result<Recording, EngineError> {
    let lib = lib.inner().clone();
    off_thread(move || {
        let name = library::validate_name(&name)?;
        let e = lib.find(&id)?;
        if lock(&lib.jobs).contains_key(&id) {
            return Err(failed("Wait for the export to finish before renaming"));
        }
        let ext = e.path.extension().map(|x| x.to_string_lossy().into_owned());
        let file = match ext {
            Some(x) => format!("{name}.{x}"),
            None => name.clone(),
        };
        let dir = e.path.parent().ok_or_else(not_found)?;
        let to = dir.join(&file);
        if to != e.path {
            // Case-only renames are the same file on APFS; anything else must be free.
            let same = file.eq_ignore_ascii_case(
                &e.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            if to.exists() && !same {
                return Err(failed("A recording with that name already exists"));
            }
            std::fs::rename(&e.path, &to).map_err(|err| failed(format!("Rename failed: {err}")))?;
        }
        let list = rescan(&app)?;
        let to = to.display().to_string();
        list.into_iter()
            .find(|r| r.path == to)
            .ok_or_else(not_found)
    })
    .await
}

/// `count` JPEG thumbnails spread over the recording, as `data:` URLs (cached on disk).
#[tauri::command]
#[specta::specta]
pub async fn recording_thumbnails(
    lib: Lib<'_>,
    id: String,
    count: u32,
) -> Result<Vec<String>, EngineError> {
    let lib = lib.inner().clone();
    off_thread(move || {
        let count = count.clamp(1, MAX_THUMBNAILS);
        let e = lib.find(&id)?;
        let dir = lib.cache_file("thumbs", format!("{}-{count}", e.key));
        let cached: Option<Vec<Vec<u8>>> = (0..count)
            .map(|i| std::fs::read(dir.join(format!("{i}.jpg"))).ok())
            .collect();
        let jpegs = match cached {
            Some(j) => j,
            None => {
                let j = media::thumbnails(&e.path, count, THUMB_WIDTH).map_err(failed)?;
                if std::fs::create_dir_all(&dir).is_ok() {
                    for (i, bytes) in j.iter().enumerate() {
                        let _ = std::fs::write(dir.join(format!("{i}.jpg")), bytes);
                    }
                }
                j
            }
        };
        Ok(jpegs
            .iter()
            .map(|j| format!("data:image/jpeg;base64,{}", base64(j)))
            .collect())
    })
    .await
}

/// Peak level (0..1) of the played audio in `buckets` equal slices. Empty without audio.
#[tauri::command]
#[specta::specta]
pub async fn recording_waveform(
    lib: Lib<'_>,
    id: String,
    buckets: u32,
) -> Result<Vec<f32>, EngineError> {
    let lib = lib.inner().clone();
    off_thread(move || {
        let buckets = buckets.clamp(1, MAX_WAVEFORM_BUCKETS);
        let e = lib.find(&id)?;
        let file = lib.cache_file("waveforms", format!("{}-{buckets}.json", e.key));
        if let Some(p) = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<f32>>(&b).ok())
        {
            return Ok(p);
        }
        let peaks = media::waveform(&e.path, buckets).map_err(failed)?;
        if let (Some(dir), Ok(json)) = (file.parent(), serde_json::to_vec(&peaks)) {
            let _ = std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&file, json));
        }
        Ok(peaks)
    })
    .await
}

/// Where a trim writes first, and where the result ends up.
struct Target {
    write: PathBuf,
    final_path: PathBuf,
}

fn target(src: &Path, output: TrimOutput) -> Result<Target, EngineError> {
    let dir = src.parent().ok_or_else(not_found)?;
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = src
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mov".into());
    let taken = |n: &str| dir.join(n).exists();
    Ok(match output {
        TrimOutput::Copy => {
            let p = dir.join(library::unique_name(&stem, "-trim", &ext, taken));
            Target {
                write: p.clone(),
                final_path: p,
            }
        }
        // Hidden while it is written, so the list never shows a half-written file.
        TrimOutput::Replace => Target {
            write: dir.join(library::unique_name(
                &format!(".{stem}"),
                ".trimming",
                &ext,
                taken,
            )),
            final_path: src.to_path_buf(),
        },
    })
}

/// Export `start..end` of a recording (see `TrimRequest`). Copy writes "<name>-trim.<ext>";
/// Replace moves the original to the Trash and puts the trimmed file at its name. Progress
/// arrives as `trim://progress`; `cancel_trim` stops it. Returns the new recording.
#[tauri::command]
#[specta::specta]
pub async fn trim_recording(
    app: AppHandle,
    lib: Lib<'_>,
    id: String,
    request: TrimRequest,
) -> Result<Recording, EngineError> {
    let lib = lib.inner().clone();
    off_thread(move || {
        let e = lib.find(&id)?;
        let (start, end) =
            library::trim_range(request.start_secs, request.end_secs, e.info.duration_secs)?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = lock(&lib.jobs);
            if jobs.contains_key(&id) {
                return Err(failed("This recording is already being exported"));
            }
            jobs.insert(id.clone(), cancel.clone());
        }
        let result = run_trim(&app, &e, &id, start, end, request, &cancel);
        lock(&lib.jobs).remove(&id);
        let path = result?;
        let list = rescan(&app)?;
        let path = path.display().to_string();
        let rec = list
            .into_iter()
            .find(|r| r.path == path)
            .ok_or_else(not_found)?;
        // Like a new recording: the toast and the menu bar list pick it up.
        (lib.emit)(Outbound::RecordingFinished(rec.clone()));
        Ok(rec)
    })
    .await
}

fn run_trim(
    app: &AppHandle,
    e: &Entry,
    id: &str,
    start: f64,
    end: f64,
    request: TrimRequest,
    cancel: &AtomicBool,
) -> Result<PathBuf, EngineError> {
    let t = target(&e.path, request.output)?;
    let mut last = Instant::now();
    let mut progress = |p: f64| {
        if last.elapsed() >= Duration::from_millis(90) || p >= 1.0 {
            last = Instant::now();
            let _ = TrimProgressEvent(TrimProgress {
                id: id.to_string(),
                progress: p,
            })
            .emit(app);
        }
    };
    progress(0.0);
    let opts = ExportOptions {
        src: &e.path,
        dst: &t.write,
        start_secs: start,
        end_secs: end,
        mode: request.mode,
        keep: request.keep,
    };
    if let Err(err) = media::export(&opts, cancel, &mut progress) {
        let _ = std::fs::remove_file(&t.write);
        return Err(failed(if err.to_string() == "cancelled" {
            "Export cancelled".to_string()
        } else {
            format!("Trim failed: {err:#}")
        }));
    }
    if t.write != t.final_path {
        // Replace: the original goes to the Trash (recoverable), never deleted outright.
        if let Err(err) = media::trash(&t.final_path) {
            let _ = std::fs::remove_file(&t.write);
            return Err(failed(format!("Could not replace the original: {err:#}")));
        }
        std::fs::rename(&t.write, &t.final_path).map_err(|err| {
            failed(format!(
                "The original is in the Trash, but the trimmed file kept its temporary name: {err}"
            ))
        })?;
    }
    Ok(t.final_path)
}

/// Stop a running export of `id` (no-op if none runs).
#[tauri::command]
#[specta::specta]
pub fn cancel_trim(lib: Lib<'_>, id: String) {
    if let Some(c) = lock(&lib.jobs).get(&id) {
        c.store(true, Ordering::Relaxed);
    }
}

/// `cortex-media://localhost/<recording id>`: the recording's bytes, with Range support,
/// for the Trim window's `<video>`. Anything else gets 403 or 404.
pub fn media_protocol<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let allowed = MEDIA_WINDOWS.contains(&ctx.webview_label());
    let lib = ctx
        .app_handle()
        .try_state::<Arc<Library>>()
        .map(|l| l.inner().clone());
    std::thread::spawn(move || {
        let response = match (allowed, lib) {
            (true, Some(lib)) => serve(&lib, &request),
            _ => status(StatusCode::FORBIDDEN),
        };
        responder.respond(response);
    });
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut r = Response::new(vec![]);
    *r.status_mut() = code;
    r
}

fn serve(lib: &Library, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let id = percent_decode(request.uri().path().trim_start_matches('/'));
    let Some(path) = id.and_then(|id| lib.media_path(&id)) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(mut file) = std::fs::File::open(&path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let len = file.metadata().map_or(0, |m| m.len());
    let mime = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
    {
        "video/mp4"
    } else {
        "video/quicktime"
    };
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok());
    let (start, end, partial) = match range {
        None => (0, len.saturating_sub(1).min(MAX_CHUNK - 1), false),
        Some(h) => match parse_range(h, len) {
            Some((s, e)) => (s, e.min(s + MAX_CHUNK - 1), true),
            None => {
                let mut r = status(StatusCode::RANGE_NOT_SATISFIABLE);
                if let Ok(v) = format!("bytes */{len}").parse() {
                    r.headers_mut().insert(header::CONTENT_RANGE, v);
                }
                return r;
            }
        },
    };
    let mut body = vec![
        0u8;
        if len == 0 {
            0
        } else {
            (end - start + 1) as usize
        }
    ];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    // A file larger than one chunk is always answered as a range, so the player keeps asking.
    if partial || end + 1 < len {
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    builder
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

/// A single `bytes=` range as inclusive (start, end) within `len`, or None if unsatisfiable.
fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let spec = h.trim().strip_prefix("bytes=")?;
    // Multiple ranges are answered with the first; players only ask for one.
    let spec = spec.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    if len == 0 {
        return None;
    }
    let (start, end) = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return None;
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let s: u64 = a.parse().ok()?;
        let e = if b.is_empty() {
            len - 1
        } else {
            b.parse::<u64>().ok()?.min(len - 1)
        };
        (s, e)
    };
    (start <= end && start < len).then_some((start, end))
}

/// `%XX`-decoded UTF-8, or None for malformed input.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= c.len() {
                out.push(T[(n >> shift & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-1", 100), Some((0, 1)));
        assert_eq!(parse_range("bytes=10-", 100), Some((10, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=0-0,5-9", 100), Some((0, 0)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=5-1", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
    }

    #[test]
    fn decoding() {
        assert_eq!(
            percent_decode("Cortex%202026-11-03%20at%2010.00").as_deref(),
            Some("Cortex 2026-11-03 at 10.00")
        );
        assert_eq!(percent_decode("caf%C3%A9").as_deref(), Some("café"));
        assert_eq!(percent_decode("bad%2"), None);
        assert_eq!(percent_decode("%FF"), None);
    }

    #[test]
    fn base64_matches_rfc4648() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(raw.as_bytes()), enc);
        }
    }

    #[test]
    fn copy_and_replace_targets() {
        let dir = std::env::temp_dir().join(format!("cortex-trim-target-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("Demo.mov");
        std::fs::write(&src, b"x").unwrap();
        std::fs::write(dir.join("Demo-trim.mov"), b"x").unwrap();
        let copy = target(&src, TrimOutput::Copy).unwrap();
        assert_eq!(copy.write, dir.join("Demo-trim 2.mov"));
        assert_eq!(copy.write, copy.final_path);
        let replace = target(&src, TrimOutput::Replace).unwrap();
        assert_eq!(replace.write, dir.join(".Demo.trimming.mov"));
        assert_eq!(replace.final_path, src);
        assert!(!library::is_recording_file(&replace.write));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_lists_recordings_newest_first_and_skips_others() {
        let dir = std::env::temp_dir().join(format!("cortex-trim-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Not media: left out (unreadable), as are hidden and other files.
        std::fs::write(dir.join("broken.mov"), b"not a movie").unwrap();
        std::fs::write(dir.join(".hidden.mov"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let lib = Library::new(dir.join("cache"), Arc::new(|_| {}));
        assert!(lib.scan(&dir, false).is_empty());
        assert!(lib.media_path("broken").is_none());
        assert!(matches!(lib.find("../x"), Err(EngineError::Failed { .. })));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
