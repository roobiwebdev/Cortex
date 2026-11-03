//! SOURCE OF TRUTH KEYWORDS: updater, check_for_updates, install_update, update_status, updater://status, latest.json, auto update, CORTEX_DEBUG_UPDATE_URL
//! WHAT:  Software updates: checking the update feed, downloading and verifying the signed
//!        bundle, installing it and relaunching. Also the quiet daily check while
//!        `autoUpdate` is on.
//! WHY:   Everything runs in Rust behind three IPC commands. The webview gets no updater or
//!        process plugin permission, so a compromised page can at most ask for the one
//!        update the feed announces, which must carry a valid signature from our key
//!        (`plugins.updater.pubkey` in `tauri.conf.json`). Installing replaces the app and
//!        relaunches it, so it is refused while recording, counting down, finalizing or
//!        live (`Shell::begin_update`), and nothing can start until the relaunch.
//! WHERE: `lib.rs` (setup, relaunch at exit), `ipc.rs` (commands and event), the Settings
//!        Updates group and the menu bar panel (`apps/desktop/src/lib/updates.ts`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rec_core::api::EngineError;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State, Url};
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_specta::Event;

use crate::shell::Shell;

/// Debug hook: check this feed instead of the configured one. Loopback URLs only (see
/// `debug_feed`). Release builds still need `https` unless the build opted into plain
/// HTTP (`scripts/desktop/release-macos.sh --local-test-feed`), and the bundle must still
/// carry a valid signature from our key.
pub const DEBUG_URL_VAR: &str = "CORTEX_DEBUG_UPDATE_URL";
/// Debug hook, with `CORTEX_DEBUG_UPDATE_URL`: install what the launch check finds.
const DEBUG_INSTALL_VAR: &str = "CORTEX_DEBUG_UPDATE_INSTALL";

/// The quiet check runs at most this often.
const CHECK_EVERY_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// Let the app settle (preview, devices) before the first quiet check.
const LAUNCH_DELAY: Duration = Duration::from_secs(20);
/// How often a long-running app asks itself whether a quiet check is due.
const WAKE_EVERY: Duration = Duration::from_secs(60 * 60);
/// Progress events while downloading, at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);
/// Release notes come from the feed; keep what reaches the UI short.
const MAX_NOTES: usize = 2000;

/// An update the feed announced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    /// `pub_date` from the feed (RFC 3339), as given.
    pub date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub downloaded: f64,
    /// None when the server sends no length.
    pub total: Option<f64>,
}

/// What the Settings Updates group and the menu bar show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    /// Unix time in ms of the last check that reached the feed.
    pub last_checked: Option<f64>,
    pub checking: bool,
    /// A newer version the last check found.
    pub available: Option<UpdateInfo>,
    /// Set while the bundle downloads.
    pub download: Option<DownloadProgress>,
    /// Downloaded and verified; `install_update` installs it once nothing is recording.
    pub ready: bool,
    /// Installing; Cortex relaunches next.
    pub installing: bool,
    /// Why the last check, download or install failed. Cleared when the next one starts.
    pub error: Option<String>,
}

/// The update status changed (check, progress, ready, installing).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateStatusEvent(pub UpdateStatus);
impl Event for UpdateStatusEvent {
    const NAME: &'static str = "updater://status";
}

/// Written next to the settings, so the daily rule survives relaunches.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stamp {
    last_attempt: Option<f64>,
    last_checked: Option<f64>,
}

struct Inner {
    status: UpdateStatus,
    last_attempt: Option<f64>,
    pending: Option<Update>,
    bytes: Option<Arc<Vec<u8>>>,
    /// Relaunch the app from `RunEvent::Exit` (set after a successful install).
    relaunch: bool,
}

/// Managed state: the update status, the announced update and its verified bytes.
pub struct Updates {
    inner: Mutex<Inner>,
    stamp_path: PathBuf,
    feed: Option<Url>,
}

impl Updates {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn status(&self) -> UpdateStatus {
        self.lock().status.clone()
    }

    fn save_stamp(&self) {
        let stamp = {
            let g = self.lock();
            Stamp {
                last_attempt: g.last_attempt,
                last_checked: g.status.last_checked,
            }
        };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = self.stamp_path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&self.stamp_path, serde_json::to_vec(&stamp)?)
        };
        if let Err(e) = write() {
            eprintln!("cortex: could not save the update check time: {e}");
        }
    }
}

/// `CORTEX_DEBUG_UPDATE_URL` when it is an http(s) URL on this Mac. Anything else is
/// ignored, so the hook can never point an installed app at another server.
pub fn debug_feed(value: Option<&str>) -> Option<Url> {
    let url = Url::parse(value?.trim()).ok()?;
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    (loopback && matches!(url.scheme(), "http" | "https")).then_some(url)
}

/// A quiet check is due when none was attempted in the last day (or the clock went back).
pub fn check_due(last_attempt: Option<f64>, now: f64) -> bool {
    last_attempt.is_none_or(|t| now - t >= CHECK_EVERY_MS || now < t)
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        notes: u
            .body
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(|n| n.chars().take(MAX_NOTES).collect()),
        date: u
            .raw_json
            .get("pub_date")
            .and_then(|d| d.as_str())
            .map(String::from),
    }
}

fn emit(app: &AppHandle, status: UpdateStatus) {
    if let Err(e) = UpdateStatusEvent(status).emit(app) {
        eprintln!("cortex: failed to emit the update status: {e}");
    }
}

fn updates(app: &AppHandle) -> Result<Arc<Updates>, EngineError> {
    app.try_state::<Arc<Updates>>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| EngineError::failed("Updates are not available"))
}

/// Changes the status and tells every window.
fn set_status(app: &AppHandle, u: &Updates, f: impl FnOnce(&mut Inner)) -> UpdateStatus {
    let status = {
        let mut g = u.lock();
        f(&mut g);
        g.status.clone()
    };
    emit(app, status.clone());
    status
}

/**
 * SOURCE OF TRUTH KEYWORDS: updater::setup, launch check, daily update check, update-check.json
 * WHAT:  Creates the update state and starts the quiet check: 20 s after launch and then
 *        hourly, it checks when `autoUpdate` is on and no check ran in the last day.
 * WHY:   A menu bar app can stay open for weeks, so "once a day" can't only mean "at
 *        launch". The time of the last attempt is kept in `update-check.json` in the
 *        settings folder (`CORTEX_DEBUG_DATA_DIR` in test runs). With
 *        `CORTEX_DEBUG_UPDATE_URL` it checks once at launch regardless, and installs what
 *        it finds when `CORTEX_DEBUG_UPDATE_INSTALL=1`.
 * WHERE: `lib.rs` setup, after the shell is managed.
 */
pub fn setup(app: &tauri::App, data_dir: &Path) {
    let stamp_path = data_dir.join("update-check.json");
    let stamp: Stamp = std::fs::read(&stamp_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let raw_feed = var(DEBUG_URL_VAR);
    let feed = debug_feed(raw_feed.as_deref());
    if raw_feed.is_some() && feed.is_none() {
        eprintln!("cortex: {DEBUG_URL_VAR} ignored: only http(s) URLs on 127.0.0.1 or localhost");
    }
    let debug_install = feed.is_some() && var(DEBUG_INSTALL_VAR).is_some_and(|v| v == "1");
    let updates = Arc::new(Updates {
        inner: Mutex::new(Inner {
            status: UpdateStatus {
                current_version: app.package_info().version.to_string(),
                last_checked: stamp.last_checked,
                checking: false,
                available: None,
                download: None,
                ready: false,
                installing: false,
                error: None,
            },
            last_attempt: stamp.last_attempt,
            pending: None,
            bytes: None,
            relaunch: false,
        }),
        stamp_path,
        feed: feed.clone(),
    });
    app.manage(updates.clone());

    let handle = app.handle().clone();
    let spawned = std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            if feed.is_some() {
                // After a `CORTEX_DEBUG_RECORD_SECS` recording has started (at 3 s), so a
                // test run can check that installing waits for it.
                std::thread::sleep(Duration::from_secs(5));
                debug_run(&handle, debug_install);
                return;
            }
            std::thread::sleep(LAUNCH_DELAY);
            loop {
                let auto = handle
                    .try_state::<Arc<Shell>>()
                    .is_some_and(|s| s.settings().auto_update);
                let due = auto && check_due(updates.lock().last_attempt, now_ms());
                if due && let Err(e) = tauri::async_runtime::block_on(check(&handle)) {
                    eprintln!("cortex: update check failed: {e}");
                }
                std::thread::sleep(WAKE_EVERY);
            }
        });
    if let Err(e) = spawned {
        eprintln!("cortex: could not start the update check: {e}");
    }
}

/// The `CORTEX_DEBUG_UPDATE_URL` run: one check, its result on stdout, then an install
/// when asked.
fn debug_run(app: &AppHandle, install_found: bool) {
    match tauri::async_runtime::block_on(check(app)) {
        Ok(s) => match &s.available {
            Some(a) => {
                println!(
                    "cortex: update available {} -> {}",
                    s.current_version, a.version
                );
                if install_found {
                    match tauri::async_runtime::block_on(install(app)) {
                        Ok(_) => println!("cortex: update installed, relaunching"),
                        Err(e) => println!("cortex: update install failed: {e}"),
                    }
                }
            }
            None => println!("cortex: update check: up to date at {}", s.current_version),
        },
        Err(e) => println!("cortex: update check failed: {e}"),
    }
}

/// Asks the feed for a newer version. A check while another runs, or while an update
/// downloads or installs, returns the current status.
pub async fn check(app: &AppHandle) -> Result<UpdateStatus, EngineError> {
    let u = updates(app)?;
    {
        let mut g = u.lock();
        let busy = g.status.checking || g.status.download.is_some() || g.status.installing;
        if busy {
            return Ok(g.status.clone());
        }
        g.status.checking = true;
        g.status.error = None;
    }
    emit(app, u.status());
    let result = async {
        let mut builder = app.updater_builder().timeout(Duration::from_secs(30));
        if let Some(feed) = &u.feed {
            builder = builder.endpoints(vec![feed.clone()])?;
        }
        builder.build()?.check().await
    }
    .await;
    let now = now_ms();
    let error = result
        .as_ref()
        .err()
        .map(|e| format!("Couldn't check for updates: {e}"));
    let status = set_status(app, &u, |g| {
        g.status.checking = false;
        g.status.error = error.clone();
        g.last_attempt = Some(now);
        if let Ok(found) = &result {
            g.status.last_checked = Some(now);
            let same = matches!((found, &g.pending), (Some(a), Some(b)) if a.version == b.version);
            // A different version (or none) replaces what was downloaded before.
            if !same {
                g.bytes = None;
                g.status.ready = false;
            }
            g.status.available = found.as_ref().map(info);
            g.pending = found.clone();
        }
    });
    u.save_stamp();
    match error {
        None => Ok(status),
        Some(e) => Err(EngineError::failed(e)),
    }
}

/// Records why an install step failed and returns it as the command's error.
fn fail(app: &AppHandle, u: &Updates, why: String, f: impl FnOnce(&mut Inner)) -> EngineError {
    set_status(app, u, |g| {
        f(g);
        g.status.error = Some(why.clone());
    });
    EngineError::failed(why)
}

/// Downloads (verifying the signature), installs and relaunches. Refused while
/// recording; a bundle downloaded meanwhile is kept and installs on the next call.
pub async fn install(app: &AppHandle) -> Result<UpdateStatus, EngineError> {
    let u = updates(app)?;
    let shell = app
        .try_state::<Arc<Shell>>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| EngineError::failed("Cortex is still starting"))?;
    if u.lock().pending.is_none() {
        check(app).await?;
    }
    let (update, bytes) = {
        let mut g = u.lock();
        if g.status.download.is_some() || g.status.installing {
            return Ok(g.status.clone());
        }
        let Some(update) = g.pending.clone() else {
            return Err(EngineError::failed("Cortex is up to date"));
        };
        g.status.error = None;
        (update, g.bytes.clone())
    };
    // Refuse before downloading when a recording is already running.
    if let Some(why) = recording_blocker(&shell) {
        return Err(fail(app, &u, why.into(), |_| {}));
    }
    let bytes = match bytes {
        Some(b) => b,
        None => {
            set_status(app, &u, |g| {
                g.status.download = Some(DownloadProgress {
                    downloaded: 0.0,
                    total: None,
                });
            });
            let mut downloaded = 0usize;
            let mut last = Instant::now();
            let app2 = app.clone();
            let u2 = u.clone();
            let result = update
                .download(
                    |chunk, total| {
                        downloaded += chunk;
                        if last.elapsed() >= PROGRESS_EVERY {
                            last = Instant::now();
                            let d = downloaded as f64;
                            set_status(&app2, &u2, |g| {
                                g.status.download = Some(DownloadProgress {
                                    downloaded: d,
                                    total: total.map(|t| t as f64),
                                });
                            });
                        }
                    },
                    || {},
                )
                .await;
            match result {
                Ok(b) => {
                    let b = Arc::new(b);
                    set_status(app, &u, |g| {
                        g.status.download = None;
                        g.status.ready = true;
                        g.bytes = Some(b.clone());
                    });
                    b
                }
                Err(e) => {
                    let why = format!("The update could not be downloaded or verified: {e}");
                    return Err(fail(app, &u, why, |g| g.status.download = None));
                }
            }
        }
    };
    // A recording may have started during the download: keep the bundle for later.
    if let Err(e) = shell.begin_update() {
        return Err(fail(app, &u, e.to_string(), |_| {}));
    }
    set_status(app, &u, |g| g.status.installing = true);
    // `install` verifies the signature again before touching the app.
    if let Err(e) = update.install(bytes.as_slice()) {
        shell.end_update();
        let why = format!("The update could not be installed: {e}");
        return Err(fail(app, &u, why, |g| g.status.installing = false));
    }
    println!("cortex: installed Cortex {}", update.version);
    u.lock().relaunch = true;
    // The quit flow finalizes and flushes settings; `relaunch_after_exit` reopens us.
    app.exit(0);
    Ok(u.status())
}

fn recording_blocker(shell: &Shell) -> Option<&'static str> {
    let st = shell.state();
    crate::shell::update_blocker(st.status, st.live)
}

/// After an installed update, from `RunEvent::Exit`: open the new app as a new process.
/// Like Tauri's own restart it goes through LaunchServices (`open -n`), so launchd owns
/// the new process. Unlike it, the `CORTEX_DEBUG_*` variables are passed on, so a test
/// copy keeps its scratch settings folder instead of reading the user's.
pub fn relaunch_after_exit(app: &AppHandle) {
    let wanted = app
        .try_state::<Arc<Updates>>()
        .is_some_and(|u| u.lock().relaunch);
    if !wanted {
        return;
    }
    let Some(bundle) = std::env::current_exe()
        .ok()
        .and_then(|exe| app_bundle(&exe))
    else {
        eprintln!("cortex: not running from an app bundle; reopen Cortex to finish the update");
        return;
    };
    let mut cmd = std::process::Command::new("/usr/bin/open");
    cmd.arg("-n").arg(&bundle);
    for (k, v) in std::env::vars() {
        if k.starts_with("CORTEX_DEBUG_") {
            cmd.arg("--env").arg(format!("{k}={v}"));
        }
    }
    match cmd.stdin(std::process::Stdio::null()).output() {
        Ok(o) if o.status.success() => {}
        Ok(o) => eprintln!(
            "cortex: could not reopen after the update: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => eprintln!("cortex: could not reopen after the update: {e}"),
    }
}

/// `…/Cortex.app` for `…/Cortex.app/Contents/MacOS/<binary>`.
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let named = |p: &Path, n: &str| p.file_name().is_some_and(|f| f == n);
    let is_app = bundle.extension().is_some_and(|e| e == "app");
    (named(macos, "MacOS") && named(contents, "Contents") && is_app).then(|| bundle.to_path_buf())
}

/// The version, the last check and any update found or downloading.
#[tauri::command]
#[specta::specta]
pub fn update_status(updates: State<'_, Arc<Updates>>) -> UpdateStatus {
    updates.status()
}

/// Check the update feed now. Progress and the result also arrive as `updater://status`.
#[tauri::command]
#[specta::specta]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateStatus, EngineError> {
    check(&app).await
}

/// Download, verify and install the announced update, then relaunch. Refused while
/// recording, counting down, saving or live.
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> Result<UpdateStatus, EngineError> {
    install(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_debug_feed_must_be_on_this_mac() {
        let ok = |s: &str| debug_feed(Some(s)).is_some();
        assert!(ok("http://127.0.0.1:8765/latest.json"));
        assert!(ok("https://localhost/updates/latest.json"));
        assert!(ok("http://[::1]:9000/latest.json"));
        assert!(!ok("https://cortex.example/updates/latest.json"));
        assert!(!ok("http://127.0.0.1.evil.example/latest.json"));
        assert!(!ok("http://localhost@evil.example/latest.json"));
        assert!(!ok("file:///tmp/latest.json"));
        assert!(!ok("not a url"));
        assert!(debug_feed(None).is_none());
    }

    #[test]
    fn quiet_checks_run_at_most_once_a_day() {
        let day = CHECK_EVERY_MS;
        assert!(check_due(None, 1000.0));
        assert!(!check_due(Some(1000.0), 1000.0 + day - 1.0));
        assert!(check_due(Some(1000.0), 1000.0 + day));
        // A clock set back does not stop checks for good.
        assert!(check_due(Some(5.0 * day), day));
    }

    #[test]
    fn only_app_bundles_relaunch() {
        let p = |s: &str| app_bundle(Path::new(s));
        assert_eq!(
            p("/tmp/x/Cortex.app/Contents/MacOS/cortex-desktop"),
            Some(PathBuf::from("/tmp/x/Cortex.app"))
        );
        assert_eq!(p("/repo/target/release/cortex-desktop"), None);
        assert_eq!(p("/tmp/Cortex/Contents/MacOS/cortex-desktop"), None);
    }

    /// One version string everywhere: `scripts/desktop/bump-version.sh` keeps these equal.
    #[test]
    fn the_version_matches_in_every_manifest() {
        let dir = env!("CARGO_MANIFEST_DIR");
        let version = |path: &str| -> String {
            let text = std::fs::read_to_string(format!("{dir}/{path}")).unwrap();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            json["version"].as_str().unwrap().to_string()
        };
        let cargo = env!("CARGO_PKG_VERSION");
        assert_eq!(version("tauri.conf.json"), cargo);
        assert_eq!(version("../package.json"), cargo);
    }

    #[test]
    fn the_feed_and_key_are_configured() {
        let dir = env!("CARGO_MANIFEST_DIR");
        let text = std::fs::read_to_string(format!("{dir}/tauri.conf.json")).unwrap();
        let conf: serde_json::Value = serde_json::from_str(&text).unwrap();
        let updater = &conf["plugins"]["updater"];
        assert!(updater["pubkey"].as_str().is_some_and(|k| k.len() > 40));
        let endpoints = updater["endpoints"].as_array().unwrap();
        assert!(!endpoints.is_empty());
        for e in endpoints {
            assert!(e.as_str().unwrap().starts_with("https://"), "{e}");
        }
        // Plain HTTP is only for local test builds, set from the release script.
        assert_ne!(updater["dangerousInsecureTransportProtocol"], true);
        assert_eq!(updater["requireSignedVersion"], true);
    }
}
