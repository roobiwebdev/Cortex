//! SOURCE OF TRUTH KEYWORDS: Engine, Backend, Session, SessionRequest, SessionInfo, WindowsEngine, RecorderEngine, ticker, countdown, finalize
//! WHAT:  The Windows recorder's state machine and event flow, generic over a `Backend`
//!        that starts capture sessions. It implements `rec_core::api::RecorderEngine`:
//!        countdown, record, pause/resume, stop, discard, levels, progress, permissions,
//!        devices, stream keys, and `RecordingFinished` with a `Recording`.
//! WHY:   The behaviour must match `MacEngine` (same transitions through
//!        `rec_core::api::transition`, same events, same 30 Hz ticker, same "send events
//!        after the lock is released" rule, same bounded shutdown), and it must be testable
//!        on a Mac, where Windows APIs can't run. So the OS work sits behind `Backend` and
//!        `Session`; the real ones are in `win/`, the tests use a fake.
//! WHERE: `WindowsEngine` (= `Engine<win::WinBackend>`) in the Tauri shell's
//!        `platform/windows.rs`; the unit tests below.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rec_core::api::{Action, EngineError, EngineEvent, EventSink, RecorderEngine, transition};
use rec_core::library::{LocalTime, timestamped_name};
use rec_core::platform::{EngineOptions, PlatformCapabilities, Secrets};
use rec_core::settings::{RecorderSettings, StreamDest};
use rec_core::sources::{SourcePlan, dropped_note, plan_sources};
use rec_core::state::{
    Devices, EngineState, EngineStatus, Levels, Permission, PermissionState, Permissions, Progress,
    Recording,
};

use crate::caps::dest_url;
use crate::plan::FILE_EXT;

/// What a capture session needs to start.
#[derive(Clone, Debug)]
pub struct SessionRequest {
    pub file: PathBuf,
    pub settings: RecorderSettings,
    /// The sources the grants allow (`rec_core::sources::plan_sources`).
    pub plan: SourcePlan,
    pub test_pattern: bool,
    /// Keep this app's windows out of the capture (`EngineOptions::require_self_exclusion`).
    pub exclude_self: bool,
    /// Stop writing after this many recorded seconds (self-tests).
    pub max_duration_secs: Option<f64>,
}

/// What a started session records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub audio_tracks: u32,
}

/// A started session and the notes for the user (a fallback was taken).
pub struct Started {
    pub session: Box<dyn Session>,
    pub info: SessionInfo,
    pub notes: Vec<String>,
}

/// How a finished file came out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// Video frames written (including repeats).
    pub frames: u64,
}

/// A running recording. Calls come from the engine's lock; `finish` runs on its own thread.
pub trait Session: Send {
    /// The first frame arrived (the timeline has started).
    fn has_frames(&self) -> bool;
    /// Seconds on the output timeline (pauses excluded).
    fn recorded_secs(&self) -> f64;
    fn dropped_frames(&self) -> u64;
    /// Peak [system, mic] levels since the last call (0..1).
    fn take_levels(&self) -> [f32; 2];
    fn pause(&self) -> bool;
    fn resume(&self) -> bool;
    /// Why a source or the writer stopped, once one has.
    fn error(&self) -> Option<String>;
    /// Faders, mutes and the camera overlay apply live.
    fn update(&self, settings: &RecorderSettings);
    /// Stop and write the file.
    fn finish(self: Box<Self>) -> Result<SessionStats, String>;
}

/// The operating system side of the engine.
pub trait Backend: Send + Sync + 'static {
    fn capabilities(&self) -> PlatformCapabilities;
    /// Every display, window, app, camera and mic (may take tens of milliseconds).
    fn devices(&self) -> Devices;
    /// The privacy grants as the OS reports them.
    fn permissions(&self) -> Permissions;
    fn start(&self, req: &SessionRequest) -> Result<Started, EngineError>;
    /// Free bytes on the volume holding `dir`.
    fn free_disk_bytes(&self, dir: &Path) -> u64;
    fn local_time(&self) -> LocalTime;
    fn secrets(&self) -> &dyn Secrets;
}

/// How often the device lists are re-read while nothing records (ticks at 30 Hz): Windows
/// has no single notification for monitors, windows, cameras and mics together.
const DEVICE_POLL_TICKS: u64 = 150;
/// How long `shutdown` waits for the ticker thread.
const TICKER_STOP_WAIT: Duration = Duration::from_millis(500);

struct Active {
    sess: Box<dyn Session>,
    info: SessionInfo,
    file: PathBuf,
    created_ms: f64,
}

struct Finishing {
    handle: std::thread::JoinHandle<Result<SessionStats, String>>,
    info: SessionInfo,
    file: PathBuf,
    created_ms: f64,
    discard: bool,
}

struct Inner {
    opts: EngineOptions,
    settings: RecorderSettings,
    state: EngineState,
    countdown_until: Option<Instant>,
    recording: Option<Active>,
    finishing: Option<Finishing>,
    ticks: u64,
    last_levels: Instant,
}

type Events = Vec<EngineEvent>;

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Grants as the UI should see them: the test pattern needs no screen grant, withheld ones
/// read as never asked (self-tests).
fn state_permissions(opts: &EngineOptions, os: Permissions) -> Permissions {
    let mut p = os;
    if opts.test_pattern {
        p.screen = PermissionState::Granted;
    }
    for w in &opts.withheld {
        p.set(*w, PermissionState::NotDetermined);
    }
    p
}

impl Inner {
    fn set_status(&mut self, s: EngineStatus, ev: &mut Events) {
        self.state.status = s;
        if s == EngineStatus::Idle {
            self.state.countdown_left = 0;
        }
        ev.push(EngineEvent::StateChanged(self.state.clone()));
    }

    fn act(&self, a: Action) -> Result<EngineStatus, EngineError> {
        transition(self.state.status, a)
    }

    fn begin_recording<B: Backend>(&mut self, b: &B, ev: &mut Events) -> Result<(), EngineError> {
        let started = (|| {
            let plan = plan_sources(
                self.settings.scene,
                self.settings.mic_enabled,
                &self.state.permissions,
                self.opts.test_pattern,
            )
            .map_err(|permissions| EngineError::MissingPermissions { permissions })?;
            let dir = self.settings.output_dir_in(&self.opts.home);
            std::fs::create_dir_all(&dir).map_err(|e| {
                EngineError::failed(format!("Could not create {}: {e}", dir.display()))
            })?;
            let file = dir.join(timestamped_name("rec", FILE_EXT, b.local_time()));
            let req = SessionRequest {
                file: file.clone(),
                settings: self.settings.clone(),
                plan: plan.clone(),
                test_pattern: self.opts.test_pattern,
                exclude_self: self.opts.require_self_exclusion,
                max_duration_secs: self.opts.max_duration_secs,
            };
            let s = b.start(&req)?;
            if let Some(note) = dropped_note(&plan.dropped) {
                ev.push(EngineEvent::Error {
                    message: note,
                    recoverable: true,
                });
            }
            for note in &s.notes {
                ev.push(EngineEvent::Error {
                    message: note.clone(),
                    recoverable: true,
                });
            }
            Ok::<_, EngineError>(Active {
                sess: s.session,
                info: s.info,
                file,
                created_ms: now_ms(),
            })
        })();
        self.countdown_until = None;
        match started {
            Ok(a) => {
                self.recording = Some(a);
                self.state.set_progress(&Progress::default());
                self.set_status(EngineStatus::Recording, ev);
                Ok(())
            }
            Err(e) => {
                self.set_status(EngineStatus::Idle, ev);
                Err(e)
            }
        }
    }

    /// Stop the recording; the file is written on a background thread.
    fn begin_finalize(&mut self, discard: bool, ev: &mut Events) {
        let Some(a) = self.recording.take() else {
            return;
        };
        let sess = a.sess;
        let handle = std::thread::Builder::new()
            .name("win-finalize".into())
            .spawn(move || sess.finish());
        let handle = match handle {
            Ok(h) => h,
            Err(e) => {
                // No thread for it: report and leave whatever was written.
                ev.push(EngineEvent::Error {
                    message: format!("Recording failed: could not finish the file ({e})"),
                    recoverable: false,
                });
                if let Ok(s) = self.act(Action::Finalized) {
                    self.set_status(s, ev);
                }
                return;
            }
        };
        self.finishing = Some(Finishing {
            handle,
            info: a.info,
            file: a.file,
            created_ms: a.created_ms,
            discard,
        });
        self.set_status(EngineStatus::Finalizing, ev);
    }

    fn finish_done(&mut self, ev: &mut Events) {
        if !self
            .finishing
            .as_ref()
            .is_some_and(|f| f.handle.is_finished())
        {
            return;
        }
        let Some(f) = self.finishing.take() else {
            return;
        };
        let result = f
            .handle
            .join()
            .unwrap_or_else(|_| Err("the writer crashed".into()));
        if f.discard {
            let _ = std::fs::remove_file(&f.file);
        } else {
            match result {
                Ok(stats) => {
                    let name = f
                        .file
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let r = Recording {
                        id: name.clone(),
                        name,
                        path: f.file.display().to_string(),
                        duration_secs: stats.frames as f64 / f.info.fps.max(1) as f64,
                        bytes: std::fs::metadata(&f.file).map(|m| m.len()).unwrap_or(0),
                        created_at: f.created_ms,
                        width: f.info.width,
                        height: f.info.height,
                        has_camera_track: false,
                        fps: f.info.fps,
                        audio_tracks: f.info.audio_tracks,
                    };
                    self.state.recordings.insert(0, r.clone());
                    ev.push(EngineEvent::RecordingFinished { recording: r });
                }
                Err(e) => ev.push(EngineEvent::Error {
                    message: format!("Recording failed: {e}"),
                    recoverable: false,
                }),
            }
        }
        if let Ok(s) = self.act(Action::Finalized) {
            self.set_status(s, ev);
        }
    }

    fn progress(&self) -> Progress {
        let mut p = Progress {
            countdown_left: self.state.countdown_left,
            ..Progress::default()
        };
        if let Some(a) = &self.recording {
            p.elapsed_secs = a.sess.recorded_secs();
            p.dropped_frames = a.sess.dropped_frames();
            p.bytes_written = std::fs::metadata(&a.file).map(|m| m.len()).unwrap_or(0);
        }
        p
    }

    fn tick<B: Backend>(&mut self, b: &B, ev: &mut Events) {
        self.ticks += 1;
        if self.state.status == EngineStatus::Countdown
            && let Some(until) = self.countdown_until
        {
            let left = until
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u32;
            if left == 0 {
                if let Err(e) = self.begin_recording(b, ev) {
                    ev.push(EngineEvent::Error {
                        message: format!("Could not start recording: {e}"),
                        recoverable: false,
                    });
                }
            } else if left != self.state.countdown_left {
                self.state.countdown_left = left;
                ev.push(EngineEvent::Progress(self.progress()));
            }
        }
        if let Some(err) = self.recording.as_ref().and_then(|a| a.sess.error()) {
            ev.push(EngineEvent::Error {
                message: format!("Recording stopped: {err}"),
                recoverable: false,
            });
            if self.act(Action::Fail).is_ok() {
                self.begin_finalize(false, ev);
            }
        }
        if let (Some(max), Some(a)) = (self.opts.max_duration_secs, &self.recording)
            && a.sess.has_frames()
            && a.sess.recorded_secs() >= max
            && self.act(Action::Stop).is_ok()
        {
            self.begin_finalize(false, ev);
        }
        self.finish_done(ev);
        let dt = self.last_levels.elapsed().as_secs_f32();
        self.last_levels = Instant::now();
        let lv = self.recording.as_ref().map(|a| a.sess.take_levels());
        if lv.is_some() || self.state.levels != Levels::default() {
            self.state.levels.feed(lv.unwrap_or([0.0; 2]), dt);
            ev.push(EngineEvent::Levels(self.state.levels));
        }
        if self.ticks.is_multiple_of(3) && self.state.status.is_capturing() {
            let p = self.progress();
            self.state.set_progress(&p);
            ev.push(EngineEvent::Progress(p));
        }
        if self.ticks.is_multiple_of(30) {
            let p = state_permissions(&self.opts, b.permissions());
            for perm in Permission::ALL {
                if p.get(perm) != self.state.permissions.get(perm) {
                    ev.push(EngineEvent::PermissionChanged {
                        permission: perm,
                        state: p.get(perm),
                    });
                }
            }
            if p != self.state.permissions {
                self.state.permissions = p;
                ev.push(EngineEvent::StateChanged(self.state.clone()));
            }
            self.state.free_disk_bytes =
                b.free_disk_bytes(&self.settings.output_dir_in(&self.opts.home));
        }
    }
}

struct Core<B: Backend> {
    inner: Mutex<Inner>,
    events: EventSink,
    backend: B,
    devices_changed: AtomicBool,
    quit: AtomicBool,
}

impl<B: Backend> Core<B> {
    /// Run `f` under the lock, then send the events it produced.
    fn run<R>(&self, f: impl FnOnce(&mut Inner, &mut Events) -> R) -> R {
        let mut ev = vec![];
        let r = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            f(&mut inner, &mut ev)
        };
        for e in ev {
            (self.events)(e);
        }
        r
    }

    fn tick(&self) {
        // Device lists are read outside the lock. While recording they are left alone:
        // listing windows and cameras costs time the capture threads can use.
        let (poll, capturing) = self.run(|i, _| {
            (
                i.ticks.is_multiple_of(DEVICE_POLL_TICKS),
                i.state.status != EngineStatus::Idle,
            )
        });
        if self.devices_changed.swap(false, Ordering::SeqCst) || (poll && !capturing) {
            let d = self.backend.devices();
            self.run(|i, ev| {
                let now = Devices {
                    displays: i.state.displays.clone(),
                    windows: i.state.windows.clone(),
                    apps: i.state.apps.clone(),
                    cameras: i.state.cameras.clone(),
                    mics: i.state.mics.clone(),
                };
                if !self.quit.load(Ordering::SeqCst) && now != d {
                    i.state.set_devices(d.clone());
                    ev.push(EngineEvent::DeviceChanged(d));
                }
            });
        }
        self.run(|i, ev| {
            if !self.quit.load(Ordering::SeqCst) {
                i.tick(&self.backend, ev);
            }
        });
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: Engine, WindowsEngine, RecorderEngine implementation, generic backend
 * WHAT:  A `RecorderEngine` over a `Backend` (Windows APIs in production, a fake in tests).
 * WHY:   One state machine for the Windows engine that runs in unit tests on the Mac.
 * WHERE: `rec_engine_windows::WindowsEngine`; the shell's `platform/windows.rs`.
 */
pub struct Engine<B: Backend> {
    core: Arc<Core<B>>,
    ticker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl<B: Backend> Engine<B> {
    pub fn new(
        backend: B,
        opts: EngineOptions,
        settings: RecorderSettings,
        events: EventSink,
    ) -> Self {
        let settings = settings.validated();
        let mut state = EngineState {
            permissions: state_permissions(&opts, backend.permissions()),
            capabilities: backend.capabilities(),
            ..EngineState::default()
        };
        state.free_disk_bytes = backend.free_disk_bytes(&settings.output_dir_in(&opts.home));
        let core = Arc::new(Core {
            inner: Mutex::new(Inner {
                opts,
                settings,
                state,
                countdown_until: None,
                recording: None,
                finishing: None,
                ticks: 0,
                last_levels: Instant::now(),
            }),
            events,
            backend,
            // The first tick lists the devices.
            devices_changed: AtomicBool::new(true),
            quit: AtomicBool::new(false),
        });
        let c = core.clone();
        let ticker = std::thread::Builder::new()
            .name("engine-ticker".into())
            .spawn(move || {
                while !c.quit.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(33));
                    c.tick();
                }
            })
            .ok();
        Engine {
            core,
            ticker: Mutex::new(ticker),
        }
    }

    pub fn backend(&self) -> &B {
        &self.core.backend
    }
}

impl<B: Backend> RecorderEngine for Engine<B> {
    fn state(&self) -> EngineState {
        self.core.run(|i, _| i.state.clone())
    }

    fn capabilities(&self) -> PlatformCapabilities {
        self.core.backend.capabilities()
    }

    fn start(&self, settings: &RecorderSettings) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            let settings = settings.clone().validated();
            let countdown = settings.countdown > 0;
            i.act(if countdown {
                Action::StartCountdown
            } else {
                Action::Start
            })?;
            i.settings = settings;
            if countdown {
                i.countdown_until =
                    Some(Instant::now() + Duration::from_secs(u64::from(i.settings.countdown)));
                i.state.countdown_left = i.settings.countdown;
                i.set_status(EngineStatus::Countdown, ev);
                Ok(())
            } else {
                i.begin_recording(&core.backend, ev)
            }
        })
    }

    fn cancel_countdown(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::CancelCountdown)?;
            i.countdown_until = None;
            i.set_status(s, ev);
            Ok(())
        })
    }

    fn stop(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            i.act(Action::Stop)?;
            i.begin_finalize(false, ev);
            Ok(())
        })
    }

    fn pause(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::Pause)?;
            // The timeline starts at the first frame; pausing waits for it.
            let ok = i
                .recording
                .as_ref()
                .is_some_and(|a| a.sess.has_frames() && a.sess.pause());
            if !ok {
                return Err(EngineError::failed("The recording hasn't started yet"));
            }
            i.set_status(s, ev);
            Ok(())
        })
    }

    fn resume(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::Resume)?;
            if !i.recording.as_ref().is_some_and(|a| a.sess.resume()) {
                return Err(EngineError::failed("Could not resume"));
            }
            i.set_status(s, ev);
            Ok(())
        })
    }

    fn discard(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::Discard)?;
            if s == EngineStatus::Idle {
                i.countdown_until = None;
                i.set_status(s, ev);
            } else {
                i.begin_finalize(true, ev);
            }
            Ok(())
        })
    }

    fn update_settings(&self, settings: &RecorderSettings) -> Result<(), EngineError> {
        self.core.run(|i, _| {
            let settings = settings.clone().validated();
            if let Some(a) = &i.recording {
                a.sess.update(&settings);
            }
            i.settings = settings;
            Ok(())
        })
    }

    fn set_preview_enabled(&self, _enabled: bool) -> Result<(), EngineError> {
        // No preview session on Windows yet (`capabilities.preview`); the page shows a
        // placeholder. Nothing to start or stop.
        Ok(())
    }

    fn save_replay(&self) -> Result<(), EngineError> {
        Err(EngineError::Unsupported {
            message: "The replay buffer is not available on Windows yet".into(),
        })
    }

    fn go_live(&self) -> Result<(), EngineError> {
        Err(EngineError::Unsupported {
            message: "Going live is not available on Windows yet".into(),
        })
    }

    fn stop_live(&self) -> Result<(), EngineError> {
        Ok(())
    }

    fn devices(&self) -> Devices {
        let d = self.core.backend.devices();
        self.core.run(|i, ev| {
            i.state.set_devices(d.clone());
            ev.push(EngineEvent::DeviceChanged(d.clone()));
        });
        d
    }

    fn permission(&self, p: Permission) -> PermissionState {
        let os = self.core.backend.permissions();
        self.core.run(|i, _| state_permissions(&i.opts, os)).get(p)
    }

    fn request_permission(&self, p: Permission) -> PermissionState {
        // Windows has no prompt for desktop apps: the switches live in Settings › Privacy,
        // which the UI opens for a denied grant. Report what is set now.
        let os = self.core.backend.permissions();
        self.core.run(|i, ev| {
            let s = state_permissions(&i.opts, os).get(p);
            if i.state.permissions.get(p) != s {
                i.state.permissions.set(p, s);
                ev.push(EngineEvent::PermissionChanged {
                    permission: p,
                    state: s,
                });
            }
            s
        })
    }

    fn shutdown(&self) {
        if self.core.quit.swap(true, Ordering::SeqCst) {
            return;
        }
        // Same rule as `MacEngine::shutdown`: the ticker may be inside an event sink that
        // waits on the thread calling this, so wait a bounded time, then go on without it.
        if let Some(t) = self.ticker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let deadline = Instant::now() + TICKER_STOP_WAIT;
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
        self.core.run(|i, ev| {
            i.countdown_until = None;
            if i.recording.is_some() {
                let _ = i.act(Action::Stop);
                i.begin_finalize(false, ev);
            }
            if let Some(f) = &i.finishing {
                while !f.handle.is_finished() {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            i.finish_done(ev);
        });
    }

    fn set_preview_pixel_size(&self, _width: u32, _height: u32) {}

    fn watch_devices(&self) {
        // Polled by the ticker (`DEVICE_POLL_TICKS`); ask for a fresh list now.
        self.core.devices_changed.store(true, Ordering::SeqCst);
    }

    fn set_stream_key(&self, dest: &StreamDest, key: &str) -> Result<(), EngineError> {
        let url = dest_url(dest);
        let key = key.trim();
        if url.is_empty() || key.is_empty() {
            return Err(EngineError::failed("Enter a server URL and a stream key"));
        }
        self.core
            .backend
            .secrets()
            .set(&url, key)
            .map_err(|e| EngineError::failed(format!("Could not save the stream key: {e}")))
    }

    fn remove_stream_key(&self, dest: &StreamDest) {
        let url = dest_url(dest);
        if !url.is_empty() {
            let _ = self.core.backend.secrets().delete(&url);
        }
    }

    fn with_key_status(&self, mut settings: RecorderSettings) -> RecorderSettings {
        for d in &mut settings.stream_dests {
            let url = dest_url(d);
            d.key_stored = !url.is_empty() && self.core.backend.secrets().get(&url).is_some();
        }
        settings
    }
}

impl<B: Backend> Drop for Engine<B> {
    /// Recordings in progress are finalized, never lost.
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rec_core::state::{Display, Rect};
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;

    #[derive(Default)]
    struct FakeSecrets(Mutex<HashMap<String, String>>);

    impl Secrets for FakeSecrets {
        fn get(&self, account: &str) -> Option<String> {
            self.0.lock().ok()?.get(account).cloned()
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            self.0
                .lock()
                .map_err(|e| e.to_string())?
                .insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            self.0.lock().map_err(|e| e.to_string())?.remove(account);
            Ok(())
        }
    }

    /// Shared with the test: what the fake session was asked to do.
    #[derive(Default)]
    struct Log {
        frames: AtomicU64,
        paused: AtomicBool,
        fail: Mutex<Option<String>>,
        requests: Mutex<Vec<SessionRequest>>,
    }

    struct FakeSession {
        log: Arc<Log>,
        file: PathBuf,
    }

    impl Session for FakeSession {
        fn has_frames(&self) -> bool {
            true
        }
        fn recorded_secs(&self) -> f64 {
            self.log.frames.load(Ordering::SeqCst) as f64 / 30.0
        }
        fn dropped_frames(&self) -> u64 {
            0
        }
        fn take_levels(&self) -> [f32; 2] {
            [0.5, 0.25]
        }
        fn pause(&self) -> bool {
            !self.log.paused.swap(true, Ordering::SeqCst)
        }
        fn resume(&self) -> bool {
            self.log.paused.swap(false, Ordering::SeqCst)
        }
        fn error(&self) -> Option<String> {
            self.log.fail.lock().ok()?.clone()
        }
        fn update(&self, _settings: &RecorderSettings) {}
        fn finish(self: Box<Self>) -> Result<SessionStats, String> {
            std::fs::write(&self.file, b"fake mp4").map_err(|e| e.to_string())?;
            Ok(SessionStats {
                frames: self.log.frames.load(Ordering::SeqCst),
            })
        }
    }

    struct Fake {
        log: Arc<Log>,
        perms: Permissions,
        secrets: FakeSecrets,
    }

    impl Backend for Fake {
        fn capabilities(&self) -> PlatformCapabilities {
            crate::caps::windows_capabilities(&crate::caps::Probe {
                build: 22_631,
                capture: true,
                hevc: false,
            })
        }
        fn devices(&self) -> Devices {
            Devices {
                displays: vec![Display {
                    id: 1,
                    name: "Display 1".into(),
                    width: 1920,
                    height: 1080,
                    frame: Rect {
                        x: 0.0,
                        y: 0.0,
                        w: 1920.0,
                        h: 1080.0,
                    },
                    scale: 1.0,
                }],
                ..Devices::default()
            }
        }
        fn permissions(&self) -> Permissions {
            self.perms
        }
        fn start(&self, req: &SessionRequest) -> Result<Started, EngineError> {
            if let Ok(mut r) = self.log.requests.lock() {
                r.push(req.clone());
            }
            Ok(Started {
                session: Box::new(FakeSession {
                    log: self.log.clone(),
                    file: req.file.clone(),
                }),
                info: SessionInfo {
                    width: 1920,
                    height: 1080,
                    fps: 30,
                    audio_tracks: 3,
                },
                notes: vec!["a note".into()],
            })
        }
        fn free_disk_bytes(&self, _dir: &Path) -> u64 {
            42
        }
        fn local_time(&self) -> LocalTime {
            LocalTime {
                year: 2026,
                month: 11,
                day: 3,
                hour: 12,
                minute: 0,
                second: 1,
            }
        }
        fn secrets(&self) -> &dyn Secrets {
            &self.secrets
        }
    }

    fn granted() -> Permissions {
        Permissions {
            screen: PermissionState::Granted,
            microphone: PermissionState::Granted,
            camera: PermissionState::Granted,
        }
    }

    type EventLog = Arc<Mutex<Vec<EngineEvent>>>;

    fn engine(perms: Permissions) -> (Engine<Fake>, Arc<Log>, EventLog, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rec-engine-windows-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let log = Arc::new(Log::default());
        let events = Arc::new(Mutex::new(vec![]));
        let ev = events.clone();
        let sink: EventSink = Arc::new(move |e| {
            if let Ok(mut v) = ev.lock() {
                v.push(e);
            }
        });
        let settings = RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        };
        let e = Engine::new(
            Fake {
                log: log.clone(),
                perms,
                secrets: FakeSecrets::default(),
            },
            EngineOptions {
                require_self_exclusion: true,
                ..EngineOptions::default()
            },
            settings,
            sink,
        );
        (e, log, events, dir)
    }

    fn wait_for(events: &Mutex<Vec<EngineEvent>>, f: impl Fn(&EngineEvent) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if events.lock().map(|v| v.iter().any(&f)).unwrap_or(false) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn record_pause_resume_stop_writes_a_recording() {
        let (e, log, events, dir) = engine(granted());
        let s = e.state();
        assert!(s.capabilities.recording);
        assert_eq!(s.free_disk_bytes, 42);
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        })
        .unwrap();
        assert_eq!(e.state().status, EngineStatus::Recording);
        let req = log.requests.lock().unwrap()[0].clone();
        assert!(req.exclude_self);
        assert!(req.file.ends_with("rec-2026-11-03-120001.mp4"));
        assert!(req.plan.screen && req.plan.camera && req.plan.mic);
        e.pause().unwrap();
        assert_eq!(e.state().status, EngineStatus::Paused);
        assert!(e.pause().is_err(), "already paused");
        e.resume().unwrap();
        // Meters and progress run while recording.
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::Levels(l) if l.system > 0.0)
        ));
        log.frames.store(90, Ordering::SeqCst);
        e.stop().unwrap();
        assert_eq!(e.state().status, EngineStatus::Finalizing);
        assert!(wait_for(&events, |ev| matches!(
            ev,
            EngineEvent::RecordingFinished { .. }
        )));
        let r = e.state().recordings[0].clone();
        assert_eq!(
            (r.width, r.height, r.fps, r.audio_tracks),
            (1920, 1080, 30, 3)
        );
        assert_eq!(r.duration_secs, 3.0);
        assert_eq!(r.bytes, 8);
        assert_eq!(r.id, "rec-2026-11-03-120001");
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::Error { message, recoverable: true } if message == "a note")
        ));
        assert_eq!(e.state().status, EngineStatus::Idle);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn discard_deletes_the_file() {
        let (e, _log, events, dir) = engine(granted());
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        })
        .unwrap();
        e.discard().unwrap();
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::StateChanged(s) if s.status == EngineStatus::Idle && s.recordings.is_empty())
        ));
        assert_eq!(std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failing_source_keeps_what_was_written() {
        let (e, log, events, dir) = engine(granted());
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        })
        .unwrap();
        *log.fail.lock().unwrap() = Some("the GPU went away".into());
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::Error { message, recoverable: false } if message.contains("GPU"))
        ));
        assert!(wait_for(&events, |ev| matches!(
            ev,
            EngineEvent::RecordingFinished { .. }
        )));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_grants_degrade_or_block() {
        let mut p = granted();
        p.microphone = PermissionState::Denied;
        let (e, log, events, dir) = engine(p);
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        })
        .unwrap();
        assert!(!log.requests.lock().unwrap()[0].plan.mic);
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::Error { message, .. } if message.contains("microphone"))
        ));
        e.discard().unwrap();
        let _ = std::fs::remove_dir_all(dir);

        let mut p = granted();
        p.camera = PermissionState::Denied;
        let (e, _log, _events, dir) = engine(p);
        let r = e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            scene: rec_core::settings::Scene::Cam,
            ..RecorderSettings::default()
        });
        assert_eq!(
            r,
            Err(EngineError::MissingPermissions {
                permissions: vec![Permission::Camera]
            })
        );
        assert_eq!(e.state().status, EngineStatus::Idle);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn countdown_can_be_cancelled_and_replay_is_unsupported() {
        let (e, _log, _events, dir) = engine(granted());
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 3,
            ..RecorderSettings::default()
        })
        .unwrap();
        assert_eq!(e.state().status, EngineStatus::Countdown);
        assert_eq!(e.state().countdown_left, 3);
        e.cancel_countdown().unwrap();
        assert_eq!(e.state().status, EngineStatus::Idle);
        assert!(matches!(
            e.save_replay(),
            Err(EngineError::Unsupported { .. })
        ));
        assert!(matches!(e.go_live(), Err(EngineError::Unsupported { .. })));
        assert!(e.set_preview_enabled(true).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn devices_arrive_on_the_first_tick() {
        let (e, _log, events, _dir) = engine(granted());
        assert!(wait_for(
            &events,
            |ev| matches!(ev, EngineEvent::DeviceChanged(d) if d.displays.len() == 1)
        ));
        assert_eq!(e.state().displays.len(), 1);
    }

    #[test]
    fn stream_keys_go_to_the_secret_store() {
        let (e, _log, _events, _dir) = engine(granted());
        let dest = StreamDest::default();
        assert!(e.set_stream_key(&dest, "  ").is_err());
        e.set_stream_key(&dest, " abc ").unwrap();
        let s = e.with_key_status(RecorderSettings::default());
        assert!(s.stream_dests[0].key_stored);
        assert_eq!(
            e.backend()
                .secrets()
                .get("rtmps://a.rtmps.youtube.com:443/live2"),
            Some("abc".into())
        );
        e.remove_stream_key(&dest);
        assert!(!e.with_key_status(RecorderSettings::default()).stream_dests[0].key_stored);
    }

    #[test]
    fn shutdown_finalizes_a_running_recording() {
        let (e, log, events, dir) = engine(granted());
        e.start(&RecorderSettings {
            output_dir: dir.display().to_string(),
            countdown: 0,
            ..RecorderSettings::default()
        })
        .unwrap();
        log.frames.store(30, Ordering::SeqCst);
        e.shutdown();
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|ev| matches!(ev, EngineEvent::RecordingFinished { .. }))
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
