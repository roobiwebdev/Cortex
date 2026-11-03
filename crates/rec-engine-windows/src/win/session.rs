//! SOURCE OF TRUTH KEYWORDS: WinSession, start_session, video thread, pacer, audio writer, overlay image, test pattern, pause, finish
//! WHAT:  One recording on Windows: resolves the target (display, area, window, or an
//!        app's main window), starts the capture, camera and audio sources, and runs two
//!        threads: the video pacer (newest frame → crop → camera overlay → NV12 → encoder at
//!        a constant frame rate, repeating the last frame over gaps) and the audio writer
//!        (faders → mixers → pause cuts → AAC tracks). `finish` stops everything and writes
//!        the file.
//! WHY:   The pacer and the timeline rules are the macOS writer's (`timeline.rs`): the
//!        timeline starts at the first captured frame, every slot gets a frame, a pause
//!        cuts whole slots and audio follows the same cuts, so A/V stay in sync and the
//!        file is CFR. Every time stamp is on the QPC clock.
//! WHERE: `WinBackend::start` (through the engine's `Backend` trait).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rec_core::api::EngineError;
use rec_core::settings::{CameraResolution, CaptureTarget, PipShape, RecorderSettings, Scene};
use rec_core::state::Rect;

use super::audio::{AudioCapture, Endpoint};
use super::camera::{CamFrame, Camera};
use super::capture::{Capture, Source as CaptureSource};
use super::compose::{Compositor, RING};
use super::d3d::Gpu;
use super::devices::{hwnd, listed_windows, monitors};
use super::encoder::Encoder;
use super::shell::exclude_own_windows;
use super::util::{com_init, msg, qpc_secs};
use crate::audio::{
    Fader, Mixer, RATE, Source, TrackRole, Volume, peak, to_pcm16, track_layout, track_sources,
};
use crate::caps::Probe;
use crate::devices::main_window;
use crate::engine::{Session, SessionInfo, SessionRequest, SessionStats, Started};
use crate::overlay::{CamImage, OverlayRenderer, OverlaySpec, border_px, overlay_rect};
use crate::pattern::{pattern_frame, pattern_tone};
use crate::plan::{CropRect, crop_pixels, encoder_config};
use crate::timeline::{
    FrameClock, Pauses, TrackClock, audio_keep, slot_duration, slot_time, slots_due,
};

/// The test pattern's frame size.
const PATTERN_SIZE: (u32, u32) = (1920, 1080);
/// A pacer that wakes this many slots late fills at most this many (the rest are drops).
const MAX_CATCH_UP: usize = 8;

/// Moves a value with COM pointers to the thread that will own it.
struct SendBox<T>(T);
// SAFETY: used only to hand the capture, compositor and textures to the video thread,
// which is then their sole user. The D3D11 device is multithread-protected and the WinRT
// capture objects are agile.
unsafe impl<T> Send for SendBox<T> {}

/// The camera overlay, shared by the camera thread (renders) and the video thread (uploads).
struct OverlayState {
    spec: OverlaySpec,
    /// Camera-only scene: the camera fills the frame.
    cam_only: bool,
    canvas: (u32, u32),
    out: (u32, u32),
    /// Premultiplied BGRA, its size, and where it goes on the canvas.
    image: Option<(Vec<u8>, u32, u32, [f32; 4])>,
    seq: u64,
}

/// What to render for one camera frame: the spec, the size in canvas pixels, the border,
/// and the rectangle on the canvas. None until the frame sizes are known.
fn overlay_job(
    o: &OverlayState,
    cam: (u32, u32),
) -> Option<(OverlaySpec, u32, u32, f64, [f32; 4])> {
    let (ow, oh) = (o.out.0 as f64, o.out.1 as f64);
    if ow <= 0.0 || oh <= 0.0 || o.canvas.0 == 0 {
        return None;
    }
    let (spec, rect) = if o.cam_only {
        let spec = OverlaySpec {
            shape: PipShape::Rect,
            border: 0.0,
            ..o.spec.clone()
        };
        (spec, [0.0, 0.0, ow, oh])
    } else {
        let aspect = cam.0 as f64 / cam.1.max(1) as f64;
        (o.spec.clone(), overlay_rect(&o.spec, ow, oh, aspect))
    };
    // The canvas is the capture's size; the overlay is placed in output pixels.
    let k = o.canvas.0 as f64 / ow;
    let r = [rect[0] * k, rect[1] * k, rect[2] * k, rect[3] * k];
    let (w, h) = (r[2].round().max(1.0) as u32, r[3].round().max(1.0) as u32);
    let border = border_px(&spec, oh) * k;
    Some((
        spec,
        w,
        h,
        border,
        [r[0] as f32, r[1] as f32, w as f32, h as f32],
    ))
}

/// The camera thread's callback: render outside the lock, then publish.
fn camera_sink(sh: Arc<Shared>) -> Box<dyn FnMut(CamFrame) + Send> {
    let mut renderer = OverlayRenderer::default();
    Box::new(move |f: CamFrame| {
        let job = sh
            .overlay
            .lock()
            .ok()
            .and_then(|o| overlay_job(&o, (f.width, f.height)));
        let Some((spec, w, h, border, rect)) = job else {
            return;
        };
        let cam = CamImage {
            width: f.width,
            height: f.height,
            stride: f.width as usize * 4,
            data: &f.data,
        };
        let img = renderer.render(&spec, &cam, w, h, border);
        if let Ok(mut o) = sh.overlay.lock() {
            o.image = Some((img, w, h, rect));
            o.seq += 1;
        }
    })
}

/// State the session's threads share.
struct Shared {
    stop: AtomicBool,
    fps: u32,
    clock: Mutex<Option<FrameClock>>,
    pauses: Mutex<Pauses>,
    frames: AtomicU64,
    out_slots: AtomicU64,
    drops: AtomicU64,
    /// Peak [system, mic] since the last read, as f32 bits.
    levels: [AtomicU32; 2],
    error: Mutex<Option<String>>,
    volumes: Mutex<[Volume; 2]>,
    overlay: Mutex<OverlayState>,
    max_slots: Option<u64>,
}

impl Shared {
    fn fail(&self, e: String) {
        if let Ok(mut g) = self.error.lock()
            && g.is_none()
        {
            *g = Some(e);
        }
    }

    fn clock(&self) -> Option<FrameClock> {
        self.clock.lock().ok().and_then(|c| *c)
    }

    fn note_level(&self, i: usize, v: f32) {
        let slot = &self.levels[i];
        let mut cur = slot.load(Ordering::Relaxed);
        while f32::from_bits(cur) < v {
            match slot.compare_exchange(cur, v.to_bits(), Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(x) => cur = x,
            }
        }
    }
}

fn volumes(s: &RecorderSettings) -> [Volume; 2] {
    [
        Volume {
            db: s.system_gain_db,
            muted: s.system_muted,
        },
        Volume {
            db: s.mic_gain_db,
            muted: s.mic_muted,
        },
    ]
}

/// Where frames come from.
enum VideoSource {
    Capture(SendBox<Capture>),
    Pattern,
    /// Camera-only scene: the overlay fills a black canvas.
    CameraOnly,
}

enum AudioMsg {
    Data(Source, f64, f64, Vec<[f32; 2]>),
}

/**
 * SOURCE OF TRUTH KEYWORDS: WinSession, Session implementation, finish, pause, levels
 * WHAT:  A running Windows recording (see the module header).
 * WHY:   The engine drives every platform's recording through `engine::Session`.
 * WHERE: Returned by `start`.
 */
pub struct WinSession {
    shared: Arc<Shared>,
    video: Option<JoinHandle<()>>,
    audio: Option<JoinHandle<()>>,
    audio_tx: Option<Sender<AudioMsg>>,
    sources: Vec<AudioCapture>,
    tone: Option<JoinHandle<()>>,
    camera: Option<Camera>,
    encoder: Arc<Mutex<Option<Encoder>>>,
}

/// The capture source for a target, the crop inside it (None = all of it), and a note.
fn resolve_target(
    target: &CaptureTarget,
) -> Result<(CaptureSource, Option<CropRect>, Option<String>), EngineError> {
    let mons = monitors();
    let monitor = |id: u32| {
        let i = (id.max(1) - 1) as usize;
        match mons.get(i) {
            Some(m) => Ok((m.clone(), None)),
            None => mons
                .first()
                .cloned()
                .map(|m| {
                    (
                        m,
                        Some(format!("Display {id} not connected; using display 1")),
                    )
                })
                .ok_or_else(|| EngineError::failed("No display found")),
        }
    };
    match target {
        CaptureTarget::Display { display_id } => {
            let (m, note) = monitor(*display_id)?;
            Ok((CaptureSource::Monitor(m.handle), None, note))
        }
        CaptureTarget::Region {
            display_id,
            x,
            y,
            w,
            h,
        } => {
            let (m, note) = monitor(*display_id)?;
            let px = (m.facts.rect.w.round() as u32, m.facts.rect.h.round() as u32);
            let crop = crop_pixels(
                Rect {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                },
                m.facts.scale,
                px,
            )
            .ok_or_else(|| {
                EngineError::failed(format!("The area is outside display {display_id}"))
            })?;
            Ok((CaptureSource::Monitor(m.handle), Some(crop), note))
        }
        CaptureTarget::Window { window_id } => {
            Ok((CaptureSource::Window(hwnd(*window_id)), None, None))
        }
        CaptureTarget::App { bundle_id } => {
            let windows = listed_windows();
            let w = main_window(bundle_id, &windows)
                .ok_or_else(|| EngineError::failed("That app has no open window to record"))?;
            Ok((
                CaptureSource::Window(hwnd(w.id)),
                None,
                Some(format!(
                    "Windows can't record every window of an app: recording \"{}\" only",
                    w.title
                )),
            ))
        }
    }
}

fn camera_cap(r: CameraResolution) -> u32 {
    match r {
        CameraResolution::R720p => 720,
        CameraResolution::R1080p => 1080,
    }
}

fn even(v: u32) -> u32 {
    (v & !1).max(2)
}

/// Start a recording for `req` on `gpu`.
pub fn start(
    req: &SessionRequest,
    gpu: &Gpu,
    probe: &Probe,
    fragmented: bool,
) -> Result<Started, EngineError> {
    com_init();
    super::util::mf_startup().map_err(EngineError::failed)?;
    let s = &req.settings;
    let mut notes: Vec<String> = vec![];
    let cam_only = s.scene == Scene::Cam;
    let want_camera = req.plan.camera && s.scene != Scene::Screen;

    if req.exclude_self && exclude_own_windows() > 0 {
        notes.push(if probe.capture_exclusion() {
            "Some Cortex windows could not be hidden and may appear in the recording".into()
        } else {
            "This version of Windows can't hide Cortex's windows from the recording (Windows 10 2004 or later can)".into()
        });
    }

    // Video source and the part of it to record.
    let (source, crop) = if req.test_pattern {
        (
            VideoSource::Pattern,
            CropRect::full(PATTERN_SIZE.0, PATTERN_SIZE.1),
        )
    } else if cam_only {
        // Sized from the camera once it is open.
        (VideoSource::CameraOnly, CropRect::full(0, 0))
    } else {
        let (src, crop, note) = resolve_target(&s.target)?;
        notes.extend(note);
        let cap = Capture::start(gpu, src, s.cursor, probe.borderless())
            .map_err(|e| EngineError::failed(msg("Could not start the screen capture", &e)))?;
        let size = cap.size();
        let crop = crop.unwrap_or_else(|| CropRect::full(even(size.0), even(size.1)));
        (VideoSource::Capture(SendBox(cap)), crop)
    };
    finish_start(
        req,
        gpu,
        probe,
        fragmented,
        source,
        crop,
        want_camera,
        notes,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_start(
    req: &SessionRequest,
    gpu: &Gpu,
    probe: &Probe,
    fragmented: bool,
    source: VideoSource,
    mut crop: CropRect,
    want_camera: bool,
    mut notes: Vec<String>,
) -> Result<Started, EngineError> {
    let s = &req.settings;
    let cam_only = matches!(source, VideoSource::CameraOnly);
    let shared = Arc::new(Shared {
        stop: AtomicBool::new(false),
        fps: s.fps,
        clock: Mutex::new(None),
        pauses: Mutex::new(Pauses::default()),
        frames: AtomicU64::new(0),
        out_slots: AtomicU64::new(0),
        drops: AtomicU64::new(0),
        levels: [AtomicU32::new(0), AtomicU32::new(0)],
        error: Mutex::new(None),
        volumes: Mutex::new(volumes(s)),
        overlay: Mutex::new(OverlayState {
            spec: OverlaySpec::from_settings(s),
            cam_only,
            canvas: (0, 0),
            out: (0, 0),
            image: None,
            seq: 0,
        }),
        max_slots: req
            .max_duration_secs
            .map(|d| (d * f64::from(s.fps.max(1))).round().max(1.0) as u64),
    });

    // Camera.
    let camera = if want_camera {
        let on_frame = camera_sink(shared.clone());
        match Camera::start(
            s.camera_id.clone(),
            camera_cap(s.camera_resolution),
            on_frame,
        ) {
            Ok(c) => Some(c),
            Err(e) if cam_only => return Err(EngineError::failed(e)),
            Err(e) => {
                notes.push(format!("Recording without the camera: {e}"));
                None
            }
        }
    } else {
        None
    };
    if cam_only {
        let size = camera.as_ref().map_or((1280, 720), |c| c.size);
        crop = CropRect::full(even(size.0), even(size.1));
    }

    // Encoder settings from the frame size.
    let (cfg, note) = encoder_config(s, (crop.w, crop.h), probe.hevc)?;
    notes.extend(note);
    if let Ok(mut o) = shared.overlay.lock() {
        o.canvas = (crop.w, crop.h);
        o.out = (cfg.width, cfg.height);
    }

    // Audio sources; a device that fails to open is left out with a note.
    let (tx, rx) = std::sync::mpsc::channel::<AudioMsg>();
    let mut sources = vec![];
    let mut tone = None;
    let mut has = [false, false];
    let deliver = |src: Source, tx: Sender<AudioMsg>| -> super::audio::Deliver {
        Box::new(move |t, rate, frames| {
            let _ = tx.send(AudioMsg::Data(src, t, rate, frames));
        })
    };
    if req.test_pattern && s.system_audio {
        let (sh, tx) = (shared.clone(), tx.clone());
        tone = std::thread::Builder::new()
            .name("pattern-tone".into())
            .spawn(move || tone_loop(&sh, &tx))
            .ok();
        has[0] = tone.is_some();
    } else if s.system_audio {
        match AudioCapture::start(Endpoint::Loopback, deliver(Source::System, tx.clone())) {
            Ok(c) => {
                sources.push(c);
                has[0] = true;
            }
            Err(e) => notes.push(format!("Recording without system audio: {e}")),
        }
    }
    if req.plan.mic {
        match AudioCapture::start(
            Endpoint::Mic(s.mic_id.clone()),
            deliver(Source::Mic, tx.clone()),
        ) {
            Ok(c) => {
                sources.push(c);
                has[1] = true;
            }
            Err(e) => notes.push(format!("Recording without the microphone: {e}")),
        }
    }
    let roles = track_layout(has[0], has[1], s.audio_tracks);

    // The file: hardware path first, then system memory.
    let encoder = match Encoder::new(&req.file, &cfg, roles.len(), Some(gpu), fragmented) {
        Ok(e) => e,
        Err(first) => {
            let _ = std::fs::remove_file(&req.file);
            match Encoder::new(&req.file, &cfg, roles.len(), None, fragmented) {
                Ok(e) => {
                    notes.push(format!(
                        "The GPU encoder is unavailable ({}); using the software encoder",
                        first.message()
                    ));
                    e
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&req.file);
                    return Err(EngineError::failed(msg("Could not start the encoder", &e)));
                }
            }
        }
    };
    let encoder = Arc::new(Mutex::new(Some(encoder)));
    let comp = Compositor::new(
        gpu,
        crop,
        (cfg.width, cfg.height),
        cfg.fps,
        camera.is_some(),
    )
    .map_err(|e| EngineError::failed(msg("Could not set up the GPU scaler", &e)))?;

    let video = {
        let (sh, enc) = (shared.clone(), encoder.clone());
        let boxed = SendBox((comp, source));
        std::thread::Builder::new()
            .name("win-video".into())
            .spawn(move || {
                let SendBox((comp, source)) = boxed;
                video_loop(&sh, comp, source, &enc);
            })
            .map_err(|e| EngineError::failed(format!("Could not start the video thread: {e}")))?
    };
    let audio = {
        let (sh, enc, roles) = (shared.clone(), encoder.clone(), roles.clone());
        std::thread::Builder::new()
            .name("win-audio".into())
            .spawn(move || audio_loop(&sh, &rx, &roles, has, &enc))
            .map_err(|e| EngineError::failed(format!("Could not start the audio thread: {e}")))?
    };

    Ok(Started {
        session: Box::new(WinSession {
            shared,
            video: Some(video),
            audio: Some(audio),
            audio_tx: Some(tx),
            sources,
            tone,
            camera,
            encoder,
        }),
        info: SessionInfo {
            width: cfg.width,
            height: cfg.height,
            fps: cfg.fps,
            audio_tracks: roles.len() as u32,
        },
        notes,
    })
}

/// The pacer: one frame per slot from the first captured frame on.
fn video_loop(
    sh: &Shared,
    mut comp: Compositor,
    mut source: VideoSource,
    enc: &Mutex<Option<Encoder>>,
) {
    com_init();
    let fps = sh.fps;
    let (cw, ch) = comp.canvas_size();
    let mut next_slot: i64 = 0;
    let mut last: Option<windows::Win32::Graphics::Direct3D11::ID3D11Texture2D> = None;
    let mut dirty = true;
    let mut overlay_seq = 0u64;
    if matches!(source, VideoSource::CameraOnly) {
        comp.draw_black();
    }
    while !sh.stop.load(Ordering::SeqCst) {
        // The newest picture.
        let now = qpc_secs();
        match &mut source {
            VideoSource::Capture(c) => {
                if let Some(f) = c.0.latest() {
                    comp.draw_capture(&f.texture, f.size);
                    dirty = true;
                    if sh.clock().is_none() {
                        // The timeline starts at the first frame.
                        if let Ok(mut g) = sh.clock.lock() {
                            *g = Some(FrameClock::new(fps, now.min(f.time.max(now - 0.1))));
                        }
                    }
                }
            }
            VideoSource::Pattern | VideoSource::CameraOnly => {
                if sh.clock().is_none()
                    && let Ok(mut g) = sh.clock.lock()
                {
                    *g = Some(FrameClock::new(fps, now));
                }
            }
        }
        if let Ok(mut o) = sh.overlay.try_lock()
            && o.seq != overlay_seq
        {
            overlay_seq = o.seq;
            if let Some((img, w, h, rect)) = o.image.take() {
                comp.set_overlay(&img, w, h, rect);
                dirty = true;
            }
        }
        let Some(clock) = sh.clock() else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        let now_slot = clock.slot_at(qpc_secs());
        if now_slot < next_slot {
            let wait = clock.slot_start(next_slot) - qpc_secs();
            std::thread::sleep(Duration::from_secs_f64(wait.clamp(0.0005, 0.004)));
            continue;
        }
        let due = match sh.pauses.lock() {
            Ok(p) => slots_due(next_slot, now_slot, &p, MAX_CATCH_UP),
            Err(_) => vec![],
        };
        let missed = (now_slot - next_slot + 1) as u64 - due.len() as u64;
        if missed > 0 {
            sh.drops.fetch_add(missed, Ordering::Relaxed);
        }
        next_slot = now_slot + 1;
        if matches!(source, VideoSource::Pattern) {
            comp.draw_pixels(&pattern_frame(now_slot.max(0) as u64, fps, cw, ch), cw, ch);
            dirty = true;
        }
        // Every ring texture may still be queued for the encoder: repeat the last frame
        // rather than overwrite one (counted as a drop).
        let backlog = enc
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(Encoder::video_backlog))
            .unwrap_or(0);
        if dirty && last.is_some() && backlog >= (RING - 1) as u64 {
            sh.drops.fetch_add(1, Ordering::Relaxed);
        } else if dirty || last.is_none() {
            match comp.finish() {
                Ok(t) => last = Some(t),
                Err(e) => {
                    sh.fail(msg("The GPU scaler failed", &e));
                    break;
                }
            }
            dirty = false;
        }
        let Some(tex) = &last else { continue };
        for slot in due {
            let Some(out) = slot.out else { continue };
            if sh.max_slots.is_some_and(|m| out as u64 >= m) {
                continue;
            }
            let r = match enc.lock() {
                Ok(g) => match g.as_ref() {
                    Some(e) => e.write_video(tex, slot_time(out, fps), slot_duration(out, fps)),
                    None => Ok(()),
                },
                Err(_) => Ok(()),
            };
            if let Err(e) = r {
                sh.fail(msg("Writing the video failed", &e));
                return;
            }
            sh.frames.fetch_add(1, Ordering::Relaxed);
            sh.out_slots.fetch_max(out as u64 + 1, Ordering::Relaxed);
        }
    }
}

/// The test pattern's sound as the "system" source, on the QPC clock.
fn tone_loop(sh: &Shared, tx: &Sender<AudioMsg>) {
    let start = qpc_secs();
    let mut sent: u64 = 0;
    while !sh.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(10));
        let due = ((qpc_secs() - start) * RATE) as u64;
        if due > sent {
            let n = (due - sent) as usize;
            let t = start + sent as f64 / RATE;
            if tx
                .send(AudioMsg::Data(
                    Source::System,
                    t,
                    RATE,
                    pattern_tone(sent, n),
                ))
                .is_err()
            {
                return;
            }
            sent = due;
        }
    }
}

/// Faders, mixers and the pause cuts, then the AAC tracks.
fn audio_loop(
    sh: &Shared,
    rx: &Receiver<AudioMsg>,
    roles: &[TrackRole],
    has: [bool; 2],
    enc: &Mutex<Option<Encoder>>,
) {
    com_init();
    let v = sh.volumes.lock().map(|v| *v).unwrap_or([Volume::UNITY; 2]);
    let mut faders = [Fader::new(v[0].gain()), Fader::new(v[1].gain())];
    let mut mixers: Option<Vec<Mixer>> = None;
    let mut clocks = vec![TrackClock::default(); roles.len()];
    let write = |k: usize,
                 t: f64,
                 frames: &[[f32; 2]],
                 clocks: &mut Vec<TrackClock>|
     -> Result<(), String> {
        let pauses = sh.pauses.lock().map(|p| p.clone()).unwrap_or_default();
        let Some(keep) = audio_keep(t, frames.len(), RATE, sh.fps, &pauses) else {
            return Ok(());
        };
        let Some(fix) = clocks[k].place(keep.out_sample, keep.take) else {
            return Ok(());
        };
        let kept = &frames[keep.skip + fix.skip.min(keep.take)..keep.skip + keep.take];
        let mut run = vec![[0.0f32; 2]; fix.pad];
        run.extend_from_slice(kept);
        if run.is_empty() {
            return Ok(());
        }
        let pcm = to_pcm16(&run);
        let time = fix.start * 10_000_000 / 48_000;
        let dur = run.len() as i64 * 10_000_000 / 48_000;
        match enc.lock() {
            Ok(g) => match g.as_ref() {
                Some(e) => e
                    .write_audio(k, &pcm, time, dur)
                    .map_err(|e| msg("Writing audio failed", &e)),
                None => Ok(()),
            },
            Err(_) => Ok(()),
        }
    };
    loop {
        let (msg_in, done) = match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(m) => (Some(m), false),
            Err(RecvTimeoutError::Timeout) => (None, false),
            Err(RecvTimeoutError::Disconnected) => (None, true),
        };
        if let Some(AudioMsg::Data(src, t, rate, mut frames)) = msg_in {
            let i = src.index();
            let vol = sh.volumes.lock().map(|v| v[i]).unwrap_or(Volume::UNITY);
            sh.note_level(i, peak(&frames) * vol.fader_gain());
            faders[i].set(vol.gain());
            faders[i].apply(&mut frames);
            if mixers.is_none()
                && let Some(clock) = sh.clock()
            {
                mixers = Some(
                    roles
                        .iter()
                        .map(|r| {
                            let (a, b) = track_sources(*r);
                            Mixer::new(clock.t0, a && has[0], b && has[1], *r == TrackRole::Mix)
                        })
                        .collect(),
                );
            }
            if let Some(ms) = &mut mixers {
                for m in ms.iter_mut() {
                    if m.is_active(src) {
                        m.push(src, t, rate, frames.clone());
                    }
                }
            }
        }
        if let Some(ms) = &mut mixers {
            for (k, m) in ms.iter_mut().enumerate() {
                while let Some((t, f)) = m.pull(done) {
                    if let Err(e) = write(k, t, &f, &mut clocks) {
                        sh.fail(e);
                        return;
                    }
                }
            }
        }
        if done {
            return;
        }
    }
}

impl Session for WinSession {
    fn has_frames(&self) -> bool {
        self.shared.clock().is_some()
    }

    fn recorded_secs(&self) -> f64 {
        self.shared.out_slots.load(Ordering::Relaxed) as f64 / f64::from(self.shared.fps.max(1))
    }

    fn dropped_frames(&self) -> u64 {
        self.shared.drops.load(Ordering::Relaxed)
    }

    fn take_levels(&self) -> [f32; 2] {
        let take = |a: &AtomicU32| f32::from_bits(a.swap(0, Ordering::Relaxed));
        [take(&self.shared.levels[0]), take(&self.shared.levels[1])]
    }

    fn pause(&self) -> bool {
        let Some(c) = self.shared.clock() else {
            return false;
        };
        self.shared
            .pauses
            .lock()
            .map(|mut p| p.pause(c.slot_at(qpc_secs()) + 1))
            .unwrap_or(false)
    }

    fn resume(&self) -> bool {
        let Some(c) = self.shared.clock() else {
            return false;
        };
        self.shared
            .pauses
            .lock()
            .map(|mut p| p.resume(c.slot_at(qpc_secs()) + 1))
            .unwrap_or(false)
    }

    fn error(&self) -> Option<String> {
        self.shared.error.lock().ok()?.clone()
    }

    fn update(&self, settings: &RecorderSettings) {
        if let Ok(mut v) = self.shared.volumes.lock() {
            *v = volumes(settings);
        }
        if let Ok(mut o) = self.shared.overlay.lock() {
            o.spec = OverlaySpec::from_settings(settings);
        }
    }

    fn finish(mut self: Box<Self>) -> Result<SessionStats, String> {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(v) = self.video.take() {
            let _ = v.join();
        }
        for s in self.sources.drain(..) {
            s.stop();
        }
        if let Some(t) = self.tone.take() {
            let _ = t.join();
        }
        self.camera = None;
        // Closing the channel tells the audio writer to flush and exit.
        self.audio_tx = None;
        if let Some(a) = self.audio.take() {
            let _ = a.join();
        }
        let enc = self
            .encoder
            .lock()
            .map_err(|_| "the encoder lock was poisoned".to_string())?
            .take();
        if let Some(e) = enc {
            e.finish()
                .map_err(|e| msg("Could not finish the file", &e))?;
        }
        if let Some(e) = self.shared.error.lock().ok().and_then(|g| g.clone())
            && self.shared.frames.load(Ordering::Relaxed) == 0
        {
            return Err(e);
        }
        Ok(SessionStats {
            frames: self.shared.frames.load(Ordering::Relaxed),
        })
    }
}

impl Drop for WinSession {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
    }
}
