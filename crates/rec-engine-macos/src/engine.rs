//! SOURCE OF TRUTH KEYWORDS: MacEngine, RecorderEngine, EngineOptions, record_args_for, rec_settings_for, preview sink
//! WHAT:  The macOS implementation of `rec_core::api::RecorderEngine`.
//! WHY:   The Tauri shell (and later the control socket) drive the recorder through the
//!        platform-neutral API. Recording itself is untouched: settings are translated into
//!        Rec's own `Settings`, then run down the exact path the reference GUI uses
//!        (`Settings::record_args` → `args::parse_record_args` → `args::prepare` →
//!        `record::Session::start_with_camera`).
//! WHERE: `apps/desktop/src-tauri` (planned), `examples/engine_smoke.rs`.
//!
//! Threads: commands run on the caller's thread under one lock; a ticker thread
//! (30 Hz) runs the countdown, meters, progress, failure checks and finalization
//! results. Events are sent after the lock is released, so a sink may call back
//! into the engine. `shutdown` never waits on the ticker for long (a sink may be
//! waiting on the thread that quits), and finalizes the recording itself.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rec_core::api::{Action, EngineError, EngineEvent, EventSink, RecorderEngine, transition};
use rec_core::settings::{self as cs, CaptureTarget, RecorderSettings, StreamService};
use rec_core::state::{
    CaptureWindow, Devices, Display, EngineState, EngineStatus, InputDevice, Levels, Permission,
    PermissionState, Permissions, Progress, Recording, StreamStatus,
};

use crate::compose::{Corner, Place, Scene, Shape};
use crate::record::{self, Session};
use crate::settings::{Settings, StreamDest as RecDest};
use crate::writer::{self, AudioLayout, Codec, PreviewFn, Stats};
use crate::{camera, live, screen, tcc};
use rec_core::sources::{dropped_note, plan_sources};

/// How the engine runs; not user settings.
#[derive(Clone, Debug)]
pub struct EngineOptions {
    /// Expands `~` in the output folder.
    pub home: PathBuf,
    /// Record Rec's generated test pattern instead of the screen (self-tests;
    /// needs no Screen Recording permission).
    pub test_pattern: bool,
    /// Stop recordings automatically after this many recorded seconds.
    pub max_duration_secs: Option<f64>,
    /// Treat these grants as missing (self-tests of the no-permission paths on a Mac
    /// where they are granted).
    pub withheld: Vec<Permission>,
}

impl Default for EngineOptions {
    fn default() -> Self {
        EngineOptions {
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default(),
            test_pattern: false,
            max_duration_secs: None,
            withheld: vec![],
        }
    }
}

// ---- Settings translation ----------------------------------------------------

fn unsupported(m: &str) -> EngineError {
    EngineError::Unsupported { message: m.into() }
}

fn rec_scene(s: cs::Scene) -> Scene {
    match s {
        cs::Scene::Screen => Scene::Screen,
        cs::Scene::ScreenCam => Scene::ScreenCam,
        cs::Scene::Cam => Scene::Cam,
    }
}

fn rec_shape(s: cs::PipShape) -> Shape {
    match s {
        cs::PipShape::Rounded => Shape::Rounded,
        cs::PipShape::Circle => Shape::Circle,
        cs::PipShape::Square => Shape::Square,
        cs::PipShape::Pill => Shape::Pill,
        cs::PipShape::Squircle => Shape::Squircle,
        cs::PipShape::Rect => Shape::Rect,
    }
}

fn rec_corner(c: cs::PipCorner) -> Corner {
    match c {
        cs::PipCorner::TopLeft => Corner::TopLeft,
        cs::PipCorner::TopRight => Corner::TopRight,
        cs::PipCorner::BottomLeft => Corner::BottomLeft,
        cs::PipCorner::BottomRight => Corner::BottomRight,
    }
}

fn hex_rgb(s: &str) -> [u8; 3] {
    let h = cs::normalize_hex(s).unwrap_or_else(|| "ffffff".into());
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(255);
    [c(0), c(2), c(4)]
}

/// The stream URL of a destination (a service's ingest URL, or the custom URL);
/// its Keychain entry is keyed by this URL, as in Rec.
pub fn dest_url(d: &cs::StreamDest) -> String {
    match d.service {
        StreamService::Custom => d.url.clone().unwrap_or_default().trim().to_string(),
        s => live::destination_url(s.as_str()),
    }
}

/// Our settings as Rec's (`crate::settings::Settings`), so Rec's own
/// `record_args`, `capture_key`, `pip_spec`, `volumes` and `live_config` apply.
pub fn rec_settings_for(s: &RecorderSettings, home: &Path) -> Result<Settings, EngineError> {
    let (display, window, region, app) = match &s.target {
        CaptureTarget::Display { display_id } => (*display_id as usize, None, None, None),
        CaptureTarget::Region {
            display_id,
            x,
            y,
            w,
            h,
        } => (*display_id as usize, None, Some([*x, *y, *w, *h]), None),
        CaptureTarget::Window { window_id } => (1, Some(*window_id), None, None),
        CaptureTarget::App { bundle_id } => {
            if bundle_id.trim().is_empty() {
                return Err(unsupported("Choose an app to record"));
            }
            (1, None, None, Some(bundle_id.clone()))
        }
    };
    Ok(Settings {
        output_dir: s.output_dir_in(home),
        container: s.container.as_str().into(),
        codec: match s.codec {
            cs::Codec::Auto => None,
            cs::Codec::H264 => Some(Codec::H264),
            cs::Codec::Hevc => Some(Codec::Hevc),
            cs::Codec::Prores => Some(Codec::Prores),
        },
        resolution: s.resolution.as_str().into(),
        fps: s.fps,
        bitrate_mbps: s.bitrate_mbps,
        display: display.max(1),
        scene: rec_scene(s.scene),
        camera: s.camera_id.clone(),
        mic_enabled: s.mic_enabled,
        mic: s.mic_id.clone(),
        mic_name: None,
        system_audio: s.system_audio,
        audio_tracks: match s.audio_tracks {
            cs::AudioTracks::Both => AudioLayout::Both,
            cs::AudioTracks::Mix => AudioLayout::Mix,
            cs::AudioTracks::Stems => AudioLayout::Stems,
        },
        system_gain_db: s.system_gain_db,
        mic_gain_db: s.mic_gain_db,
        system_muted: s.system_muted,
        mic_muted: s.mic_muted,
        pip_size: s.pip_size,
        pip_place: match &s.pip_place {
            cs::PipPlace::Corner { corner } => Place::Corner(rec_corner(*corner)),
            cs::PipPlace::Free { x, y } => Place::At([*x, *y]),
        },
        pip_shape: rec_shape(s.pip_shape),
        pip_roundness: s.pip_roundness,
        pip_border: s.pip_border,
        pip_border_color: hex_rgb(&s.pip_border_color),
        pip_shadow: s.pip_shadow,
        mirror: s.mirror,
        cursor: s.cursor,
        clicks: s.clicks,
        preview_low_power: s.preview_low_power,
        replay_enabled: s.replay_enabled,
        replay_secs: s.replay_secs,
        stream_dests: s
            .stream_dests
            .iter()
            .map(|d| RecDest {
                dest: match d.service {
                    StreamService::Custom => d.url.clone().unwrap_or_default(),
                    sv => sv.as_str().into(),
                },
                enabled: d.enabled,
            })
            .collect(),
        live_resolution: s.live_resolution.as_str().into(),
        stream_bitrate_kbps: s.stream_bitrate_kbps,
        stream_bitrate_auto: s.stream_bitrate_auto,
        stream_codec: s.stream_codec.as_str().into(),
        keyframe_secs: s.keyframe_secs,
        stream_cbr: s.stream_cbr,
        stream_audio_kbps: s.stream_audio_kbps,
        window,
        region,
        app,
    })
}

/// `rec record` arguments for `s` (`output` None = preview): Rec's own
/// `Settings::record_args`, plus the options Rec's app had no setting for.
pub fn record_args_for(
    s: &RecorderSettings,
    rec: &Settings,
    output: Option<&Path>,
    test_pattern: bool,
) -> Vec<String> {
    let mut a = rec.record_args(output);
    if let Some(ms) = s.system_offset_ms {
        a.extend(["--system-audio-offset-ms".into(), format!("{ms}")]);
    }
    if s.mic_offset_ms != 0.0 {
        a.extend(["--mic-offset-ms".into(), format!("{}", s.mic_offset_ms)]);
    }
    if s.mic_backend == cs::MicBackend::Avcapture {
        a.extend(["--mic-backend".into(), "avcapture".into()]);
    }
    if test_pattern {
        a.push("--test-pattern".into());
    }
    // The raw camera: a second track in a .mov, a sidecar for .mp4.
    if s.camera_track {
        a.extend(["--camera-track".into(), "file".into()]);
    }
    a.extend([
        "--camera-resolution".into(),
        s.camera_resolution.as_str().into(),
    ]);
    a
}

fn audio_track_count(o: &record::Options) -> u32 {
    match (o.system_audio, o.mic.is_some()) {
        (true, true) => match o.audio_layout {
            AudioLayout::Both => 3,
            AudioLayout::Mix => 1,
            AudioLayout::Stems => 2,
        },
        (false, false) => 0,
        _ => 1,
    }
}

fn perm_state(s: tcc::Status) -> PermissionState {
    match s {
        tcc::Status::Granted => PermissionState::Granted,
        tcc::Status::NotDetermined => PermissionState::NotDetermined,
        tcc::Status::Denied | tcc::Status::Restricted => PermissionState::Denied,
    }
}

fn read_permissions() -> Permissions {
    let (screen, mic, cam) = tcc::report();
    Permissions {
        screen: perm_state(screen),
        microphone: perm_state(mic),
        camera: perm_state(cam),
    }
}

/// Permissions as the UI should see them: the test pattern needs no Screen
/// Recording grant, so it reports screen as granted (self-tests, perf runs), and
/// withheld grants report as never asked.
fn state_permissions(opts: &EngineOptions) -> Permissions {
    let mut p = read_permissions();
    if opts.test_pattern {
        p.screen = PermissionState::Granted;
    }
    for w in &opts.withheld {
        p.set(*w, PermissionState::NotDetermined);
    }
    p
}

/// Turn an engine error from `prepare`/`Session::start` into an API error.
fn engine_error(e: anyhow::Error) -> EngineError {
    match e.downcast_ref::<record::MissingPermissions>() {
        Some(m) => EngineError::MissingPermissions {
            permissions: m
                .0
                .iter()
                .map(|n| match *n {
                    "Microphone" => Permission::Microphone,
                    "Camera" => Permission::Camera,
                    _ => Permission::Screen,
                })
                .collect(),
        },
        None => EngineError::failed(format!("{e:#}")),
    }
}

fn free_disk_bytes(dir: &Path) -> u64 {
    // The folder may not exist yet: measure its nearest existing ancestor.
    let Some(dir) = dir.ancestors().find(|d| d.exists()) else {
        return 0;
    };
    let Ok(c) = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()) else {
        return 0;
    };
    // SAFETY: `c` is a valid NUL-terminated path; `st` is a plain C struct.
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return 0;
        }
        st.f_bavail as u64 * st.f_frsize
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// Every capture source (blocking: ScreenCaptureKit lists displays and windows).
pub fn list_devices() -> Devices {
    let infos = screen::displays().unwrap_or_default();
    let displays = infos
        .iter()
        .enumerate()
        .map(|(i, d)| Display {
            id: i as u32 + 1,
            name: format!("Display {}", i + 1),
            width: d.pixels.0 as u32,
            height: d.pixels.1 as u32,
            frame: d.frame,
            scale: d.scale(),
        })
        .collect();
    let all = screen::windows().unwrap_or_default();
    let apps = rec_core::state::apps_from_windows(
        all.iter().map(|w| (w.bundle_id.as_str(), w.app.as_str())),
    );
    let windows = all
        .iter()
        .filter(|w| !w.title.is_empty())
        .map(|w| {
            // Points from the top-left of the display holding most of the window.
            let i = screen::display_index_of(&w.frame, &infos).unwrap_or(0);
            let origin = infos.get(i).map_or((0.0, 0.0), |d| (d.frame.x, d.frame.y));
            CaptureWindow {
                id: w.id,
                app: w.app.clone(),
                title: w.title.clone(),
                bundle_id: w.bundle_id.clone(),
                display_id: i as u32 + 1,
                frame: rec_core::state::Rect {
                    x: w.frame.x - origin.0,
                    y: w.frame.y - origin.1,
                    ..w.frame
                },
            }
        })
        .collect();
    let dev = |d: camera::Device| InputDevice {
        id: d.id,
        name: d.name,
        connected: true,
    };
    Devices {
        displays,
        windows,
        apps,
        cameras: camera::cameras().into_iter().map(dev).collect(),
        mics: camera::microphones().into_iter().map(dev).collect(),
    }
}

// ---- Engine -----------------------------------------------------------------

/// The recording in progress.
struct Active {
    sess: Session,
    meta: Meta,
}

/// What a finished recording needs to describe itself.
#[derive(Clone)]
struct Meta {
    file: PathBuf,
    size: (usize, usize),
    fps: u32,
    audio_tracks: u32,
    created_ms: f64,
    /// The camera is a track in `file` (true) or in this sidecar file.
    camera_in_file: bool,
    camera_sidecar: Option<PathBuf>,
}

struct Finishing {
    handle: std::thread::JoinHandle<Result<Stats, String>>,
    meta: Meta,
    discard: bool,
}

struct Inner {
    opts: EngineOptions,
    settings: RecorderSettings,
    /// `settings` in Rec's shape (None while they can't be recorded, e.g. app capture).
    rec: Option<Settings>,
    state: EngineState,
    countdown_until: Option<Instant>,
    recording: Option<Active>,
    finishing: Option<Finishing>,
    preview: Option<Session>,
    /// Rec's `capture_key` of the settings the preview runs with.
    /// What the running preview was opened with: a change reopens it (the
    /// camera resolution too, so a recording can take over its camera).
    preview_key: Option<(Settings, cs::CameraResolution)>,
    preview_wanted: bool,
    /// Set when the preview failed to start; cleared by a settings change, so
    /// a broken setup isn't retried 30 times a second.
    preview_error: Option<String>,
    /// A running camera handed between sessions so it never restarts dark.
    spare_cam: Option<camera::Camera>,
    live: Arc<live::Live>,
    streams: Vec<(u64, String)>,
    replay_saves: Vec<std::thread::JoinHandle<anyhow::Result<PathBuf>>>,
    pip_applied: Option<crate::compose::PipSpec>,
    volumes_applied: Option<[crate::mixer::Volume; 2]>,
    /// The mic fallback warning last sent, so it isn't repeated.
    warned: Option<String>,
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
        self.state.previewing = self.preview.is_some();
        ev.push(EngineEvent::StateChanged(self.state.clone()));
    }

    fn act(&mut self, a: Action) -> Result<EngineStatus, EngineError> {
        transition(self.state.status, a)
    }

    fn rec(&self) -> Result<&Settings, EngineError> {
        self.rec.as_ref().ok_or_else(|| {
            unsupported("Choose what to record: a display, window, app or area")
        })
    }

    /// Session options for the current settings (the GUI's `App::options`).
    fn options(
        &mut self,
        output: Option<&Path>,
        ev: &mut Events,
    ) -> Result<(record::Options, Option<screen::Target>), EngineError> {
        let mut rec = self.rec()?.clone();
        let mut warning = None;
        if !self.opts.test_pattern {
            // A display that was unplugged falls back to the main one.
            let count = screen::displays().map(|d| d.len()).unwrap_or(0);
            if count > 0 && rec.window.is_none() && rec.app.is_none() && rec.display > count {
                warning = Some(format!(
                    "Display {} not connected; using display 1",
                    rec.display
                ));
                rec.display = 1;
            }
        }
        let mut args = record_args_for(&self.settings, &rec, output, self.opts.test_pattern);
        // The writer caps the file at exactly this length, like `rec record --duration`.
        if let (Some(d), Some(_)) = (self.opts.max_duration_secs, output) {
            args.extend(["--duration".into(), format!("{d}")]);
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut parsed = crate::args::parse_record_args(&refs)
            .map_err(|e| EngineError::failed(format!("{e:#}")))?;
        // Open what the grants allow; only a source the scene can't do without blocks it.
        let plan = plan_sources(
            self.settings.scene,
            self.settings.mic_enabled,
            &state_permissions(&self.opts),
            self.opts.test_pattern,
        )
        .map_err(|permissions| EngineError::MissingPermissions { permissions })?;
        crate::args::apply_plan(&mut parsed, &plan);
        let (mut o, t) = crate::args::prepare(parsed).map_err(engine_error)?;
        // A recording says what it runs without (the preview shows Allow buttons instead).
        if output.is_some()
            && let Some(note) = dropped_note(&plan.dropped)
        {
            ev.push(EngineEvent::Error {
                message: note,
                recoverable: true,
            });
        }
        o.live = Some(self.live.clone());
        // With the mutes, which the CLI doesn't take.
        o.volumes = rec.volumes();
        let warning = [warning, o.mic_warning.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        let warning = Some(warning).filter(|w| !w.is_empty());
        if warning != self.warned {
            if let Some(w) = &warning {
                ev.push(EngineEvent::Error {
                    message: w.clone(),
                    recoverable: true,
                });
            }
            self.warned = warning;
        }
        Ok((o, t))
    }

    fn preview_callback(core: &Core) -> PreviewFn {
        let sink = core.preview_sink.clone();
        let low_power = core.low_power.clone();
        let view_px = core.preview_px.clone();
        let last = Mutex::new(None::<Instant>);
        let scaler = Mutex::new(None::<PreviewScaler>);
        // The writer already limits preview frames to 30 a second.
        Arc::new(move |pb| {
            let Some(f) = sink.lock().unwrap().clone() else {
                return;
            };
            // Low-power preview: at most 15 frames a second.
            if low_power.load(Ordering::Relaxed) {
                let mut l = last.lock().unwrap();
                if l.is_some_and(|t| t.elapsed() < Duration::from_millis(66)) {
                    return;
                }
                *l = Some(Instant::now());
            }
            let packed = view_px.load(Ordering::Relaxed);
            let view = ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize);
            let pb = match scaler.try_lock() {
                Ok(mut s) => PreviewScaler::downscale(&mut s, pb, view),
                Err(_) => pb,
            };
            f(pb)
        })
    }

    fn begin_recording(&mut self, core: &Core, ev: &mut Events) -> Result<(), EngineError> {
        // One capture session at a time: the recording takes over the
        // preview's camera (already exposed for the room).
        let mut cam = self.spare_cam.take();
        if let Some(mut s) = self.preview.take() {
            cam = cam.or_else(|| s.take_camera());
            let _ = s.finish();
        }
        self.preview_key = None;
        let started = (|| {
            let rec = self.rec()?.clone();
            std::fs::create_dir_all(&rec.output_dir).map_err(|e| {
                EngineError::failed(format!(
                    "Could not create {}: {e}",
                    rec.output_dir.display()
                ))
            })?;
            let file = rec.new_output_file();
            let (o, t) = self.options(Some(&file), ev)?;
            let sess =
                Session::start_with_camera(&o, t, true, Some(Self::preview_callback(core)), cam)
                    .map_err(engine_error)?;
            let cam_track = o
                .webcam
                .as_ref()
                .and_then(|_| writer::CamTrackSpec::for_output(o.camera_track, &file, (0, 0)));
            let meta = Meta {
                size: o.size,
                fps: o.fps,
                audio_tracks: audio_track_count(&o),
                created_ms: now_ms(),
                camera_in_file: cam_track.as_ref().is_some_and(|c| c.sidecar.is_none()),
                camera_sidecar: cam_track.and_then(|c| c.sidecar),
                file,
            };
            Ok::<_, EngineError>(Active { sess, meta })
        })();
        match started {
            Ok(a) => {
                self.pip_applied = Some(self.rec()?.pip_spec());
                self.volumes_applied = Some(self.rec()?.volumes());
                self.recording = Some(a);
                self.countdown_until = None;
                self.state.set_progress(&Progress::default());
                self.set_status(EngineStatus::Recording, ev);
                Ok(())
            }
            Err(e) => {
                self.countdown_until = None;
                self.set_status(EngineStatus::Idle, ev);
                self.sync_preview(core, ev);
                Err(e)
            }
        }
    }

    /// Stop the recording; the file is finalized on a background thread.
    fn begin_finalize(&mut self, discard: bool, ev: &mut Events) {
        let Some(Active { mut sess, meta }) = self.recording.take() else {
            return;
        };
        // The camera goes back to the preview once the file is finished.
        self.spare_cam = sess.take_camera();
        let handle = std::thread::spawn(move || {
            objc2::rc::autoreleasepool(|_| sess.finish()).map_err(|e| format!("{e:#}"))
        });
        self.finishing = Some(Finishing {
            handle,
            meta,
            discard,
        });
        self.set_status(EngineStatus::Finalizing, ev);
    }

    fn finish_done(&mut self, core: &Core, ev: &mut Events) {
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
            if let Some(sc) = &f.meta.camera_sidecar {
                let _ = std::fs::remove_file(sc);
            }
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
                        duration_secs: stats.video_frames_out as f64 / m.fps.max(1) as f64,
                        bytes: std::fs::metadata(&m.file).map(|x| x.len()).unwrap_or(0),
                        created_at: m.created_ms,
                        width: m.size.0 as u32,
                        height: m.size.1 as u32,
                        // In-file only: an .mp4's camera is in the sidecar.
                        has_camera_track: m.camera_in_file && stats.cam_track_frames > 0,
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
        self.sync_preview(core, ev);
    }

    fn live_wanted(&self) -> bool {
        self.settings.replay_enabled || !self.streams.is_empty()
    }

    /// Apply the replay and live-encode settings (the GUI's `sync_live`).
    fn sync_live(&mut self) {
        let Some(rec) = &self.rec else { return };
        let cfg = rec.live_config();
        if self.live.config() != cfg {
            self.live.set_config(cfg);
        }
        let want = self
            .settings
            .replay_enabled
            .then(|| self.settings.replay_secs.clamp(15, 300) as f64);
        if want.is_some() || self.live.replay_stats().is_some() {
            self.live.set_replay(want);
        }
        self.state.replay_buffered_secs = self.live.replay_stats().map_or(0.0, |r| r.0);
    }

    /// Run a preview session while wanted and nothing records; restart it when
    /// a capture setting changes (the GUI's `sync_preview`).
    /// A grant changed: re-list cameras and mics (macOS may hide them before a grant), and
    /// rebuild the preview so it picks up (or drops) the source.
    fn permissions_changed(&mut self, core: &Core, ev: &mut Events) {
        core.devices_changed.store(true, Ordering::SeqCst);
        self.preview_error = None;
        if self.preview.is_some() {
            self.preview_key = None;
        }
        self.sync_preview(core, ev);
    }

    fn sync_preview(&mut self, core: &Core, ev: &mut Events) {
        let want = (self.preview_wanted || self.live_wanted())
            && self.recording.is_none()
            && self.finishing.is_none()
            && self.rec.is_some()
            && !core.quit.load(Ordering::SeqCst);
        let key = self
            .rec
            .as_ref()
            .map(|r| (r.capture_key(), self.settings.camera_resolution));
        let stale = self.preview_key != key;
        if self.preview.is_some() && (!want || stale) {
            if let Some(mut s) = self.preview.take() {
                if want {
                    self.spare_cam = s.take_camera();
                }
                let _ = s.finish();
            }
            self.preview_key = None;
        }
        if want && self.preview.is_none() && self.preview_error.is_none() {
            let cam = self.spare_cam.take();
            let r = self.options(None, ev).and_then(|(o, t)| {
                Session::start_with_camera(&o, t, false, Some(Self::preview_callback(core)), cam)
                    .map_err(engine_error)
            });
            match r {
                Ok(s) => {
                    self.pip_applied = self.rec.as_ref().map(Settings::pip_spec);
                    self.volumes_applied = self.rec.as_ref().map(Settings::volumes);
                    self.preview = Some(s);
                }
                Err(e) => {
                    let msg = format!("Preview unavailable: {e}");
                    ev.push(EngineEvent::Error {
                        message: msg.clone(),
                        recoverable: true,
                    });
                    self.preview_error = Some(msg);
                }
            }
            self.preview_key = key;
        }
        // Nothing took the camera: turn it off.
        if self.finishing.is_none()
            && let Some(c) = self.spare_cam.take()
        {
            c.stop();
        }
        if self.state.previewing != self.preview.is_some() {
            self.state.previewing = self.preview.is_some();
            ev.push(EngineEvent::StateChanged(self.state.clone()));
        }
    }

    /// Overlay and fader changes go to the running session live.
    fn apply_live_changes(&mut self) {
        let Some(rec) = &self.rec else { return };
        let Some(sess) = self
            .recording
            .as_ref()
            .map(|a| &a.sess)
            .or(self.preview.as_ref())
        else {
            return;
        };
        let spec = rec.pip_spec();
        if self.pip_applied.as_ref() != Some(&spec) {
            sess.set_pip(spec.clone());
            self.pip_applied = Some(spec);
        }
        let v = rec.volumes();
        if self.volumes_applied != Some(v) {
            sess.set_volumes(v);
            self.volumes_applied = Some(v);
        }
    }

    fn session(&self) -> Option<&Session> {
        self.recording
            .as_ref()
            .map(|a| &a.sess)
            .or(self.preview.as_ref())
    }

    fn tick(&mut self, core: &Core, ev: &mut Events) {
        self.ticks += 1;
        let progress_tick = self.ticks.is_multiple_of(3);
        // Countdown.
        if self.state.status == EngineStatus::Countdown
            && let Some(until) = self.countdown_until
        {
            let left = until
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u32;
            if left == 0 {
                if let Err(e) = self.begin_recording(core, ev) {
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
        // A source or the writer failed: keep what was written.
        if let Some(a) = &self.recording
            && a.sess.failed()
        {
            let err = a.sess.error().unwrap_or_default();
            ev.push(EngineEvent::Error {
                message: format!("Recording stopped: {err}"),
                recoverable: false,
            });
            if self.act(Action::Fail).is_ok() {
                self.begin_finalize(false, ev);
            }
        }
        // Self-tests: stop after the requested recorded time.
        if let (Some(max), Some(a)) = (self.opts.max_duration_secs, &self.recording)
            && a.sess.has_frames()
            && a.sess.recorded_secs() >= max
            && self.act(Action::Stop).is_ok()
        {
            self.begin_finalize(false, ev);
        }
        if self.preview.as_ref().is_some_and(|s| s.failed()) {
            let err = self
                .preview
                .as_ref()
                .and_then(|s| s.error())
                .unwrap_or_default();
            if let Some(s) = self.preview.take() {
                let _ = s.finish();
            }
            let msg = format!("Preview stopped: {err}");
            ev.push(EngineEvent::Error {
                message: msg.clone(),
                recoverable: true,
            });
            self.preview_error = Some(msg);
        }
        self.finish_done(core, ev);
        // Replay saves.
        let (done, running): (Vec<_>, Vec<_>) =
            self.replay_saves.drain(..).partition(|h| h.is_finished());
        self.replay_saves = running;
        for h in done {
            match h.join() {
                Ok(Ok(p)) => ev.push(EngineEvent::ReplaySaved {
                    path: p.display().to_string(),
                }),
                Ok(Err(e)) => ev.push(EngineEvent::Error {
                    message: format!("Replay: {e:#}"),
                    recoverable: true,
                }),
                Err(_) => ev.push(EngineEvent::Error {
                    message: "Replay: the writer crashed".into(),
                    recoverable: true,
                }),
            }
        }
        // Meters (30 Hz while a session runs, then once more to zero them).
        let dt = self.last_levels.elapsed().as_secs_f32();
        self.last_levels = Instant::now();
        let lv = self.session().map(|s| s.poll().1);
        if lv.is_some() || self.state.levels != Levels::default() {
            self.state.levels.feed(lv.unwrap_or([0.0; 2]), dt);
            ev.push(EngineEvent::Levels(self.state.levels));
        }
        if progress_tick {
            self.sync_live();
            self.sync_streams(ev);
            if self.state.status.is_capturing() || self.settings.replay_enabled {
                let p = self.progress();
                self.state.set_progress(&p);
                ev.push(EngineEvent::Progress(p));
            }
        }
        // About once a second: permissions granted in System Settings, disk space.
        if self.ticks.is_multiple_of(30) {
            let p = state_permissions(&self.opts);
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
                self.permissions_changed(core, ev);
            }
            self.state.free_disk_bytes =
                free_disk_bytes(&self.settings.output_dir_in(&self.opts.home));
        }
        self.apply_live_changes();
    }

    fn sync_streams(&mut self, ev: &mut Events) {
        let statuses = self.live.statuses();
        let streams: Vec<StreamStatus> = self
            .streams
            .iter()
            .filter_map(|(id, _)| statuses.iter().find(|(i, _)| i == id))
            .enumerate()
            .map(|(n, (_, s))| {
                // A drop or a failed first connection becomes a toast.
                let was = self.state.streams.get(n);
                let (was_live, was_state) = was.map_or((false, ""), |p| (p.live, p.state.as_str()));
                if let Some(message) = live::stream_alert(was_live, was_state, s) {
                    ev.push(EngineEvent::Error {
                        message,
                        recoverable: true,
                    });
                }
                s
            })
            .map(|s| StreamStatus {
                name: s.name.clone(),
                live: s.live,
                state: s.state.clone(),
                bitrate_kbps: (s.bitrate / 1000.0) as u32,
                dropped_frames: s.dropped_frames,
            })
            .collect();
        let changed = streams.iter().map(|s| (&s.name, s.live, &s.state)).ne(self
            .state
            .streams
            .iter()
            .map(|s| (&s.name, s.live, &s.state)));
        self.state.streams = streams;
        if changed {
            ev.push(EngineEvent::StateChanged(self.state.clone()));
        }
    }

    fn progress(&self) -> Progress {
        let mut p = Progress {
            countdown_left: self.state.countdown_left,
            replay_buffered_secs: self.live.replay_stats().map_or(0.0, |r| r.0),
            ..Default::default()
        };
        if let Some(a) = &self.recording {
            if a.sess.has_frames() {
                p.elapsed_secs = a.sess.recorded_secs();
            }
            p.dropped_frames = a.sess.poll().0.video_drops;
            p.bytes_written = std::fs::metadata(&a.meta.file)
                .map(|m| m.len())
                .unwrap_or(0);
        }
        p
    }
}

struct Core {
    inner: Mutex<Inner>,
    events: EventSink,
    preview_sink: Arc<Mutex<Option<PreviewFn>>>,
    /// The preview view's size in pixels, packed `w << 32 | h` (0 = unknown,
    /// frames go out at the output size).
    preview_px: Arc<AtomicU64>,
    low_power: Arc<AtomicBool>,
    devices_changed: Arc<AtomicBool>,
    quit: AtomicBool,
}

impl Core {
    /// Run `f` under the lock, then send the events it produced.
    fn run<R>(&self, f: impl FnOnce(&mut Inner, &mut Events) -> R) -> R {
        let mut ev = vec![];
        let r = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            objc2::rc::autoreleasepool(|_| f(&mut inner, &mut ev))
        };
        for e in ev {
            (self.events)(e);
        }
        r
    }

    fn tick(&self) {
        // Device lists are read outside the lock (ScreenCaptureKit can be slow).
        if self.devices_changed.swap(false, Ordering::SeqCst) {
            let d = list_devices();
            self.run(|inner, ev| {
                if !self.quit.load(Ordering::SeqCst) {
                    inner.state.set_devices(d.clone());
                    ev.push(EngineEvent::DeviceChanged(d));
                }
            });
        }
        // Checked under the lock: once `shutdown` has set `quit`, a ticker that was held up
        // (in an event sink, say) never touches the state `shutdown` cleaned up.
        self.run(|inner, ev| {
            if !self.quit.load(Ordering::SeqCst) {
                inner.tick(self, ev);
            }
        });
    }
}

/// Widest preview frame worth sending: a preview never needs more pixels.
const PREVIEW_MAX_WIDTH: usize = 1920;

/// The size to scale a `src` preview frame to for a view of `view` pixels,
/// keeping the aspect ratio (even sizes, as 4:2:0 needs). None = send as is
/// (unknown view size, or less than ~10% to gain).
fn preview_target(src: (usize, usize), view: (usize, usize)) -> Option<(usize, usize)> {
    let (sw, sh) = src;
    let (vw, vh) = view;
    if sw == 0 || sh == 0 || vw == 0 || vh == 0 {
        return None;
    }
    let fit = (vw as f64 / sw as f64)
        .min(vh as f64 / sh as f64)
        .min(PREVIEW_MAX_WIDTH as f64 / sw as f64);
    if fit > 0.9 {
        return None;
    }
    let even = |v: f64| ((v / 2.0).round() as usize * 2).max(2);
    Some((even(sw as f64 * fit), even(sh as f64 * fit)))
}

/// Rec's Metal scaler plus a buffer pool, sized for the current preview view.
struct PreviewScaler {
    scaler: crate::scale::FrameScaler,
    pool: objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBufferPool>,
    size: (usize, usize),
    format: u32,
}

// SAFETY: only used behind the preview callback's mutex. CVPixelBufferPool is
// a thread-safe CF type, and FrameScaler is already Send (same rule as the
// writer's scaler).
unsafe impl Send for PreviewScaler {}

impl PreviewScaler {
    /// `pb` scaled down for a `view`-sized preview, or `pb` itself when no
    /// scaling is needed or it fails (the layer then scales it on the GPU).
    fn downscale(
        slot: &mut Option<PreviewScaler>,
        pb: objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer>,
        view: (usize, usize),
    ) -> objc2_core_foundation::CFRetained<objc2_core_video::CVPixelBuffer> {
        use objc2_core_video::{
            CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
            CVPixelBufferPool,
        };
        let src = (CVPixelBufferGetWidth(&pb), CVPixelBufferGetHeight(&pb));
        let Some(size) = preview_target(src, view) else {
            return pb;
        };
        let format = CVPixelBufferGetPixelFormatType(&pb);
        if slot.as_ref().is_none_or(|s| s.size != size || s.format != format) {
            *slot = crate::scale::FrameScaler::new(size.0, size.1).map(|scaler| PreviewScaler {
                scaler,
                pool: crate::util::make_pool(size.0, size.1, format),
                size,
                format,
            });
        }
        let Some(s) = slot.as_mut() else { return pb };
        let mut buf = std::ptr::null_mut();
        // SAFETY: `buf` is a valid out-pointer; the pool is alive.
        unsafe { CVPixelBufferPool::create_pixel_buffer(None, &s.pool, std::ptr::NonNull::from(&mut buf)) };
        let Some(b) = std::ptr::NonNull::new(buf) else { return pb };
        // SAFETY: the pool returned a +1 pixel buffer.
        let out = unsafe { objc2_core_foundation::CFRetained::from_raw(b) };
        if !s.scaler.scale(&pb, &out) {
            return pb;
        }
        crate::scale::tag_bt709(&out);
        out
    }
}

/// How long `shutdown` waits for the ticker thread before finalizing without it.
const TICKER_STOP_WAIT: Duration = Duration::from_millis(500);

/// The macOS recorder engine.
pub struct MacEngine {
    core: Arc<Core>,
    ticker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl MacEngine {
    /// Start the engine. `events` receives every `EngineEvent` (from any thread).
    pub fn new(opts: EngineOptions, settings: RecorderSettings, events: EventSink) -> MacEngine {
        let settings = settings.validated();
        let rec = rec_settings_for(&settings, &opts.home).ok();
        let live = live::Live::new();
        if let Some(r) = &rec {
            live.set_config(r.live_config());
        }
        let mut state = EngineState {
            permissions: state_permissions(&opts),
            ..Default::default()
        };
        state.free_disk_bytes = free_disk_bytes(&settings.output_dir_in(&opts.home));
        state.output_latency_ms = crate::outlatency::output_latency().map(|s| s * 1000.0);
        let low_power = Arc::new(AtomicBool::new(settings.preview_low_power));
        let inner = Inner {
            opts,
            settings,
            rec,
            state,
            countdown_until: None,
            recording: None,
            finishing: None,
            preview: None,
            preview_key: None,
            preview_wanted: false,
            preview_error: None,
            spare_cam: None,
            live,
            streams: vec![],
            replay_saves: vec![],
            pip_applied: None,
            volumes_applied: None,
            warned: None,
            ticks: 0,
            last_levels: Instant::now(),
        };
        let core = Arc::new(Core {
            inner: Mutex::new(inner),
            events,
            preview_sink: Arc::new(Mutex::new(None)),
            preview_px: Arc::new(AtomicU64::new(0)),
            low_power,
            // The first tick lists the devices.
            devices_changed: Arc::new(AtomicBool::new(true)),
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
        MacEngine {
            core,
            ticker: Mutex::new(ticker),
        }
    }

    /// Where preview frames go (None stops delivery). Called on a capture
    /// thread for every frame, so it must return quickly (hand the
    /// CVPixelBuffer to a Metal layer, don't process it there).
    pub fn set_preview_sink(&self, sink: Option<PreviewFn>) {
        *self.core.preview_sink.lock().unwrap() = sink;
    }

    /// The preview view's size in device pixels (0 × 0 = unknown). Preview
    /// frames larger than this (or than 1920 wide) are scaled down to it
    /// before they reach the sink, so the window server composites a small
    /// buffer instead of a 4K one.
    pub fn set_preview_pixel_size(&self, width: u32, height: u32) {
        let packed = (u64::from(width) << 32) | u64::from(height);
        self.core.preview_px.store(packed, Ordering::Relaxed);
    }

    /// Watch for cameras and mics being plugged in or removed
    /// (`DeviceChanged` events). Call once, on the main thread.
    pub fn watch_devices(&self) {
        let flag = self.core.devices_changed.clone();
        camera::on_device_change(move || flag.store(true, Ordering::SeqCst));
    }

    /// Store a stream key in the Keychain (never in settings, never logged).
    pub fn set_stream_key(&self, dest: &cs::StreamDest, key: &str) -> Result<(), EngineError> {
        let url = dest_url(dest);
        let key = key.trim();
        if url.is_empty() || key.is_empty() {
            return Err(EngineError::failed("Enter a server URL and a stream key"));
        }
        live::keychain::set(&url, key)
            .map_err(|e| EngineError::failed(format!("Could not save the stream key: {e:#}")))
    }

    pub fn remove_stream_key(&self, dest: &cs::StreamDest) {
        let _ = live::keychain::delete(&dest_url(dest));
    }

    /// `settings` with each destination's `key_stored` read from the Keychain.
    pub fn with_key_status(&self, mut settings: RecorderSettings) -> RecorderSettings {
        for d in &mut settings.stream_dests {
            let url = dest_url(d);
            d.key_stored = !url.is_empty() && live::keychain::get(&url).is_some();
        }
        settings
    }
}

impl RecorderEngine for MacEngine {
    fn state(&self) -> EngineState {
        self.core.run(|i, _| {
            let mut s = i.state.clone();
            s.previewing = i.preview.is_some();
            s
        })
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
            let rec = rec_settings_for(&settings, &i.opts.home)?;
            i.settings = settings;
            i.rec = Some(rec);
            core.low_power
                .store(i.settings.preview_low_power, Ordering::Relaxed);
            i.sync_live();
            if countdown {
                i.countdown_until =
                    Some(Instant::now() + Duration::from_secs(i.settings.countdown as u64));
                i.state.countdown_left = i.settings.countdown;
                i.set_status(EngineStatus::Countdown, ev);
                Ok(())
            } else {
                i.begin_recording(core, ev)
            }
        })
    }

    fn cancel_countdown(&self) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            let s = i.act(Action::CancelCountdown)?;
            i.countdown_until = None;
            i.set_status(s, ev);
            i.sync_preview(core, ev);
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
            // Pausing waits for the first frame (the timeline starts there).
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
        let core = &*self.core;
        core.run(|i, ev| {
            let s = i.act(Action::Discard)?;
            if s == EngineStatus::Idle {
                i.countdown_until = None;
                i.set_status(s, ev);
                i.sync_preview(core, ev);
            } else {
                i.begin_finalize(true, ev);
            }
            Ok(())
        })
    }

    fn update_settings(&self, settings: &RecorderSettings) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            let settings = settings.clone().validated();
            let rec = rec_settings_for(&settings, &i.opts.home);
            let changed = settings != i.settings;
            i.settings = settings;
            core.low_power
                .store(i.settings.preview_low_power, Ordering::Relaxed);
            if changed {
                i.preview_error = None;
            }
            // A setting this engine can't record (app capture) stops the
            // preview until it changes; a running recording keeps its own.
            let r = match rec {
                Ok(r) => {
                    i.rec = Some(r);
                    Ok(())
                }
                Err(e) => {
                    if i.recording.is_none() {
                        i.rec = None;
                    }
                    Err(e)
                }
            };
            i.sync_live();
            i.apply_live_changes();
            i.sync_preview(core, ev);
            r
        })
    }

    fn set_preview_enabled(&self, enabled: bool) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            i.preview_wanted = enabled;
            if enabled {
                i.preview_error = None;
            }
            i.sync_preview(core, ev);
            match &i.preview_error {
                Some(e) if enabled && i.recording.is_none() => Err(EngineError::failed(e.clone())),
                _ => Ok(()),
            }
        })
    }

    fn save_replay(&self) -> Result<(), EngineError> {
        self.core.run(|i, _| {
            if i.live.replay_stats().is_none() {
                return Err(EngineError::failed("The replay buffer is off"));
            }
            let dir = i.settings.output_dir_in(&i.opts.home);
            std::fs::create_dir_all(&dir).map_err(|e| {
                EngineError::failed(format!("Could not create {}: {e}", dir.display()))
            })?;
            let h = i
                .live
                .save_replay(dir.join(crate::settings::timestamped("replay", "mp4")))
                .map_err(|e| EngineError::failed(format!("Replay: {e:#}")))?;
            i.replay_saves.push(h);
            Ok(())
        })
    }

    fn go_live(&self) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            if !i.streams.is_empty() {
                return Ok(());
            }
            let urls = i.rec()?.stream_urls();
            if urls.is_empty() {
                return Err(EngineError::failed("Turn on a stream destination first"));
            }
            let start = |url: &str| {
                live::resolve_destination(url).and_then(|(u, key)| live::start_output(&u, &key))
            };
            let added = i
                .live
                .add_outputs(&urls, start)
                .map_err(|e| EngineError::failed(format!("{e:#}")))?;
            i.streams = added;
            i.state.live = true;
            // The live encode needs a capture session.
            i.sync_preview(core, ev);
            ev.push(EngineEvent::StateChanged(i.state.clone()));
            Ok(())
        })
    }

    fn stop_live(&self) -> Result<(), EngineError> {
        let core = &*self.core;
        core.run(|i, ev| {
            for (id, _) in std::mem::take(&mut i.streams) {
                i.live.remove_output(id);
            }
            i.state.live = false;
            i.state.streams.clear();
            i.sync_preview(core, ev);
            ev.push(EngineEvent::StateChanged(i.state.clone()));
            Ok(())
        })
    }

    fn devices(&self) -> Devices {
        let d = list_devices();
        // Every window sees the fresh list (the region picker asks when it opens).
        self.core.run(|i, ev| {
            i.state.set_devices(d.clone());
            ev.push(EngineEvent::DeviceChanged(d.clone()));
        });
        d
    }

    fn permission(&self, p: Permission) -> PermissionState {
        self.core.run(|i, _| state_permissions(&i.opts)).get(p)
    }

    fn request_permission(&self, p: Permission) -> PermissionState {
        // Blocks until the prompt is answered (up to a minute); not under the lock.
        let kind = match p {
            Permission::Screen => tcc::Kind::Screen,
            Permission::Microphone => tcc::Kind::Mic,
            Permission::Camera => tcc::Kind::Camera,
        };
        let core = &*self.core;
        if core.run(|i, _| i.opts.withheld.contains(&p)) {
            return PermissionState::NotDetermined;
        }
        let state = perm_state(tcc::request_in_app(kind, Duration::from_secs(60)));
        core.run(|i, ev| {
            if i.state.permissions.get(p) != state {
                i.state.permissions.set(p, state);
                ev.push(EngineEvent::PermissionChanged {
                    permission: p,
                    state,
                });
                i.permissions_changed(core, ev);
            }
        });
        state
    }

    fn shutdown(&self) {
        if self.core.quit.swap(true, Ordering::SeqCst) {
            return;
        }
        // The ticker may be inside an event sink that waits on the thread calling this (the
        // app's main thread, at quit). Joining it unconditionally deadlocked quit, so wait a
        // bounded time and then let it go: it exits on its own once the sink returns, and
        // the `quit` check in `Core::tick` keeps it away from the state cleaned up below.
        if let Some(t) = self.ticker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let deadline = Instant::now() + TICKER_STOP_WAIT;
            while !t.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if t.is_finished() {
                let _ = t.join();
            } else {
                eprintln!(
                    "rec-engine: the ticker did not stop within {} ms; finalizing without it",
                    TICKER_STOP_WAIT.as_millis()
                );
            }
        }
        let core = &*self.core;
        core.run(|i, ev| {
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
            i.finish_done(core, ev);
            if let Some(s) = i.preview.take() {
                let _ = s.finish();
            }
            if let Some(c) = i.spare_cam.take() {
                c.stop();
            }
            i.live.stop_all();
            for h in i.replay_saves.drain(..) {
                let _ = h.join();
            }
        });
    }
}

impl Drop for MacEngine {
    /// Recordings in progress are finalized, never lost.
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_frames_shrink_to_the_view() {
        // 4K output in a 1200×675-point view on a 2x display: 2400 px is
        // capped at 1920 wide.
        assert_eq!(preview_target((3840, 2160), (2400, 1350)), Some((1920, 1080)));
        // A small view gets a small buffer, aspect kept, even sizes.
        assert_eq!(preview_target((3840, 2160), (800, 600)), Some((800, 450)));
        // Fitting by height when the view is taller-than-wide relative to the frame.
        assert_eq!(preview_target((1920, 1080), (1920, 540)), Some((960, 540)));
        // Nothing to gain (<10% smaller) or unknown size: send as is.
        assert_eq!(preview_target((1920, 1080), (1800, 1013)), None);
        assert_eq!(preview_target((1920, 1080), (0, 0)), None);
        assert_eq!(preview_target((1280, 720), (2560, 1440)), None);
    }

    fn args(s: &RecorderSettings, test_pattern: bool) -> Vec<String> {
        let rec = rec_settings_for(s, Path::new("/Users/u")).unwrap();
        let a = record_args_for(s, &rec, Some(Path::new("/tmp/x.mov")), test_pattern);
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        crate::args::parse_record_args(&refs).expect("settings produce valid CLI arguments");
        a
    }

    fn has(a: &[String], pair: [&str; 2]) -> bool {
        a.windows(2).any(|w| w == pair)
    }

    #[test]
    fn default_settings_map_to_recs_cli() {
        let s = RecorderSettings::default();
        let a = args(&s, false);
        for p in [
            ["-r", "1080p"],
            ["--fps", "60"],
            ["--display", "1"],
            ["--scene", "screen-cam"],
            ["--webcam", "default"],
            ["--audio-tracks", "both"],
            ["--pip-corner", "bottom-right"],
            ["--pip-shape", "squircle"],
            ["--pip-roundness", "0.35"],
            ["--pip-border-color", "ffffff"],
            ["-o", "/tmp/x.mov"],
            ["--camera-track", "file"],
            ["--camera-resolution", "1080p"],
        ] {
            assert!(has(&a, p), "{p:?} in {a:?}");
        }
        assert!(a.contains(&"--pip-shadow".to_string()));
        assert!(!a.iter().any(|x| x == "--codec"
            || x == "--no-mic"
            || x == "--test-pattern"
            || x == "--region"));
        let rec = rec_settings_for(&s, Path::new("/Users/u")).unwrap();
        assert_eq!(rec.output_dir, PathBuf::from("/Users/u/Movies/Cortex"));
    }

    #[test]
    fn camera_track_settings_reach_the_session_options() {
        let s = RecorderSettings {
            camera_track: false,
            camera_resolution: cs::CameraResolution::R720p,
            ..Default::default()
        };
        let a = args(&s, false);
        assert!(!a.iter().any(|x| x == "--camera-track"));
        assert!(has(&a, ["--camera-resolution", "720p"]));
        // A .mov keeps the camera in the file; an .mp4 gets the sidecar.
        let mov = writer::CamTrackSpec::for_output(
            writer::CameraTrack::File,
            Path::new("/m/a.mov"),
            (1920, 1080),
        );
        assert!(mov.is_some_and(|c| c.sidecar.is_none() && c.width == 1920));
        let mp4 = writer::CamTrackSpec::for_output(
            writer::CameraTrack::File,
            Path::new("/m/a.mp4"),
            (1920, 1080),
        );
        assert_eq!(
            mp4.and_then(|c| c.sidecar),
            Some(PathBuf::from("/m/a.camera.mov"))
        );
        assert!(
            writer::CamTrackSpec::for_output(
                writer::CameraTrack::Off,
                Path::new("/m/a.mov"),
                (1, 1)
            )
            .is_none()
        );
    }

    #[test]
    fn representative_settings_map_to_recs_cli() {
        let s = RecorderSettings {
            codec: cs::Codec::Hevc,
            resolution: cs::Resolution::R4k,
            fps: 30,
            bitrate_mbps: Some(40.0),
            target: CaptureTarget::Region {
                display_id: 2,
                x: 10.0,
                y: 20.0,
                w: 640.0,
                h: 360.0,
            },
            scene: cs::Scene::Screen,
            mic_enabled: false,
            system_audio: false,
            system_offset_ms: Some(-12.5),
            mic_offset_ms: 40.0,
            mic_backend: cs::MicBackend::Avcapture,
            pip_place: cs::PipPlace::Free { x: 0.25, y: 0.75 },
            pip_border_color: "0AC81E".into(),
            mirror: false,
            cursor: false,
            clicks: true,
            ..Default::default()
        };
        let a = args(&s, true);
        for p in [
            ["--codec", "hevc"],
            ["-r", "4k"],
            ["--fps", "30"],
            ["--bitrate", "40"],
            ["--display", "2"],
            ["--region", "10,20,640,360"],
            ["--scene", "screen"],
            ["--pip-pos", "0.25,0.75"],
            ["--pip-border-color", "0ac81e"],
            ["--system-audio-offset-ms", "-12.5"],
            ["--mic-offset-ms", "40"],
            ["--mic-backend", "avcapture"],
        ] {
            assert!(has(&a, p), "{p:?} in {a:?}");
        }
        for f in [
            "--no-mic",
            "--no-system-audio",
            "--no-mirror",
            "--no-cursor",
            "--clicks",
            "--test-pattern",
        ] {
            assert!(a.iter().any(|x| x == f), "{f} in {a:?}");
        }
        assert!(
            !a.iter().any(|x| x == "--webcam"),
            "screen scene has no camera"
        );
        let w = args(
            &RecorderSettings {
                target: CaptureTarget::Window { window_id: 42 },
                mic_id: Some("BuiltInMic".into()),
                ..Default::default()
            },
            false,
        );
        assert!(has(&w, ["--window", "42"]) && has(&w, ["--mic", "BuiltInMic"]));
    }

    #[test]
    fn app_and_region_targets_map_to_recs_cli_and_restart_the_preview() {
        let app = RecorderSettings {
            target: CaptureTarget::App {
                bundle_id: "com.apple.Safari".into(),
            },
            ..Default::default()
        };
        let a = args(&app, false);
        assert!(has(&a, ["--app", "com.apple.Safari"]), "{a:?}");
        assert!(!a.iter().any(|x| x == "--window" || x == "--region"));
        let empty = RecorderSettings {
            target: CaptureTarget::App {
                bundle_id: " ".into(),
            },
            ..Default::default()
        };
        assert!(matches!(
            rec_settings_for(&empty, Path::new("/")),
            Err(EngineError::Unsupported { .. })
        ));
        // A new area or app needs a new capture session (the preview follows it).
        let region = |x| RecorderSettings {
            target: CaptureTarget::Region {
                display_id: 1,
                x,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
            ..Default::default()
        };
        let k = |s: &RecorderSettings| rec_settings_for(s, Path::new("/")).unwrap().capture_key();
        assert_ne!(k(&region(0.0)), k(&region(5.0)));
        assert_ne!(k(&region(0.0)), k(&app));
    }

    #[test]
    fn live_settings_reach_recs_live_config() {
        let s = RecorderSettings {
            stream_dests: vec![
                cs::StreamDest {
                    service: StreamService::Youtube,
                    enabled: false,
                    ..Default::default()
                },
                cs::StreamDest {
                    service: StreamService::Twitch,
                    enabled: true,
                    ..Default::default()
                },
                cs::StreamDest {
                    service: StreamService::Custom,
                    url: Some("rtmp://a/live".into()),
                    enabled: true,
                    key_stored: false,
                },
            ],
            ..Default::default()
        };
        let rec = rec_settings_for(&s, Path::new("/")).unwrap();
        assert_eq!(
            rec.stream_urls(),
            ["rtmps://live.twitch.tv:443/app", "rtmp://a/live"]
        );
        assert_eq!(rec.live_config().height, 1080);
        assert_eq!(dest_url(&s.stream_dests[2]), "rtmp://a/live");
    }

    #[test]
    fn overlay_and_fader_changes_keep_the_capture_key() {
        let a = RecorderSettings::default();
        let b = RecorderSettings {
            pip_size: 0.3,
            mic_gain_db: -6.0,
            pip_shape: cs::PipShape::Circle,
            ..a.clone()
        };
        let k = |s: &RecorderSettings| rec_settings_for(s, Path::new("/")).unwrap().capture_key();
        assert_eq!(k(&a), k(&b));
        assert_ne!(k(&a), k(&RecorderSettings { fps: 30, ..a }));
    }
}
