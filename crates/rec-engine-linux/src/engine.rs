//! SOURCE OF TRUTH KEYWORDS: LinuxEngine, RecorderEngine, countdown, pause, discard, finalize, ticker, levels, portal pick, test pattern
//! WHAT:  The Linux `RecorderEngine`: settings in, recordings out. It asks the portal for the
//!        screen before the countdown, runs one GStreamer `Session` per recording, cuts
//!        pauses out of the timeline, finalizes on a background thread, and reports state,
//!        levels (30 a second), progress (10 a second) and device changes like the Mac engine.
//! WHY:   The Tauri shell and its React surfaces drive every platform through the same trait
//!        and events. The state machine is `rec_core::api::transition`, source availability is
//!        `rec_core::sources::plan_sources`, sizes and bitrates `rec_core::output`.
//!        The test pattern (`EngineOptions::test_pattern`) swaps every source for a generator
//!        (`videotestsrc`, `audiotestsrc`), so the whole engine runs headless in CI and Docker.
//! WHERE: `apps/desktop/src-tauri/src/platform/linux.rs`; `tests/engine.rs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rec_core::api::{Action, EngineError, EngineEvent, EventSink, RecorderEngine, transition};
use rec_core::library::{LocalTime, timestamped_name};
use rec_core::platform::{EngineOptions, PlatformCapabilities, Secrets};
use rec_core::settings::{
    CaptureTarget, Container, RecorderSettings, Scene, StreamDest, StreamService,
};
use rec_core::sources::{dropped_note, plan_sources};
use rec_core::state::{
    Devices, Display, EngineState, EngineStatus, InputDevice, Levels, Permission, PermissionState,
    Permissions, Progress, Recording, Rect,
};

use crate::caps::{Probe, linux_capabilities};
use crate::devices::DeviceList;
use crate::pipeline::{AudioRole, CameraUse, crop_for, source_for_id};
use crate::portal::{PortalStream, pick};
use crate::portal_state::{PickKind, PortalState};
use crate::recording::{ScreenSource, Sources, camera_size, db_to_linear, plan_recording};
use crate::secrets::SecretService;
use crate::session::{MaskSpec, Session, has_factory, init};

/// The test pattern's screen size and the default display size before any portal pick.
const DEFAULT_SCREEN: (u32, u32) = (1920, 1080);
/// How long the system picker may stay open.
const PICK_TIMEOUT: Duration = Duration::from_secs(300);

fn unsupported(m: &str) -> EngineError {
    EngineError::Unsupported { message: m.into() }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// The wall-clock time in the user's time zone, for file names.
fn local_now() -> LocalTime {
    use chrono::{Datelike, Timelike};
    let t = chrono::Local::now();
    let b = |v: u32| u8::try_from(v).unwrap_or(0);
    LocalTime {
        year: u16::try_from(t.year()).unwrap_or(0),
        month: b(t.month()),
        day: b(t.day()),
        hour: b(t.hour()),
        minute: b(t.minute()),
        second: b(t.second()),
    }
}

fn free_disk_bytes(dir: &Path) -> u64 {
    let Some(dir) = dir.ancestors().find(|d| d.exists()) else {
        return 0;
    };
    rustix::fs::statvfs(dir)
        .map(|s| s.f_bavail.saturating_mul(s.f_frsize))
        .unwrap_or(0)
}

/// The account a destination's key is stored under: its service, or a custom URL.
pub fn key_account(d: &StreamDest) -> String {
    match d.service {
        StreamService::Custom => d.url.clone().unwrap_or_default().trim().to_string(),
        s => s.as_str().to_string(),
    }
}

/// Linux has no per-app privacy grants outside Flatpak: the portal asks for the screen
/// when a recording starts, and mics and cameras are open to the user's session.
fn linux_permissions(opts: &EngineOptions) -> Permissions {
    let mut p = Permissions {
        screen: PermissionState::Granted,
        microphone: PermissionState::Granted,
        camera: PermissionState::Granted,
    };
    for w in &opts.withheld {
        p.set(*w, PermissionState::NotDetermined);
    }
    p
}

/// One display stands for "what the portal will give": its size is the last pick's, so
/// the UI shows the right shape and sizes before the first recording.
fn portal_display(size: Option<(u32, u32)>) -> Display {
    let (w, h) = size.unwrap_or(DEFAULT_SCREEN);
    Display {
        id: 1,
        name: "Screen chosen in the system dialog".into(),
        width: w,
        height: h,
        frame: Rect {
            x: 0.0,
            y: 0.0,
            w: f64::from(w),
            h: f64::from(h),
        },
        scale: 1.0,
    }
}

/// Sources opened before a recording (the countdown runs with them ready).
struct Prepared {
    sources: Sources,
    portal: Option<PortalStream>,
    notes: Vec<String>,
}

struct Meta {
    file: PathBuf,
    size: (u32, u32),
    fps: u32,
    audio_tracks: u32,
    created_ms: f64,
    camera_track: bool,
}

struct Active {
    sess: Session,
    meta: Meta,
}

struct Finishing {
    handle: std::thread::JoinHandle<Result<crate::session::Stats, String>>,
    meta: Meta,
    discard: bool,
}

struct Inner {
    opts: EngineOptions,
    settings: RecorderSettings,
    state: EngineState,
    caps: PlatformCapabilities,
    countdown_until: Option<Instant>,
    pending: Option<Prepared>,
    recording: Option<Active>,
    finishing: Option<Finishing>,
    portal_path: PathBuf,
    portal_state: PortalState,
    devices: Option<DeviceList>,
    ticks: u64,
    last_levels: Instant,
}

type Events = Vec<EngineEvent>;

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

    fn current_devices(&self) -> Devices {
        let (cameras, mics) = match (&self.devices, self.opts.test_pattern) {
            (_, true) => (
                vec![InputDevice {
                    id: "test:camera".into(),
                    name: "Test pattern camera".into(),
                    connected: true,
                }],
                vec![InputDevice {
                    id: "test:mic".into(),
                    name: "Test tone".into(),
                    connected: true,
                }],
            ),
            (Some(d), false) => d.current(),
            (None, false) => (vec![], vec![]),
        };
        Devices {
            displays: vec![portal_display(self.portal_state.last_size)],
            windows: vec![],
            apps: vec![],
            cameras,
            mics,
        }
    }

    fn begin_recording(&mut self, prepared: Prepared, ev: &mut Events) -> Result<(), EngineError> {
        let s = self.settings.clone();
        let started = (|| {
            let dir = s.output_dir_in(&self.opts.home);
            std::fs::create_dir_all(&dir).map_err(|e| {
                EngineError::failed(format!("Could not create {}: {e}", dir.display()))
            })?;
            let ext = match s.container {
                Container::Mp4 => "mp4",
                Container::Mov => "mov",
            };
            let file = dir.join(timestamped_name("rec", ext, local_now()));
            let planned =
                plan_recording(&s, &prepared.sources, &has_factory).map_err(EngineError::failed)?;
            let notes: Vec<String> = prepared
                .notes
                .iter()
                .cloned()
                .chain(planned.note.clone())
                .collect();
            if !notes.is_empty() {
                ev.push(EngineEvent::Error {
                    message: notes.join(" · "),
                    recoverable: true,
                });
            }
            let mask = MaskSpec::from_settings(&s, planned.plan.out.1);
            let keepalive: Option<Box<dyn Send>> =
                prepared.portal.map(|p| Box::new(p) as Box<dyn Send>);
            let sess = Session::start(&planned, &file, mask, keepalive)
                .map_err(|e| EngineError::failed(format!("Could not start recording: {e}")))?;
            let plan = sess.plan();
            let meta = Meta {
                file,
                size: plan.out,
                fps: plan.fps,
                audio_tracks: plan.tracks.len() as u32,
                created_ms: now_ms(),
                camera_track: plan
                    .camera
                    .as_ref()
                    .is_some_and(|c| c.track.is_some() && plan.camera_encoder.is_some()),
            };
            Ok::<_, EngineError>(Active { sess, meta })
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

    fn begin_finalize(&mut self, discard: bool, ev: &mut Events) {
        let Some(Active { sess, meta }) = self.recording.take() else {
            return;
        };
        let handle = std::thread::Builder::new()
            .name("recording-finish".into())
            .spawn(move || {
                if discard {
                    sess.stop_now();
                    Ok(crate::session::Stats::default())
                } else {
                    sess.finish()
                }
            });
        match handle {
            Ok(handle) => {
                self.finishing = Some(Finishing {
                    handle,
                    meta,
                    discard,
                })
            }
            Err(e) => ev.push(EngineEvent::Error {
                message: format!("Could not finish the recording: {e}"),
                recoverable: false,
            }),
        }
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
            let _ = std::fs::remove_file(&f.meta.file);
        } else {
            match result {
                Ok(stats) => {
                    let m = &f.meta;
                    let name = m
                        .file
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let r = Recording {
                        id: name.clone(),
                        name,
                        path: m.file.display().to_string(),
                        // The timeline's length (pauses cut): frames dropped under load leave
                        // gaps in the file but do not shorten it.
                        duration_secs: if stats.recorded_secs > 0.0 {
                            stats.recorded_secs
                        } else {
                            stats.video_frames as f64 / f64::from(m.fps.max(1))
                        },
                        bytes: std::fs::metadata(&m.file).map(|x| x.len()).unwrap_or(0),
                        created_at: m.created_ms,
                        width: m.size.0,
                        height: m.size.1,
                        has_camera_track: m.camera_track,
                        fps: m.fps,
                        audio_tracks: m.audio_tracks,
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
            ..Default::default()
        };
        if let Some(a) = &self.recording {
            p.elapsed_secs = a.sess.recorded_secs();
            // Frames the timeline should have minus frames that reached the encoder; half a
            // second of slack covers the pipeline's latency.
            let expected = (p.elapsed_secs * f64::from(a.meta.fps)) as u64;
            p.dropped_frames =
                expected.saturating_sub(a.sess.video_frames() + u64::from(a.meta.fps / 2));
            p.bytes_written = std::fs::metadata(&a.meta.file)
                .map(|m| m.len())
                .unwrap_or(0);
        }
        p
    }

    fn tick(&mut self, ev: &mut Events) {
        self.ticks += 1;
        if self.state.status == EngineStatus::Countdown
            && let Some(until) = self.countdown_until
        {
            let left = until
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u32;
            if left == 0 {
                let prepared = self.pending.take().unwrap_or(Prepared {
                    sources: Sources::default(),
                    portal: None,
                    notes: vec![],
                });
                if let Err(e) = self.begin_recording(prepared, ev) {
                    ev.push(EngineEvent::Error {
                        message: e.to_string(),
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
        // Every 2 s: plugged or unplugged devices, disk space.
        if self.ticks.is_multiple_of(60) {
            let d = self.current_devices();
            if d.cameras != self.state.cameras || d.mics != self.state.mics {
                self.state.set_devices(d.clone());
                ev.push(EngineEvent::DeviceChanged(d));
            }
            self.state.free_disk_bytes =
                free_disk_bytes(&self.settings.output_dir_in(&self.opts.home));
        }
    }
}

struct Core {
    inner: Mutex<Inner>,
    events: EventSink,
    quit: AtomicBool,
    /// A portal pick is in progress (start() runs it outside the lock).
    picking: AtomicBool,
    secrets: SecretService,
}

impl Core {
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
}

/// The source text for the system audio (the default output's monitor), if any.
fn system_audio_source() -> Option<String> {
    if has_factory("pulsesrc") {
        Some("pulsesrc device=@DEFAULT_MONITOR@".into())
    } else if has_factory("pipewiresrc") {
        Some("pipewiresrc stream-properties=\"props,stream.capture.sink=true\"".into())
    } else {
        None
    }
}

fn default_mic_source() -> Option<String> {
    ["pulsesrc", "pipewiresrc", "autoaudiosrc"]
        .into_iter()
        .find(|f| has_factory(f))
        .map(str::to_string)
}

/**
 * SOURCE OF TRUTH KEYWORDS: LinuxEngine, Linux recorder engine
 * WHAT:  See the module comment.
 * WHY:   One engine object per app, driven by the shell through `RecorderEngine`.
 * WHERE: `platform::linux::open_engine`.
 */
pub struct LinuxEngine {
    core: Arc<Core>,
    ticker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl LinuxEngine {
    pub fn new(opts: EngineOptions, settings: RecorderSettings, events: EventSink) -> LinuxEngine {
        let gst_ok = init();
        let has = |f: &str| gst_ok && has_factory(f);
        let can_record = gst_ok
            && crate::encoders::H264_ENCODERS.iter().any(|f| has(f))
            && has("mp4mux")
            && has("compositor");
        let probe = Probe {
            can_record,
            hevc: crate::encoders::has_hevc(&has),
            system_audio: opts.test_pattern || has("pulsesrc") || has("pipewiresrc"),
            global_shortcuts: !opts.test_pattern && crate::desktop::global_shortcuts_available(),
        };
        let caps = linux_capabilities(&probe);
        let portal_path =
            PortalState::path(&opts.home, std::env::var("XDG_STATE_HOME").ok().as_deref());
        let portal_state = PortalState::load(&portal_path);
        let mut state = EngineState {
            capabilities: caps,
            permissions: linux_permissions(&opts),
            ..EngineState::default()
        };
        let settings = settings.validated();
        state.free_disk_bytes = free_disk_bytes(&settings.output_dir_in(&opts.home));
        let mut inner = Inner {
            opts,
            settings,
            state,
            caps,
            countdown_until: None,
            pending: None,
            recording: None,
            finishing: None,
            portal_path,
            portal_state,
            devices: None,
            ticks: 0,
            last_levels: Instant::now(),
        };
        let d = inner.current_devices();
        inner.state.set_devices(d);
        let core = Arc::new(Core {
            inner: Mutex::new(inner),
            events,
            quit: AtomicBool::new(false),
            picking: AtomicBool::new(false),
            secrets: SecretService,
        });
        let c = core.clone();
        let ticker = std::thread::Builder::new()
            .name("engine-tick".into())
            .spawn(move || {
                while !c.quit.load(Ordering::Relaxed) {
                    c.run(|i, ev| i.tick(ev));
                    std::thread::sleep(Duration::from_millis(33));
                }
            })
            .ok();
        LinuxEngine {
            core,
            ticker: Mutex::new(ticker),
        }
    }

    /// Open the sources `s` needs: the portal pick (blocking on the system dialog), the
    /// camera, the mic and system audio. Runs outside the engine lock.
    fn prepare(&self, s: &RecorderSettings) -> Result<Prepared, EngineError> {
        let (opts, perms, portal_state, devices) = self.core.run(|i, _| {
            (
                i.opts.clone(),
                i.state.permissions,
                i.portal_state.clone(),
                i.current_devices(),
            )
        });
        let test = opts.test_pattern;
        let plan = plan_sources(s.scene, s.mic_enabled, &perms, test)
            .map_err(|permissions| EngineError::MissingPermissions { permissions })?;
        let mut notes: Vec<String> = dropped_note(&plan.dropped).into_iter().collect();
        let mut sources = Sources::default();
        let mut portal = None;
        if plan.screen && s.scene != Scene::Cam {
            if test {
                let (w, h) = DEFAULT_SCREEN;
                let crop = match &s.target {
                    CaptureTarget::Region {
                        x, y, w: rw, h: rh, ..
                    } => crop_for([*x, *y, *rw, *rh], (f64::from(w), f64::from(h)), (w, h)),
                    _ => None,
                };
                sources.screen = Some(ScreenSource {
                    src: format!(
                        "videotestsrc is-live=true pattern=smpte ! video/x-raw,width={w},height={h},framerate=60/1"
                    ),
                    size: (w, h),
                    crop,
                });
            } else {
                let kind = match &s.target {
                    CaptureTarget::Window { .. } => PickKind::Window,
                    CaptureTarget::App { .. } => {
                        return Err(unsupported(
                            "Recording one app isn't available on Linux. Choose a screen or a window",
                        ));
                    }
                    _ => PickKind::Monitor,
                };
                let picked = pick(
                    kind,
                    s.cursor,
                    portal_state.token(kind).map(str::to_string),
                    PICK_TIMEOUT,
                )
                .map_err(EngineError::failed)?;
                let size = picked
                    .stream
                    .size
                    .or(portal_state.last_size)
                    .unwrap_or(DEFAULT_SCREEN);
                let crop = match &s.target {
                    CaptureTarget::Region { x, y, w, h, .. } => {
                        // The region was drawn on the display Cortex reported (the last
                        // stream's size); scale it to this stream.
                        let shown = portal_state.last_size.unwrap_or(size);
                        crop_for(
                            [*x, *y, *w, *h],
                            (f64::from(shown.0), f64::from(shown.1)),
                            size,
                        )
                    }
                    _ => None,
                };
                sources.screen = Some(ScreenSource {
                    src: picked.stream.source(),
                    size,
                    crop,
                });
                self.core.run(|i, ev| {
                    i.portal_state.set_token(kind, picked.restore_token.clone());
                    if kind == PickKind::Monitor {
                        i.portal_state.last_size = Some(size);
                    }
                    if let Err(e) = i.portal_state.save(&i.portal_path) {
                        eprintln!("cortex: could not save the portal token: {e}");
                    }
                    let d = i.current_devices();
                    if d.displays != i.state.displays {
                        i.state.set_devices(d.clone());
                        ev.push(EngineEvent::DeviceChanged(d));
                    }
                });
                portal = Some(picked.stream);
            }
        }
        if plan.camera {
            let (w, h) = camera_size(s.camera_resolution);
            sources.camera = if test {
                Some(format!(
                    "videotestsrc is-live=true pattern=ball ! video/x-raw,width={w},height={h},framerate=30/1"
                ))
            } else {
                let id = s
                    .camera_id
                    .clone()
                    .filter(|id| devices.cameras.iter().any(|c| &c.id == id))
                    .or_else(|| devices.cameras.first().map(|c| c.id.clone()));
                let src = id.as_deref().and_then(source_for_id);
                if src.is_none() {
                    notes.push("No camera found; recording without it".into());
                }
                src
            };
        }
        if s.system_audio {
            sources.system = if test {
                Some("audiotestsrc is-live=true wave=sine freq=440 volume=0.4".into())
            } else {
                system_audio_source()
            };
        }
        if plan.mic {
            sources.mic = if test {
                Some("audiotestsrc is-live=true wave=sine freq=880 volume=0.3".into())
            } else {
                s.mic_id
                    .as_deref()
                    .filter(|id| devices.mics.iter().any(|m| m.id == *id))
                    .and_then(source_for_id)
                    .or_else(default_mic_source)
            };
        }
        if sources.screen.is_none() && sources.camera.is_none() {
            return Err(EngineError::failed(
                "Nothing to record: no screen and no camera",
            ));
        }
        Ok(Prepared {
            sources,
            portal,
            notes,
        })
    }

    fn apply_live(&self) {
        self.core.run(|i, _| {
            let s = i.settings.clone();
            let Some(a) = &i.recording else { return };
            a.sess.set_volume(
                AudioRole::System,
                db_to_linear(s.system_gain_db),
                s.system_muted,
            );
            a.sess
                .set_volume(AudioRole::Mic, db_to_linear(s.mic_gain_db), s.mic_muted);
            let plan = a.sess.plan();
            if let Some(cam) = &plan.camera
                && let CameraUse::Overlay([_, _, w, h]) = cam.place
            {
                // Same size as the running overlay, new place.
                let (ow, oh) = (f64::from(plan.out.0), f64::from(plan.out.1));
                let r = rec_core::overlay::overlay_rect(
                    f64::from(w) / ow,
                    &s.pip_place,
                    rec_core::settings::PipShape::Rect,
                    ow,
                    oh,
                    f64::from(w) / f64::from(h.max(1)),
                );
                a.sess.set_overlay(
                    MaskSpec::from_settings(&s, plan.out.1),
                    Some((r[0] as u32, r[1] as u32)),
                );
            }
        });
    }
}

impl RecorderEngine for LinuxEngine {
    fn state(&self) -> EngineState {
        self.core.run(|i, _| i.state.clone())
    }

    fn capabilities(&self) -> PlatformCapabilities {
        self.core.run(|i, _| i.caps)
    }

    fn start(&self, settings: &RecorderSettings) -> Result<(), EngineError> {
        let s = settings.clone().validated();
        let countdown = s.countdown > 0;
        let action = if countdown {
            Action::StartCountdown
        } else {
            Action::Start
        };
        self.core.run(|i, _| {
            if !i.caps.recording {
                return Err(unsupported(
                    "Recording needs GStreamer with an H.264 encoder (x264, OpenH264, VA-API or NVENC)",
                ));
            }
            i.act(action)?;
            i.settings = s.clone();
            Ok(())
        })?;
        if self.core.picking.swap(true, Ordering::SeqCst) {
            return Err(EngineError::failed("Already choosing what to record"));
        }
        let prepared = self.prepare(&s);
        self.core.picking.store(false, Ordering::SeqCst);
        let prepared = match prepared {
            Ok(p) => p,
            Err(e) => {
                self.core.run(|_, ev| {
                    ev.push(EngineEvent::Error {
                        message: e.to_string(),
                        recoverable: true,
                    })
                });
                return Err(e);
            }
        };
        self.core.run(|i, ev| {
            // The status may have moved while the dialog was open (a quit, say).
            i.act(action)?;
            if countdown {
                i.countdown_until =
                    Some(Instant::now() + Duration::from_secs(u64::from(s.countdown)));
                i.state.countdown_left = s.countdown;
                i.pending = Some(prepared);
                i.set_status(EngineStatus::Countdown, ev);
                Ok(())
            } else {
                i.begin_recording(prepared, ev)
            }
        })
    }

    fn cancel_countdown(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::CancelCountdown)?;
            i.countdown_until = None;
            i.pending = None;
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
            if let Some(a) = &i.recording {
                a.sess.pause();
            }
            i.set_status(s, ev);
            Ok(())
        })
    }

    fn resume(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::Resume)?;
            if let Some(a) = &i.recording {
                a.sess.resume();
            }
            i.set_status(s, ev);
            Ok(())
        })
    }

    fn discard(&self) -> Result<(), EngineError> {
        self.core.run(|i, ev| {
            let s = i.act(Action::Discard)?;
            if i.state.status == EngineStatus::Countdown {
                i.countdown_until = None;
                i.pending = None;
                i.set_status(s, ev);
            } else {
                i.begin_finalize(true, ev);
            }
            Ok(())
        })
    }

    fn update_settings(&self, settings: &RecorderSettings) -> Result<(), EngineError> {
        let s = settings.clone().validated();
        self.core.run(|i, _| i.settings = s);
        self.apply_live();
        Ok(())
    }

    fn set_preview_enabled(&self, enabled: bool) -> Result<(), EngineError> {
        if enabled {
            Err(unsupported("The live preview isn't available on Linux yet"))
        } else {
            Ok(())
        }
    }

    fn save_replay(&self) -> Result<(), EngineError> {
        Err(unsupported(
            "The replay buffer isn't available on Linux yet",
        ))
    }

    fn go_live(&self) -> Result<(), EngineError> {
        Err(unsupported("Going live isn't available on Linux yet"))
    }

    fn stop_live(&self) -> Result<(), EngineError> {
        Ok(())
    }

    fn devices(&self) -> Devices {
        self.core.run(|i, _| i.current_devices())
    }

    fn permission(&self, p: Permission) -> PermissionState {
        self.core.run(|i, _| i.state.permissions.get(p))
    }

    fn request_permission(&self, p: Permission) -> PermissionState {
        // Nothing to ask ahead of time: the portal asks for the screen at record time.
        self.permission(p)
    }

    fn shutdown(&self) {
        let _ = self.stop();
        let deadline = Instant::now() + Duration::from_secs(25);
        while Instant::now() < deadline {
            let busy = self
                .core
                .run(|i, _| i.finishing.is_some() || i.recording.is_some());
            if !busy {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.core.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.ticker.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
        // Finish a recording the ticker did not get to.
        self.core.run(|i, ev| i.finish_done(ev));
    }

    fn set_preview_pixel_size(&self, _width: u32, _height: u32) {}

    fn watch_devices(&self) {
        let list = DeviceList::start();
        self.core.run(|i, ev| {
            if i.opts.test_pattern {
                return;
            }
            i.devices = list;
            let d = i.current_devices();
            i.state.set_devices(d.clone());
            ev.push(EngineEvent::DeviceChanged(d));
        });
    }

    fn set_stream_key(&self, dest: &StreamDest, key: &str) -> Result<(), EngineError> {
        self.core
            .secrets
            .set(&key_account(dest), key)
            .map_err(EngineError::failed)
    }

    fn remove_stream_key(&self, dest: &StreamDest) {
        if let Err(e) = self.core.secrets.delete(&key_account(dest)) {
            eprintln!("cortex: could not remove a stream key: {e}");
        }
    }

    fn with_key_status(&self, mut settings: RecorderSettings) -> RecorderSettings {
        for d in &mut settings.stream_dests {
            d.key_stored = self.core.secrets.get(&key_account(d)).is_some();
        }
        settings
    }
}

impl Drop for LinuxEngine {
    fn drop(&mut self) {
        self.core.quit.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stored_per_service_or_url() {
        let d = StreamDest {
            service: StreamService::Twitch,
            ..StreamDest::default()
        };
        assert_eq!(key_account(&d), "twitch");
        let d = StreamDest {
            service: StreamService::Custom,
            url: Some(" rtmp://example/live ".into()),
            ..StreamDest::default()
        };
        assert_eq!(key_account(&d), "rtmp://example/live");
    }

    #[test]
    fn the_portal_display_takes_the_last_size() {
        assert_eq!(portal_display(None).width, 1920);
        let d = portal_display(Some((2560, 1440)));
        assert_eq!((d.width, d.height, d.scale), (2560, 1440, 1.0));
        assert_eq!(d.frame.w, 2560.0);
    }

    #[test]
    fn withheld_grants_read_as_not_asked() {
        let opts = EngineOptions {
            withheld: vec![Permission::Camera],
            ..EngineOptions::default()
        };
        let p = linux_permissions(&opts);
        assert_eq!(p.camera, PermissionState::NotDetermined);
        assert_eq!(p.screen, PermissionState::Granted);
    }
}
