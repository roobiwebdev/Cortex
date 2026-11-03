//! SOURCE OF TRUTH KEYWORDS: Shell, Outbound, Forwarder, settings store, DesktopEngine, open_system_settings, reveal_recording
//! WHAT:  The desktop app's recorder core: the platform's engine (`platform::open_engine`,
//!        `MacEngine` on macOS), the persisted settings and the event forwarding (throttled
//!        state/levels, finished recordings, errors).
//! WHY:   Tauri commands in `ipc.rs` are one-line wrappers over these methods, so tests can
//!        drive exactly what the webview drives without a window (see `tests/record.rs`).
//!        Nothing here knows about Tauri: events leave through the `Emit` callback.
//! WHERE: `lib.rs` builds it in `setup`; `ipc.rs` exposes it; the record integration test.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rec_core::api::{EngineError, EngineEvent};
use rec_core::platform::EngineOptions;
use rec_core::settings::{RecorderSettings, StreamDest};
use rec_core::sources::prompts_before_start;
use rec_core::state::{
    Devices, EngineState, Levels, Permission, PermissionReport, PermissionState, Permissions,
    Recording,
};
use rec_core::store::{self, DEFAULT_DEBOUNCE, SettingsStore};

use crate::platform::{self, DesktopEngine};

/// At most 10 state events a second from progress (status changes always go out).
const STATE_INTERVAL: Duration = Duration::from_millis(95);
/// At most 30 level events a second.
const LEVELS_INTERVAL: Duration = Duration::from_millis(30);

/// What the shell tells the webview. `ipc.rs` maps each to a typed Tauri event.
#[derive(Clone, Debug, PartialEq)]
pub enum Outbound {
    State(EngineState),
    Levels(Levels),
    RecordingFinished(Recording),
    ReplaySaved(String),
    Error { message: String, recoverable: bool },
    Settings(RecorderSettings),
}

/// Where outbound events go (any thread; keep it quick).
pub type Emit = Arc<dyn Fn(Outbound) + Send + Sync>;

struct Cache {
    state: EngineState,
    last_state: Option<Instant>,
    last_levels: Option<Instant>,
}

fn due(last: Option<Instant>, every: Duration) -> bool {
    last.is_none_or(|t| t.elapsed() >= every)
}

/// Turns engine events into outbound events: merges progress, devices and
/// permissions into a cached `EngineState` and rate-limits state and levels.
struct Forwarder {
    emit: Emit,
    cache: Mutex<Cache>,
}

impl Forwarder {
    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn on_event(&self, e: EngineEvent) {
        let out = {
            let mut c = self.cache();
            let now = Instant::now();
            let state = |c: &mut Cache| {
                c.last_state = Some(now);
                Some(Outbound::State(c.state.clone()))
            };
            match e {
                EngineEvent::StateChanged(s) => {
                    c.state = s;
                    state(&mut c)
                }
                EngineEvent::Progress(p) => {
                    let countdown_moved = p.countdown_left != c.state.countdown_left;
                    c.state.set_progress(&p);
                    if countdown_moved || due(c.last_state, STATE_INTERVAL) {
                        state(&mut c)
                    } else {
                        None
                    }
                }
                EngineEvent::Levels(l) => {
                    c.state.levels = l;
                    due(c.last_levels, LEVELS_INTERVAL).then(|| {
                        c.last_levels = Some(now);
                        Outbound::Levels(l)
                    })
                }
                EngineEvent::DeviceChanged(d) => {
                    c.state.set_devices(d);
                    state(&mut c)
                }
                EngineEvent::PermissionChanged {
                    permission,
                    state: p,
                } => {
                    c.state.permissions.set(permission, p);
                    state(&mut c)
                }
                EngineEvent::RecordingFinished { recording } => {
                    Some(Outbound::RecordingFinished(recording))
                }
                EngineEvent::ReplaySaved { path } => Some(Outbound::ReplaySaved(path)),
                EngineEvent::Error {
                    message,
                    recoverable,
                } => Some(Outbound::Error {
                    message,
                    recoverable,
                }),
            }
        };
        if let Some(o) = out {
            (self.emit)(o);
        }
    }
}

/// The recorder as the app sees it.
pub struct Shell {
    engine: Box<dyn DesktopEngine>,
    store: SettingsStore,
    settings: Mutex<RecorderSettings>,
    forwarder: Arc<Forwarder>,
    home: PathBuf,
    /// True while an update installs: nothing may start recording or go live meanwhile.
    /// Held while starting, so a start and an install never pass their checks together.
    update_gate: Mutex<bool>,
    /// Set when the settings file was damaged and defaults were loaded.
    pub load_error: Option<String>,
}

/// Why an update can't install now, or None. Installing replaces the app and relaunches
/// it, which would cut a recording, a countdown, a file being finalized or a live stream
/// short.
pub fn update_blocker(status: rec_core::EngineStatus, live: bool) -> Option<&'static str> {
    use rec_core::EngineStatus::{Countdown, Finalizing, Idle, Paused, Recording};
    match status {
        Recording | Paused => Some("Stop the recording before installing the update"),
        Countdown => Some("Cancel the countdown before installing the update"),
        Finalizing => Some("Wait until the recording is saved, then install the update"),
        Idle if live => Some("Stop streaming before installing the update"),
        Idle => None,
    }
}

const UPDATING: &str = "Cortex is installing an update and will reopen in a moment";

impl Shell {
    /// Load settings from `settings_path`, start the engine, and send events to `emit`.
    pub fn open(opts: EngineOptions, settings_path: PathBuf, emit: Emit) -> Shell {
        let on_error = {
            let emit = emit.clone();
            Box::new(move |message: String| {
                emit(Outbound::Error {
                    message,
                    recoverable: true,
                })
            })
        };
        let (store, loaded) = SettingsStore::open(settings_path, DEFAULT_DEBOUNCE, Some(on_error));
        let forwarder = Arc::new(Forwarder {
            emit,
            cache: Mutex::new(Cache {
                state: EngineState::default(),
                last_state: None,
                last_levels: None,
            }),
        });
        let sink = {
            let f = forwarder.clone();
            Arc::new(move |e: EngineEvent| f.on_event(e))
        };
        let home = opts.home.clone();
        let engine = platform::open_engine(opts, loaded.settings.clone(), sink);
        let settings = engine.with_key_status(loaded.settings);
        forwarder.cache().state = engine.state();
        Shell {
            engine,
            store,
            settings: Mutex::new(settings),
            forwarder,
            home,
            update_gate: Mutex::new(false),
            load_error: loaded.error,
        }
    }

    /// The platform engine, for things outside the shell's own methods (preview, devices).
    pub fn engine(&self) -> &dyn DesktopEngine {
        self.engine.as_ref()
    }

    fn settings_lock(&self) -> MutexGuard<'_, RecorderSettings> {
        self.settings.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn settings(&self) -> RecorderSettings {
        self.settings_lock().clone()
    }

    /// Store, persist and announce `s` (already validated, key status filled in).
    fn commit_settings(&self, s: RecorderSettings) {
        *self.settings_lock() = s.clone();
        self.store.save(&s);
        (self.forwarder.emit)(Outbound::Settings(s));
    }

    pub fn state(&self) -> EngineState {
        let s = self.engine.state();
        self.forwarder.cache().state = s.clone();
        s
    }

    /// Start with the stored settings (the engine runs the countdown). A microphone or
    /// camera macOS has never asked about is asked for first (this waits for the answer);
    /// one that stays missing is left out of the recording, with a warning.
    pub fn start(&self) -> Result<(), EngineError> {
        let s = self.settings();
        let perms = self.permissions();
        for p in prompts_before_start(s.scene, s.mic_enabled, &perms) {
            self.engine.request_permission(p);
        }
        let gate = self.gate();
        if *gate {
            return Err(EngineError::failed(UPDATING));
        }
        self.engine.start(&s)
    }

    fn gate(&self) -> MutexGuard<'_, bool> {
        self.update_gate.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Claims the app for an update install: refused while recording, counting down,
    /// finalizing or live (`update_blocker`). Until `end_update`, `start` and `go_live`
    /// are refused, so nothing can begin between this check and the relaunch.
    pub fn begin_update(&self) -> Result<(), EngineError> {
        let mut gate = self.gate();
        if *gate {
            return Err(EngineError::failed(UPDATING));
        }
        let st = self.engine.state();
        if let Some(why) = update_blocker(st.status, st.live) {
            return Err(EngineError::failed(why));
        }
        *gate = true;
        Ok(())
    }

    /// The install failed: recording may start again.
    pub fn end_update(&self) {
        *self.gate() = false;
    }

    /// The privacy grants as macOS reports them to this process now.
    pub fn permissions(&self) -> Permissions {
        let mut p = Permissions::default();
        for perm in Permission::ALL {
            p.set(perm, self.engine.permission(perm));
        }
        p
    }

    /// The grants, plus whether a Screen Recording grant needs a relaunch to apply: this
    /// process still sees no grant, but a new process (the same app) does.
    pub fn permission_report(&self) -> PermissionReport {
        let permissions = self.permissions();
        let screen_needs_relaunch =
            permissions.screen != PermissionState::Granted && screen_granted_in_new_process();
        PermissionReport {
            permissions,
            screen_needs_relaunch,
        }
    }

    /// Relaunching would cut a recording short.
    pub fn can_relaunch(&self) -> Result<(), EngineError> {
        if self.engine.state().status == rec_core::EngineStatus::Idle {
            Ok(())
        } else {
            Err(EngineError::failed(
                "Stop the recording before reopening Cortex",
            ))
        }
    }

    pub fn cancel_countdown(&self) -> Result<(), EngineError> {
        self.engine.cancel_countdown()
    }

    pub fn stop(&self) -> Result<(), EngineError> {
        self.engine.stop()
    }

    pub fn pause(&self) -> Result<(), EngineError> {
        self.engine.pause()
    }

    pub fn resume(&self) -> Result<(), EngineError> {
        self.engine.resume()
    }

    pub fn discard(&self) -> Result<(), EngineError> {
        self.engine.discard()
    }

    pub fn save_replay(&self) -> Result<(), EngineError> {
        self.engine.save_replay()
    }

    pub fn go_live(&self) -> Result<(), EngineError> {
        let gate = self.gate();
        if *gate {
            return Err(EngineError::failed(UPDATING));
        }
        self.engine.go_live()
    }

    pub fn stop_live(&self) -> Result<(), EngineError> {
        self.engine.stop_live()
    }

    /// Validate, persist and apply new settings; returns what was stored.
    /// The settings are kept even when the engine can't use them yet
    /// (e.g. app capture): the error tells the UI why.
    pub fn update_settings(
        &self,
        settings: RecorderSettings,
    ) -> Result<RecorderSettings, EngineError> {
        let mut s = settings.validated();
        // `keyStored` is the engine's to say, not the webview's. Keep the known
        // answers and only ask the Keychain about new destinations.
        let known = self.settings_lock().stream_dests.clone();
        let mut unknown = false;
        for d in &mut s.stream_dests {
            match known
                .iter()
                .find(|k| k.service == d.service && k.url == d.url)
            {
                Some(k) => d.key_stored = k.key_stored,
                None => unknown = true,
            }
        }
        if unknown {
            s = self.engine.with_key_status(s);
        }
        self.commit_settings(s.clone());
        self.engine.update_settings(&s)?;
        Ok(s)
    }

    pub fn devices(&self) -> Devices {
        self.engine.devices()
    }

    /// Ask for a permission; blocks until answered or 60 s pass.
    pub fn request_permission(&self, p: Permission) -> PermissionState {
        self.engine.request_permission(p)
    }

    /// Store a stream key in the Keychain; the key never comes back.
    pub fn set_stream_key(
        &self,
        dest: StreamDest,
        key: String,
    ) -> Result<RecorderSettings, EngineError> {
        self.engine.set_stream_key(&dest, &key)?;
        Ok(self.refresh_key_status())
    }

    pub fn remove_stream_key(&self, dest: StreamDest) -> RecorderSettings {
        self.engine.remove_stream_key(&dest);
        self.refresh_key_status()
    }

    fn refresh_key_status(&self) -> RecorderSettings {
        let s = self.engine.with_key_status(self.settings());
        self.commit_settings(s.clone());
        s
    }

    /// Import Rec's settings.json (if the user had Rec) as the current settings.
    pub fn import_rec_settings(&self) -> Result<RecorderSettings, EngineError> {
        let path = self
            .home
            .join("Library/Application Support/rec-rs/settings.json");
        let s = store::import_rec_settings(&path).map_err(EngineError::failed)?;
        self.update_settings(s)
    }

    pub fn output_dir(&self) -> PathBuf {
        self.settings().output_dir_in(&self.home)
    }

    pub fn open_recordings_folder(&self) -> Result<(), EngineError> {
        let dir = self.output_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| EngineError::failed(format!("Could not create {}: {e}", dir.display())))?;
        platform::system().open_folder(&dir)
    }

    /// Show a recording in Finder. Only files inside the output folder.
    pub fn reveal_recording(&self, path: &str) -> Result<(), EngineError> {
        let file = inside(&self.output_dir(), Path::new(path))?;
        platform::system().reveal(&file)
    }

    /// Flush settings and finalize any recording (quit).
    pub fn shutdown(&self) {
        self.engine.shutdown();
        if let Err(e) = self.store.flush() {
            eprintln!("cortex: could not save settings: {e}");
        }
    }
}

/// The argument that makes the app binary print nothing, check Screen Recording and exit
/// (0 = granted). See `lib::screen_preflight_child`.
pub const SCREEN_PREFLIGHT_ARG: &str = "--cortex-screen-preflight";

/// Ask a new process of this app whether Screen Recording is granted. macOS answers a
/// running process from what it saw at launch, so this is how the app learns that a grant
/// made in System Settings is waiting for a relaunch.
fn screen_granted_in_new_process() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    Command::new(exe)
        .arg(SCREEN_PREFLIGHT_ARG)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `path` canonicalized, if it is an existing file under `dir`.
fn inside(dir: &Path, path: &Path) -> Result<PathBuf, EngineError> {
    let not_found = || EngineError::failed("That recording is no longer in the recordings folder");
    let dir = dir.canonicalize().map_err(|_| not_found())?;
    let file = path.canonicalize().map_err(|_| not_found())?;
    if file.starts_with(&dir) && file.is_file() {
        Ok(file)
    } else {
        Err(not_found())
    }
}

/// Open the system settings page where `p` is granted (System Settings' privacy pane on macOS).
pub fn open_system_settings(p: Permission) -> Result<(), EngineError> {
    platform::system().open_privacy_settings(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rec_core::state::Progress;

    #[test]
    fn updates_wait_for_recordings_and_streams() {
        use rec_core::EngineStatus::*;
        assert_eq!(update_blocker(Idle, false), None);
        for status in [Countdown, Recording, Paused, Finalizing] {
            assert!(update_blocker(status, false).is_some(), "{status:?}");
            assert!(update_blocker(status, true).is_some(), "{status:?} live");
        }
        assert!(update_blocker(Idle, true).is_some(), "live while idle");
    }

    fn forwarder() -> (Forwarder, Arc<Mutex<Vec<Outbound>>>) {
        let got = Arc::new(Mutex::new(vec![]));
        let g = got.clone();
        let f = Forwarder {
            emit: Arc::new(move |o| g.lock().unwrap().push(o)),
            cache: Mutex::new(Cache {
                state: EngineState::default(),
                last_state: None,
                last_levels: None,
            }),
        };
        (f, got)
    }

    #[test]
    fn progress_and_levels_are_rate_limited() {
        let (f, got) = forwarder();
        for i in 0..20 {
            f.on_event(EngineEvent::Progress(Progress {
                elapsed_secs: f64::from(i),
                ..Default::default()
            }));
            f.on_event(EngineEvent::Levels(Levels::default()));
        }
        let got = got.lock().unwrap();
        let states = got
            .iter()
            .filter(|o| matches!(o, Outbound::State(_)))
            .count();
        let levels = got
            .iter()
            .filter(|o| matches!(o, Outbound::Levels(_)))
            .count();
        assert_eq!((states, levels), (1, 1), "a burst sends one of each");
        assert_eq!(
            f.cache().state.elapsed_secs,
            19.0,
            "the cache keeps the latest"
        );
    }

    #[test]
    fn status_changes_and_countdown_ticks_always_go_out() {
        let (f, got) = forwarder();
        for left in [3, 2, 1] {
            f.on_event(EngineEvent::Progress(Progress {
                countdown_left: left,
                ..Default::default()
            }));
            f.on_event(EngineEvent::StateChanged(EngineState::default()));
        }
        assert_eq!(got.lock().unwrap().len(), 6);
    }

    #[test]
    fn reveal_only_inside_the_output_folder() {
        let dir = std::env::temp_dir().join(format!("cortex-inside-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.mov");
        std::fs::write(&file, b"x").unwrap();
        assert!(inside(&dir, &file).is_ok());
        assert!(inside(&dir, &dir.join("../a.mov")).is_err());
        assert!(inside(&dir, Path::new("/etc/hosts")).is_err());
        assert!(inside(&dir, &dir).is_err(), "folders aren't recordings");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
