//! SOURCE OF TRUTH KEYWORDS: SettingsStore, load_settings, save_atomic, debounce, import_rec_settings, settings.json
//! WHAT:  Settings persistence: versioned JSON, tolerant load, atomic save (temp file + rename),
//!        a debounced background writer, and a one-time import of Rec's settings.json.
//! WHY:   Rec wrote its file on the UI thread on every fader tick, non-atomically. Here writes
//!        are coalesced off the caller's thread and a crash mid-write never leaves a torn file.
//!        The path is passed in (e.g. ~/Library/Application Support/dev.roobi.cortex/settings.json);
//!        this crate reads no environment.
//! WHERE: the Tauri shell (open at launch, `save` on every settings change, `flush` on quit).

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use crate::settings::{
    AudioTracks, CaptureTarget, Codec, Container, PipCorner, PipPlace, PipShape, RecorderSettings,
    Resolution, SETTINGS_VERSION, Scene, StreamCodec, StreamDest, StreamService, migrate,
};

/// The result of reading the settings file.
#[derive(Debug)]
pub struct Loaded {
    pub settings: RecorderSettings,
    /// Set when the file existed but couldn't be used; defaults were loaded and
    /// the bad file was kept next to it as `<name>.corrupt`. Show it to the user.
    pub error: Option<String>,
}

/// Read settings from `path`. A missing file gives the defaults silently; an
/// unreadable or corrupt one gives the defaults and an error.
pub fn load_settings(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Loaded {
                settings: RecorderSettings::default(),
                error: None,
            };
        }
        Err(e) => {
            return Loaded {
                settings: RecorderSettings::default(),
                error: Some(format!("Could not read settings: {e}")),
            };
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(v @ Value::Object(_)) => Loaded {
            settings: migrate(v),
            error: None,
        },
        _ => {
            let backup = sibling(path, "corrupt");
            let kept = std::fs::rename(path, &backup).is_ok();
            let mut msg = "Settings file was damaged; defaults were restored".to_string();
            if kept {
                msg += &format!(" (the old file is at {})", backup.display());
            }
            Loaded {
                settings: RecorderSettings::default(),
                error: Some(msg),
            }
        }
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{suffix}"));
    path.with_file_name(name)
}

/// The JSON written to disk: the settings plus `version`.
pub fn to_file_json(s: &RecorderSettings) -> String {
    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.insert("version".into(), SETTINGS_VERSION.into());
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Write atomically: a temp file in the same folder, flushed, then renamed
/// over `path`. Readers see the old file or the new one, never a mix.
pub fn save_atomic(path: &Path, s: &RecorderSettings) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = sibling(path, &format!("tmp-{}", std::process::id()));
    let r = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(to_file_json(s).as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

enum Msg {
    Save(Box<RecorderSettings>),
    Flush(mpsc::Sender<std::io::Result<()>>),
}

/// Called with a user-facing message when a background write fails.
pub type ErrorFn = Box<dyn Fn(String) + Send>;

/// Saves settings on a background thread, coalescing changes that come
/// within `debounce` of each other (a fader drag is one write).
pub struct SettingsStore {
    path: PathBuf,
    tx: Option<mpsc::Sender<Msg>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// The delay after the last change before it is written.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(400);

impl SettingsStore {
    /// Load `path` (see `load_settings`) and start the writer.
    pub fn open(
        path: PathBuf,
        debounce: Duration,
        on_error: Option<ErrorFn>,
    ) -> (SettingsStore, Loaded) {
        let loaded = load_settings(&path);
        let (tx, rx) = mpsc::channel::<Msg>();
        let p = path.clone();
        let thread = std::thread::Builder::new()
            .name("settings-writer".into())
            .spawn(move || writer(&p, debounce, rx, on_error))
            .ok();
        (
            SettingsStore {
                path,
                tx: Some(tx),
                thread,
            },
            loaded,
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Queue a save; returns at once.
    pub fn save(&self, s: &RecorderSettings) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Save(Box::new(s.clone())));
        }
    }

    /// Write any pending change now and wait for it.
    pub fn flush(&self) -> std::io::Result<()> {
        let (ack, done) = mpsc::channel();
        if let Some(tx) = &self.tx
            && tx.send(Msg::Flush(ack)).is_ok()
        {
            return done.recv().unwrap_or(Ok(()));
        }
        Ok(())
    }
}

impl Drop for SettingsStore {
    /// Pending changes are written before the store goes away.
    fn drop(&mut self) {
        self.tx.take();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn writer(path: &Path, debounce: Duration, rx: mpsc::Receiver<Msg>, on_error: Option<ErrorFn>) {
    let write = |s: &RecorderSettings| {
        let r = save_atomic(path, s);
        if let (Err(e), Some(f)) = (&r, &on_error) {
            f(format!("Could not save settings: {e}"));
        }
        r
    };
    let mut pending: Option<Box<RecorderSettings>> = None;
    loop {
        let msg = if pending.is_some() {
            rx.recv_timeout(debounce)
        } else {
            rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match msg {
            Ok(Msg::Save(s)) => pending = Some(s),
            Ok(Msg::Flush(ack)) => {
                let r = pending.take().map_or(Ok(()), |s| write(&s));
                let _ = ack.send(r);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(s) = pending.take() {
                    let _ = write(&s);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(s) = pending.take() {
                    let _ = write(&s);
                }
                return;
            }
        }
    }
}

// ---- Rec import -------------------------------------------------------------

/// Rec 0.1.2's settings.json (snake_case), as far as it maps to ours.
#[derive(Deserialize)]
#[serde(default)]
struct RecFile {
    output_dir: Option<String>,
    container: String,
    codec: Option<String>,
    resolution: String,
    fps: u32,
    bitrate_mbps: Option<f64>,
    display: u32,
    scene: String,
    camera: Option<String>,
    mic_enabled: bool,
    mic: Option<String>,
    system_audio: bool,
    audio_tracks: String,
    system_gain_db: f32,
    mic_gain_db: f32,
    system_muted: bool,
    mic_muted: bool,
    pip_size: f64,
    pip_place: Value,
    pip_shape: String,
    pip_roundness: f64,
    pip_border: f64,
    pip_border_color: [u8; 3],
    pip_shadow: bool,
    mirror: bool,
    cursor: bool,
    clicks: bool,
    preview_low_power: bool,
    replay_enabled: bool,
    replay_secs: u32,
    stream_dests: Option<Vec<RecDest>>,
    /// Before multi-destination streaming: one service or "custom".
    stream_dest: Option<String>,
    stream_custom_url: Option<String>,
    live_resolution: String,
    stream_bitrate_kbps: u32,
    stream_bitrate_auto: bool,
    stream_codec: String,
    keyframe_secs: f64,
    stream_cbr: bool,
    stream_audio_kbps: u32,
}

#[derive(Deserialize)]
struct RecDest {
    #[serde(default)]
    dest: String,
    #[serde(default)]
    enabled: bool,
}

/// Rec's own defaults, for fields missing from its file.
impl Default for RecFile {
    fn default() -> Self {
        RecFile {
            output_dir: None,
            container: "mov".into(),
            codec: None,
            resolution: "1080p".into(),
            fps: 60,
            bitrate_mbps: None,
            display: 1,
            scene: "screen".into(),
            camera: None,
            mic_enabled: true,
            mic: None,
            system_audio: true,
            audio_tracks: "both".into(),
            system_gain_db: 0.0,
            mic_gain_db: 0.0,
            system_muted: false,
            mic_muted: false,
            pip_size: 0.22,
            pip_place: Value::Null,
            pip_shape: "rounded".into(),
            pip_roundness: 0.2,
            pip_border: 0.0,
            pip_border_color: [255, 255, 255],
            pip_shadow: false,
            mirror: true,
            cursor: true,
            clicks: false,
            preview_low_power: false,
            replay_enabled: false,
            replay_secs: 60,
            stream_dests: None,
            stream_dest: None,
            stream_custom_url: None,
            live_resolution: "1080p".into(),
            stream_bitrate_kbps: 6000,
            stream_bitrate_auto: true,
            stream_codec: "auto".into(),
            keyframe_secs: 2.0,
            stream_cbr: true,
            stream_audio_kbps: 192,
        }
    }
}

const SERVICES: [StreamService; 3] = [
    StreamService::Youtube,
    StreamService::Twitch,
    StreamService::Kick,
];

/// Rec's destinations, with its two migrations applied: a single old
/// `stream_dest` becomes the only enabled one; every service is listed in
/// order, then custom servers.
fn rec_dests(f: &RecFile) -> Vec<StreamDest> {
    let mut saved: Vec<(String, bool)> = match &f.stream_dests {
        Some(d) => d
            .iter()
            .map(|d| (d.dest.trim().to_string(), d.enabled))
            .collect(),
        None => match f.stream_dest.as_deref() {
            Some("custom") => {
                let url = f
                    .stream_custom_url
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if url.is_empty() {
                    vec![]
                } else {
                    vec![(url, true)]
                }
            }
            Some(old) => vec![(old.to_string(), true)],
            None => vec![("youtube".into(), true)],
        },
    };
    saved.retain(|(d, _)| !d.is_empty());
    let mut out: Vec<StreamDest> = SERVICES
        .iter()
        .map(|s| StreamDest {
            service: *s,
            url: None,
            enabled: saved.iter().any(|(d, on)| d == s.as_str() && *on),
            key_stored: false,
        })
        .collect();
    for (d, on) in saved {
        if StreamService::parse(&d).is_none()
            && !out.iter().any(|o| o.url.as_deref() == Some(d.as_str()))
        {
            out.push(StreamDest {
                service: StreamService::Custom,
                url: Some(d),
                enabled: on,
                key_stored: false,
            });
        }
    }
    out
}

fn rec_place(v: &Value) -> PipPlace {
    if let Some(c) = v
        .get("corner")
        .and_then(Value::as_str)
        .and_then(PipCorner::parse)
    {
        return PipPlace::Corner { corner: c };
    }
    if let Some([x, y]) = v
        .get("at")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .as_deref()
    {
        return PipPlace::Free { x: *x, y: *y };
    }
    PipPlace::default()
}

/// Map Rec's settings JSON onto ours. Stream keys stay where they are: Rec
/// keyed them by URL in the Keychain, and so does the engine.
pub fn rec_value_to_settings(v: Value) -> RecorderSettings {
    let f: RecFile = serde_json::from_value(v).unwrap_or_default();
    let d = RecorderSettings::default();
    let [r, g, b] = f.pip_border_color;
    RecorderSettings {
        output_dir: f
            .output_dir
            .clone()
            .filter(|o| !o.is_empty())
            .unwrap_or(d.output_dir.clone()),
        container: Container::parse(&f.container).unwrap_or(d.container),
        codec: f
            .codec
            .as_deref()
            .and_then(Codec::parse)
            .unwrap_or(Codec::Auto),
        resolution: Resolution::parse(&f.resolution.to_lowercase()).unwrap_or(d.resolution),
        fps: f.fps,
        bitrate_mbps: f.bitrate_mbps,
        target: CaptureTarget::Display {
            display_id: f.display,
        },
        scene: Scene::parse(&f.scene).unwrap_or(Scene::Screen),
        camera_id: f.camera.clone(),
        mic_enabled: f.mic_enabled,
        mic_id: f.mic.clone(),
        system_audio: f.system_audio,
        audio_tracks: AudioTracks::parse(&f.audio_tracks).unwrap_or_default(),
        system_gain_db: f.system_gain_db,
        mic_gain_db: f.mic_gain_db,
        system_muted: f.system_muted,
        mic_muted: f.mic_muted,
        pip_size: f.pip_size,
        pip_place: rec_place(&f.pip_place),
        pip_shape: PipShape::parse(&f.pip_shape).unwrap_or(PipShape::Rounded),
        pip_roundness: f.pip_roundness,
        pip_border: f.pip_border,
        pip_border_color: format!("{r:02x}{g:02x}{b:02x}"),
        pip_shadow: f.pip_shadow,
        mirror: f.mirror,
        cursor: f.cursor,
        clicks: f.clicks,
        preview_low_power: f.preview_low_power,
        replay_enabled: f.replay_enabled,
        replay_secs: f.replay_secs,
        stream_dests: rec_dests(&f),
        live_resolution: Resolution::parse(&f.live_resolution.to_lowercase())
            .unwrap_or(d.live_resolution),
        stream_bitrate_kbps: f.stream_bitrate_kbps,
        stream_bitrate_auto: f.stream_bitrate_auto,
        stream_codec: StreamCodec::parse(&f.stream_codec).unwrap_or_default(),
        keyframe_secs: f.keyframe_secs,
        stream_cbr: f.stream_cbr,
        // 160 kb/s was Rec's old default; it moved to 192.
        stream_audio_kbps: if f.stream_audio_kbps == 160 {
            192
        } else {
            f.stream_audio_kbps
        },
        // Rec had no countdown: keep recording instantly, as Rec users expect.
        countdown: 0,
        ..d
    }
    .validated()
}

/// Read Rec's settings.json at `path` (normally
/// ~/Library/Application Support/rec-rs/settings.json) into our settings.
pub fn import_rec_settings(path: &Path) -> Result<RecorderSettings, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    match serde_json::from_str::<Value>(&text) {
        Ok(v @ Value::Object(_)) => Ok(rec_value_to_settings(v)),
        _ => Err(format!("{} is not a Rec settings file", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rec-core-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn round_trip_through_the_file() {
        let p = tmpdir("roundtrip").join("sub/settings.json");
        let s = RecorderSettings {
            fps: 30,
            pip_place: PipPlace::Free { x: 0.2, y: 0.8 },
            mic_id: Some("m".into()),
            ..Default::default()
        };
        save_atomic(&p, &s).unwrap();
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(raw["version"], SETTINGS_VERSION);
        let l = load_settings(&p);
        assert!(l.error.is_none());
        assert_eq!(l.settings, s);
        let leftovers: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "no temp file left: {leftovers:?}");
    }

    #[test]
    fn missing_file_gives_defaults_without_error() {
        let l = load_settings(&tmpdir("missing").join("settings.json"));
        assert!(l.error.is_none());
        assert_eq!(l.settings, RecorderSettings::default());
    }

    #[test]
    fn corrupt_file_gives_defaults_an_error_and_a_backup() {
        let d = tmpdir("corrupt");
        let p = d.join("settings.json");
        std::fs::write(&p, "{ not json").unwrap();
        let l = load_settings(&p);
        assert_eq!(l.settings, RecorderSettings::default());
        assert!(l.error.as_deref().unwrap().contains("damaged"));
        assert_eq!(
            std::fs::read_to_string(d.join("settings.json.corrupt")).unwrap(),
            "{ not json"
        );
        std::fs::write(&p, "[1,2]").unwrap();
        assert!(
            load_settings(&p).error.is_some(),
            "valid JSON of the wrong shape is corrupt too"
        );
    }

    #[test]
    fn old_and_partial_files_migrate() {
        let p = tmpdir("migrate").join("settings.json");
        std::fs::write(&p, r#"{"fps": 24, "pipSize": 0.9, "scene": "cam"}"#).unwrap();
        let l = load_settings(&p);
        assert!(l.error.is_none());
        assert_eq!(
            (l.settings.fps, l.settings.pip_size, l.settings.scene),
            (24, 0.5, Scene::Cam)
        );
        assert_eq!(l.settings.output_dir, crate::settings::DEFAULT_OUTPUT_DIR);
    }

    #[test]
    fn debounced_writer_coalesces_and_flushes() {
        let p = tmpdir("debounce").join("settings.json");
        let (store, loaded) = SettingsStore::open(p.clone(), Duration::from_secs(30), None);
        assert!(loaded.error.is_none());
        for db in 0..50 {
            store.save(&RecorderSettings {
                mic_gain_db: -(db as f32),
                ..Default::default()
            });
        }
        // Inside the debounce window nothing is written: the caller never waits on disk.
        assert!(!p.exists());
        store.flush().unwrap();
        assert_eq!(load_settings(&p).settings.mic_gain_db, -49.0);
        store.save(&RecorderSettings {
            mic_gain_db: 6.0,
            ..Default::default()
        });
        drop(store);
        assert_eq!(
            load_settings(&p).settings.mic_gain_db,
            6.0,
            "dropping the store writes pending changes"
        );
    }

    #[test]
    fn debounced_writer_writes_after_the_quiet_period() {
        let p = tmpdir("quiet").join("settings.json");
        let (store, _) = SettingsStore::open(p.clone(), Duration::from_millis(30), None);
        store.save(&RecorderSettings {
            fps: 25,
            ..Default::default()
        });
        let t = std::time::Instant::now();
        while !p.exists() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(load_settings(&p).settings.fps, 25);
        drop(store);
    }

    #[test]
    fn write_errors_are_reported_not_panicked() {
        let d = tmpdir("err");
        // A directory where the file should be: rename fails.
        let p = d.join("settings.json");
        std::fs::create_dir_all(p.join("x")).unwrap();
        let (tx, rx) = mpsc::channel();
        let (store, _) = SettingsStore::open(
            p,
            Duration::from_millis(5),
            Some(Box::new(move |m| tx.send(m).unwrap())),
        );
        store.save(&RecorderSettings::default());
        assert!(store.flush().is_err());
        assert!(
            rx.recv_timeout(Duration::from_secs(2))
                .unwrap()
                .starts_with("Could not save settings")
        );
    }

    #[test]
    fn imports_rec_settings_from_a_real_file() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rec-settings.json");
        let s = import_rec_settings(&p).unwrap();
        assert_eq!(s.output_dir, "/Users/roobi/Movies");
        assert_eq!(
            (s.container, s.codec, s.resolution, s.fps),
            (Container::Mov, Codec::Auto, Resolution::R1080p, 60)
        );
        assert_eq!(s.target, CaptureTarget::Display { display_id: 1 });
        assert_eq!(s.scene, Scene::ScreenCam);
        assert_eq!((s.camera_id.as_deref(), s.mic_id.as_deref()), (None, None));
        assert_eq!(s.system_gain_db, -0.5);
        assert_eq!(
            s.pip_place,
            PipPlace::Corner {
                corner: PipCorner::TopRight
            }
        );
        assert_eq!(
            (s.pip_shape, s.pip_roundness, s.pip_shadow),
            (PipShape::Squircle, 0.2, true)
        );
        assert_eq!(s.pip_border_color, "ffffff");
        let on: Vec<_> = s
            .stream_dests
            .iter()
            .filter(|d| d.enabled)
            .map(|d| d.service)
            .collect();
        assert_eq!(on, [StreamService::Youtube]);
        assert_eq!(s.stream_dests.len(), 3);
        assert_eq!(s.countdown, 0);
        assert!(s.clone().validate().is_empty());
    }

    #[test]
    fn rec_import_applies_recs_migrations() {
        let s = rec_value_to_settings(
            json!({"stream_dest": "custom", "stream_custom_url": " rtmp://h/live ", "stream_audio_kbps": 160}),
        );
        let on: Vec<_> = s.stream_dests.iter().filter(|d| d.enabled).collect();
        assert_eq!(on.len(), 1);
        assert_eq!(
            (on[0].service, on[0].url.as_deref()),
            (StreamService::Custom, Some("rtmp://h/live"))
        );
        assert_eq!(s.stream_audio_kbps, 192);
        let s = rec_value_to_settings(
            json!({"stream_dest": "twitch", "pip_place": {"at": [0.3, 0.6]}, "pip_border_color": [10, 200, 30], "codec": "prores", "resolution": "4k"}),
        );
        assert_eq!(
            s.stream_dests
                .iter()
                .filter(|d| d.enabled)
                .map(|d| d.service)
                .collect::<Vec<_>>(),
            [StreamService::Twitch]
        );
        assert_eq!(s.pip_place, PipPlace::Free { x: 0.3, y: 0.6 });
        assert_eq!(
            (s.pip_border_color.as_str(), s.codec, s.resolution),
            ("0ac81e", Codec::Prores, Resolution::R4k)
        );
        // Rec files are recognised by `migrate` too (an import through the normal load).
        assert_eq!(
            migrate(json!({"output_dir": "/x", "fps": 30})).output_dir,
            "/x"
        );
        assert!(import_rec_settings(Path::new("/nonexistent/settings.json")).is_err());
    }
}
