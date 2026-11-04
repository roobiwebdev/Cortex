//! SOURCE OF TRUTH KEYWORDS: log_line, cortex.log, log file, log rotation, panic hook, app_log_dir, open_log_folder, diagnostics
//! WHAT:  The shell's log. `log_line!` prints `cortex: …` to stderr, as the shell always
//!        did, and appends the same line with a UTC timestamp to `cortex.log` in the
//!        platform log folder. The file is rotated at 5 MB into `cortex.1.log`.
//! WHY:   A user's problem has to be diagnosable from a file they can send us. This is a
//!        small appender instead of `tauri-plugin-log`: no new crates, no IPC command for
//!        the webview, and only our own lines (not every dependency's `log` records), so we
//!        decide what reaches the file. Lines are scrubbed of stream URLs and tokens.
//! WHERE: `log_line!` throughout the shell; `init` from the `cortex-log` plugin in
//!        `lib.rs` (before any window exists); `dir` for the `open_log_folder` command.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// The current log, in the folder `init` was given.
pub const FILE: &str = "cortex.log";
/// The one previous log kept after a rotation.
pub const PREVIOUS: &str = "cortex.1.log";
/// Rotate before the file grows past this.
const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Lines logged before `init` are kept (up to this many) and written once it runs.
const MAX_PENDING: usize = 256;

/// `cortex: <message>` on stderr, and the message in `cortex.log`.
macro_rules! log_line {
    ($($arg:tt)*) => {
        $crate::logfile::line(&format!($($arg)*))
    };
}

struct Log {
    dir: Option<PathBuf>,
    file: Option<File>,
    size: u64,
    max: u64,
    pending: Vec<String>,
}

impl Log {
    const fn new(max: u64) -> Log {
        Log {
            dir: None,
            file: None,
            size: 0,
            max,
            pending: Vec::new(),
        }
    }

    fn open(&mut self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        self.dir = Some(dir.to_path_buf());
        let size = std::fs::metadata(dir.join(FILE)).map_or(0, |m| m.len());
        if size >= self.max {
            self.rotate()
        } else {
            self.file = Some(append(&dir.join(FILE))?);
            self.size = size;
            Ok(())
        }
    }

    /// `cortex.log` becomes `cortex.1.log` (replacing it) and a new file starts.
    fn rotate(&mut self) -> std::io::Result<()> {
        let Some(dir) = self.dir.clone() else {
            return Ok(());
        };
        // Closed first: Windows cannot rename a file that is still open.
        self.file = None;
        let _ = std::fs::rename(dir.join(FILE), dir.join(PREVIOUS));
        self.file = Some(append(&dir.join(FILE))?);
        self.size = 0;
        Ok(())
    }

    fn write(&mut self, entry: &str) {
        if self.file.is_none() {
            if self.dir.is_none() && self.pending.len() < MAX_PENDING {
                self.pending.push(entry.to_string());
            }
            return;
        }
        let len = entry.len() as u64;
        if self.size > 0 && self.size + len > self.max && self.rotate().is_err() {
            return;
        }
        if let Some(f) = &mut self.file {
            match f.write_all(entry.as_bytes()) {
                Ok(()) => self.size += len,
                // A full or vanished disk: stop trying instead of failing every line.
                Err(_) => self.file = None,
            }
        }
    }

    fn flush_pending(&mut self) {
        for entry in std::mem::take(&mut self.pending) {
            self.write(&entry);
        }
    }
}

fn append(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

static LOG: Mutex<Log> = Mutex::new(Log::new(MAX_BYTES));

fn log() -> MutexGuard<'static, Log> {
    LOG.lock().unwrap_or_else(|p| p.into_inner())
}

/// Start writing to `dir/cortex.log` (created if missing), after the lines logged so far.
pub fn init(dir: &Path) -> std::io::Result<()> {
    let mut log = log();
    log.open(dir)?;
    log.flush_pending();
    Ok(())
}

/// The log folder, once `init` ran.
pub fn dir() -> Option<PathBuf> {
    log().dir.clone()
}

/// What `log_line!` does: stderr and the file.
pub fn line(message: &str) {
    eprintln!("cortex: {message}");
    record(message);
}

/// The file only (for lines that also go to stdout, which scripts read).
pub fn record(message: &str) {
    let entry = entry(SystemTime::now(), message);
    log().write(&entry);
}

fn entry(at: SystemTime, message: &str) -> String {
    let mut s = format!("{} {}", timestamp(at), scrub(message));
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Panics go to the log as well as stderr (with a backtrace), then to the default hook.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let trace = std::backtrace::Backtrace::force_capture();
        let entry = entry(
            SystemTime::now(),
            &format!("panic on thread {name}: {info}\n{trace}"),
        );
        // `try_lock`: a panic while this thread holds the log must not deadlock.
        if let Ok(mut log) = LOG.try_lock() {
            log.write(&entry);
        }
        previous(info);
    }));
}

/// `2026-11-04T09:30:12.345Z`.
fn timestamp(at: SystemTime) -> String {
    let d = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let (y, m, day) = civil_date((secs / 86_400) as i64);
    let s = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        s / 3600,
        s % 3600 / 60,
        s % 60,
        d.subsec_millis()
    )
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian (Howard Hinnant's algorithm).
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Keeps stream keys out of the log, whatever a message carries: an RTMP or SRT URL keeps
/// only its scheme and host (the key is part of the path or the `streamid`), and a bearer
/// token is dropped. Engines already redact their own errors; this is the second line.
fn scrub(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(at) = find_secret(rest) {
        out.push_str(&rest[..at.start]);
        out.push_str(&rest[at.start..at.keep]);
        out.push_str("<redacted>");
        rest = &rest[at.end..];
    }
    out.push_str(rest);
    out
}

struct Secret {
    start: usize,
    /// Up to here is kept (scheme and host, or the word "Bearer ").
    keep: usize,
    end: usize,
}

fn find_secret(text: &str) -> Option<Secret> {
    let lower = text.to_ascii_lowercase();
    let token_end = |from: usize| {
        from + text[from..]
            .find(char::is_whitespace)
            .unwrap_or(text.len() - from)
    };
    let url = ["rtmp://", "rtmps://", "srt://"]
        .iter()
        .filter_map(|scheme| lower.find(scheme).map(|i| (i, scheme.len())))
        .min_by_key(|&(i, _)| i)
        .map(|(start, scheme)| {
            let end = token_end(start);
            let host = start + scheme;
            let keep = host + text[host..end].find(['/', '?']).unwrap_or(end - host);
            Secret { start, keep, end }
        });
    let bearer = lower.find("bearer ").map(|start| {
        let keep = start + "bearer ".len();
        Secret {
            start,
            keep,
            end: token_end(keep),
        }
    });
    match (url, bearer) {
        (Some(u), Some(b)) => Some(if u.start <= b.start { u } else { b }),
        (u, b) => u.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cortex-logfile-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn timestamps_are_utc_iso() {
        let t = |s| timestamp(UNIX_EPOCH + Duration::from_millis(s));
        assert_eq!(t(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(t(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(t(1_793_784_612_345), "2026-11-04T09:30:12.345Z");
    }

    #[test]
    fn stream_keys_never_reach_the_log() {
        assert_eq!(
            scrub("could not connect to rtmp://live.example.com/app/sk_live_123 (timeout)"),
            "could not connect to rtmp://live.example.com<redacted> (timeout)"
        );
        assert_eq!(
            scrub("RTMPS://a.b:443/x/KEY and srt://h:9000?streamid=KEY"),
            "RTMPS://a.b:443<redacted> and srt://h:9000<redacted>"
        );
        assert_eq!(
            scrub("401 with Authorization: Bearer abc.def"),
            "401 with Authorization: Bearer <redacted>"
        );
        assert_eq!(scrub("recorded /tmp/a.mov"), "recorded /tmp/a.mov");
    }

    #[test]
    fn lines_before_init_are_kept_and_rotation_keeps_one_previous_file() {
        let dir = scratch("rotate");
        let mut log = Log::new(100);
        log.write("early\n");
        log.open(&dir).unwrap();
        log.flush_pending();
        let read = |f| std::fs::read_to_string(dir.join(f)).unwrap_or_default();
        assert_eq!(read(FILE), "early\n");
        let line = format!("{}\n", "x".repeat(59));
        log.write(&line);
        assert_eq!(read(FILE), format!("early\n{line}"));
        // 66 + 60 > 100: rotate, then write into a new file.
        log.write(&line);
        assert_eq!(read(PREVIOUS), format!("early\n{line}"));
        assert_eq!(read(FILE), line);
        log.write("y\n");
        log.write(&line);
        assert_eq!(read(PREVIOUS), format!("{line}y\n"));
        assert_eq!(read(FILE), line);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_full_file_is_rotated_at_launch() {
        let dir = scratch("launch");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), "z".repeat(120)).unwrap();
        let mut log = Log::new(100);
        log.open(&dir).unwrap();
        log.write("new\n");
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), "new\n");
        assert_eq!(
            std::fs::read_to_string(dir.join(PREVIOUS)).unwrap().len(),
            120
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
