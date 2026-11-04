//! SOURCE OF TRUTH KEYWORDS: Session, GStreamer pipeline, pause gate, pad probe, level meter, camera mask, EOS finalize, encoder retry
//! WHAT:  One running recording: the parsed GStreamer pipeline from `pipeline::describe`, the
//!        pause gates (pad probes that ask the shared `Timeline`), the camera overlay's
//!        alpha mask, the audio meters, live volume changes, and EOS finalization.
//! WHY:   GStreamer does the capture plumbing and encoding; what Cortex adds is cutting
//!        pauses out of every track the same way, the shaped camera, and trying the next
//!        encoder when a hardware one exists but will not open.
//! WHERE: `engine` (recordings).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use rec_core::overlay::shape_mask;
use rec_core::settings::{PipShape, normalize_hex};

use crate::pipeline::{AudioRole, CameraUse, PipelinePlan, describe, gate_names, names};
use crate::recording::Planned;
use crate::timeline::Timeline;

/// Starts GStreamer once; false when it cannot (missing libraries).
pub fn init() -> bool {
    gst::init().is_ok()
}

/// Whether an element factory is installed.
pub fn has_factory(name: &str) -> bool {
    gst::ElementFactory::find(name).is_some()
}

/// What the bus and the probes report, shared with the engine thread.
#[derive(Default)]
struct Shared {
    /// Peak levels (linear 0..1) of [system, mic] since the last poll.
    levels: Mutex<[f32; 2]>,
    error: Mutex<Option<String>>,
    eos: Mutex<bool>,
    eos_cv: Condvar,
    /// Frames that passed the video gate (the file's frames).
    video_frames: AtomicU64,
}

/// The overlay's shape, applied to every camera frame.
#[derive(Clone, Debug, PartialEq)]
pub struct MaskSpec {
    pub shape: PipShape,
    pub roundness: f64,
    /// Border width in output pixels (0 = none) and its colour.
    pub border: f64,
    pub border_rgb: [u8; 3],
}

impl MaskSpec {
    pub fn from_settings(s: &rec_core::settings::RecorderSettings, out_h: u32) -> MaskSpec {
        let hex = normalize_hex(&s.pip_border_color).unwrap_or_else(|| "ffffff".into());
        let byte =
            |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
        MaskSpec {
            shape: s.pip_shape,
            roundness: s.pip_roundness,
            // Settings give the width at 1080p.
            border: s.pip_border * f64::from(out_h) / 1080.0,
            border_rgb: [byte(0), byte(2), byte(4)],
        }
    }
}

/// The alpha of the shape and of its inside (inside the border), cached per size.
struct MaskCache {
    spec: MaskSpec,
    size: (usize, usize),
    outer: Vec<u8>,
    inner: Vec<u8>,
}

impl MaskCache {
    fn build(spec: &MaskSpec, w: usize, h: usize) -> MaskCache {
        let outer = shape_mask(spec.shape, w, h, spec.roundness, 0.0);
        let inner = if spec.border > 0.0 {
            shape_mask(spec.shape, w, h, spec.roundness, spec.border)
        } else {
            outer.clone()
        };
        MaskCache {
            spec: spec.clone(),
            size: (w, h),
            outer,
            inner,
        }
    }
}

/// Apply the mask to one BGRA frame in place.
fn apply_mask(data: &mut [u8], stride: usize, m: &MaskCache) {
    let (w, h) = m.size;
    let [br, bg, bb] = m.spec.border_rgb;
    for y in 0..h {
        let Some(row) = data.get_mut(y * stride..y * stride + w * 4) else {
            return;
        };
        for (x, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let i = y * w + x;
            let (a, inside) = (m.outer[i], m.inner[i]);
            if inside < a {
                // Border ring: blend the colour over the picture by how far outside it is.
                let k = u16::from(a - inside);
                let mix =
                    |c: u8, b: u8| ((u16::from(c) * (255 - k) + u16::from(b) * k) / 255) as u8;
                px[0] = mix(px[0], bb);
                px[1] = mix(px[1], bg);
                px[2] = mix(px[2], br);
            }
            px[3] = a;
        }
    }
}

/// Totals after a recording.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub video_frames: u64,
    pub recorded_secs: f64,
}

pub struct Session {
    pipeline: gst::Pipeline,
    shared: Arc<Shared>,
    timeline: Arc<Mutex<Timeline>>,
    mask: Arc<Mutex<MaskSpec>>,
    file: PathBuf,
    plan: PipelinePlan,
    started: Instant,
    /// The latest running time each gate has seen, kept or dropped.
    gates_seen: Vec<Arc<AtomicU64>>,
    /// The portal session and its PipeWire fd: closed when the recording ends.
    _keepalive: Option<Box<dyn Send>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn running_time(pipeline: &gst::Pipeline) -> u64 {
    pipeline
        .current_running_time()
        .map(|t| t.nseconds())
        .unwrap_or(0)
}

impl Session {
    /// Start the plan with each encoder candidate in turn until one records. `keepalive`
    /// (the portal stream) lives as long as the session.
    pub fn start(
        planned: &Planned,
        file: &Path,
        mask: MaskSpec,
        keepalive: Option<Box<dyn Send>>,
    ) -> Result<Session, String> {
        let mut last_err = String::from("no encoder");
        let mut keepalive = keepalive;
        for i in 0..planned.encoders.len() {
            let Some(plan) = planned.with_encoder(i) else {
                break;
            };
            match Session::start_plan(&plan, file, mask.clone()) {
                Ok(mut s) => {
                    s._keepalive = keepalive.take();
                    return Ok(s);
                }
                Err(e) => {
                    let factory = planned.encoders[i].factory;
                    eprintln!("cortex: encoder {factory} failed: {e}");
                    let _ = std::fs::remove_file(file);
                    last_err = e;
                }
            }
        }
        Err(last_err)
    }

    /// Parse, wire the probes and play one plan. Fails if the pipeline errors or reaches
    /// no frame within a few seconds of PLAYING.
    pub fn start_plan(plan: &PipelinePlan, file: &Path, mask: MaskSpec) -> Result<Session, String> {
        let text = describe(plan);
        if std::env::var_os("CORTEX_DEBUG_PIPELINE").is_some() {
            eprintln!("cortex: pipeline:\n{text}");
        }
        let pipeline = gst::parse::launch(&text)
            .map_err(|e| format!("could not build the pipeline: {e}"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| "the pipeline text did not make a pipeline".to_string())?;
        let by_name = |n: &str| {
            pipeline
                .by_name(n)
                .ok_or_else(|| format!("pipeline has no {n}"))
        };
        by_name(names::SINK)?.set_property("location", file.to_string_lossy().as_ref());

        let shared = Arc::new(Shared::default());
        let timeline = Arc::new(Mutex::new(Timeline::default()));
        let mut gates_seen = vec![];
        for gate in gate_names(plan) {
            let pad = by_name(&gate)?
                .static_pad("src")
                .ok_or_else(|| format!("{gate} has no src pad"))?;
            let tl = timeline.clone();
            let counter = (gate == names::VIDEO_GATE).then(|| shared.clone());
            let video = gate == names::VIDEO_GATE || gate == names::CAMERA_GATE;
            let qos = video.then(Qos::default);
            let seen = Arc::new(AtomicU64::new(0));
            gates_seen.push(seen.clone());
            pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
                gate_probe(pad, info, &tl, &seen, counter.as_deref(), qos.as_ref())
            });
        }
        let mask = Arc::new(Mutex::new(mask));
        if let Some(cam) = &plan.camera
            && let CameraUse::Overlay([_, _, w, h]) = cam.place
            && plan.screen.is_some()
        {
            let pad = by_name(names::CAMERA_MASK)?
                .static_pad("src")
                .ok_or_else(|| "cammask has no src pad".to_string())?;
            let spec = mask.clone();
            let cache: Mutex<Option<MaskCache>> = Mutex::new(None);
            let (w, h) = (w as usize, h as usize);
            pad.add_probe(gst::PadProbeType::BUFFER, move |_pad, info| {
                if let Some(gst::PadProbeData::Buffer(buffer)) = &mut info.data {
                    let spec = lock(&spec).clone();
                    let mut c = lock(&cache);
                    if c.as_ref()
                        .is_none_or(|m| m.spec != spec || m.size != (w, h))
                    {
                        *c = Some(MaskCache::build(&spec, w, h));
                    }
                    if let Some(m) = c.as_ref()
                        && let Ok(mut map) = buffer.make_mut().map_writable()
                    {
                        let stride = map.size() / h.max(1);
                        apply_mask(map.as_mut_slice(), stride, m);
                    }
                }
                gst::PadProbeReturn::Ok
            });
        }

        let bus = pipeline
            .bus()
            .ok_or_else(|| "pipeline has no bus".to_string())?;
        let sh = shared.clone();
        bus.set_sync_handler(move |_bus, msg| {
            on_message(&sh, msg);
            gst::BusSyncReply::Drop
        });

        let session = Session {
            pipeline,
            shared,
            timeline,
            mask,
            file: file.to_path_buf(),
            plan: plan.clone(),
            started: Instant::now(),
            gates_seen,
            _keepalive: None,
        };
        if session.pipeline.set_state(gst::State::Playing).is_err() {
            let e = session.error().unwrap_or_else(|| "could not start".into());
            session.stop_now();
            return Err(e);
        }
        // Live pipelines go to PLAYING without prerolling; wait for the first frame or an error.
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if let Some(e) = session.error() {
                session.stop_now();
                return Err(e);
            }
            if session.shared.video_frames.load(Ordering::Relaxed) > 0 {
                return Ok(session);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // No frame yet but no error: a still screen may send nothing; keep recording.
        Ok(session)
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn plan(&self) -> &PipelinePlan {
        &self.plan
    }

    /// A source, an encoder or the writer failed.
    pub fn error(&self) -> Option<String> {
        lock(&self.shared.error).clone()
    }

    pub fn has_frames(&self) -> bool {
        lock(&self.timeline).has_started()
    }

    pub fn video_frames(&self) -> u64 {
        self.shared.video_frames.load(Ordering::Relaxed)
    }

    /// Recorded seconds without the pauses.
    pub fn recorded_secs(&self) -> f64 {
        let now = running_time(&self.pipeline);
        lock(&self.timeline).recorded_ns(now) as f64 / 1e9
    }

    pub fn pause(&self) -> bool {
        let now = running_time(&self.pipeline);
        lock(&self.timeline).pause(now)
    }

    pub fn resume(&self) -> bool {
        let now = running_time(&self.pipeline);
        lock(&self.timeline).resume(now)
    }

    pub fn is_paused(&self) -> bool {
        lock(&self.timeline).is_paused()
    }

    /// Peak levels [system, mic] since the last call, then reset.
    pub fn take_levels(&self) -> [f32; 2] {
        std::mem::take(&mut *lock(&self.shared.levels))
    }

    /// Fader and mute changes, live.
    pub fn set_volume(&self, role: AudioRole, volume: f64, mute: bool) {
        if let Some(v) = self.pipeline.by_name(&names::volume(role)) {
            v.set_property("volume", volume);
            v.set_property("mute", mute);
        }
    }

    /// Overlay shape and position changes, live (its size is fixed for the recording).
    pub fn set_overlay(&self, spec: MaskSpec, xy: Option<(u32, u32)>) {
        *lock(&self.mask) = spec;
        let Some((x, y)) = xy else { return };
        if let Some(pad) = self
            .pipeline
            .by_name(names::COMPOSITOR)
            .and_then(|c| c.static_pad("sink_1"))
        {
            pad.set_property("xpos", i32::try_from(x).unwrap_or(0));
            pad.set_property("ypos", i32::try_from(y).unwrap_or(0));
        }
    }

    /// Stop and finalize the file: end the timeline now, let every branch pass that time,
    /// then EOS through every branch so the muxer writes its index, and shut down. A timeout
    /// still leaves a playable fragmented file.
    pub fn finish(self) -> Result<Stats, String> {
        let (end, secs) = {
            let mut tl = lock(&self.timeline);
            let end = tl.end(running_time(&self.pipeline));
            (end, tl.recorded_ns(end) as f64 / 1e9)
        };
        // EOS stops the sources at once. On a slow machine a branch can still be working
        // on frames from before the stop; wait for it so every track ends at the stop.
        let deadline = Instant::now() + CATCH_UP;
        while Instant::now() < deadline
            && self.error().is_none()
            && self
                .gates_seen
                .iter()
                .any(|g| g.load(Ordering::Relaxed) < end)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.pipeline.send_event(gst::event::Eos::new());
        let timeout = Duration::from_secs(20);
        let eos = {
            let g = lock(&self.shared.eos);
            let (g, _) = self
                .shared
                .eos_cv
                .wait_timeout_while(g, timeout, |eos| {
                    !*eos && lock(&self.shared.error).is_none()
                })
                .unwrap_or_else(|p| p.into_inner());
            *g
        };
        let err = self.error();
        let frames = self.video_frames();
        self.stop_now();
        match (eos, err) {
            (true, _) | (false, None) if frames > 0 => Ok(Stats {
                video_frames: frames,
                recorded_secs: secs,
            }),
            (_, Some(e)) => Err(e),
            _ => Err("nothing was recorded".into()),
        }
    }

    /// Stop without finalizing (discard, failed start).
    pub fn stop_now(&self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }

    pub fn elapsed_since_start(&self) -> Duration {
        self.started.elapsed()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// The longest `Session::finish` waits for late branches to reach the stop.
const CATCH_UP: Duration = Duration::from_secs(10);

/// How far behind the clock the composed picture may fall, beyond the compositor's own
/// latency, before the video gate asks it to skip frames.
const VIDEO_LAG_ALLOWED_NS: u64 = 100_000_000;

/// The video gate's lateness report to the compositor: the compositor's latency, asked once.
#[derive(Default)]
struct Qos {
    latency_ns: std::sync::OnceLock<u64>,
}

/**
 * SOURCE OF TRUTH KEYWORDS: frames_late_by, QoS, compositor lag, slow machine, skip frames, backpressure
 * WHAT:  How late (ns) a composed frame stamped `rt` is when it reaches the video gate at
 *        clock running time `now`, past the compositor's `latency` and `VIDEO_LAG_ALLOWED_NS`.
 *        None when it is on time.
 * WHY:   When the CPU can't compose and encode in real time, the live compositor keeps
 *        making every frame and falls further behind the clock. The muxer interleaves by
 *        time, so it then holds the audio back too: the audio queues fill, the tees block,
 *        the sources stall and the live audiomixer, past its deadline, mixes silence and
 *        drops its late input. A sink with `sync=true` would report the lateness as a QoS
 *        event; the file sink can't (it must not sync), so the gate does, and the
 *        compositor skips frames to catch up instead.
 * WHERE: `gate_probe` on the video gate.
 */
fn frames_late_by(rt: u64, now: u64, latency: u64) -> Option<u64> {
    let due = rt
        .saturating_add(latency)
        .saturating_add(VIDEO_LAG_ALLOWED_NS);
    now.checked_sub(due).filter(|late| *late > 0)
}

/// Tell the compositor that the frame stamped `rt` came `late` ns too late, so it drops its
/// frames until it is back on time (`videoaggregator` skips output older than `rt + late`).
fn report_late(pad: &gst::Pad, qos: &Qos, rt: u64) {
    let Some(element) = pad.parent_element() else {
        return;
    };
    let Some(now) = element.current_running_time() else {
        return;
    };
    let latency = *qos.latency_ns.get_or_init(|| {
        let mut q = gst::query::Latency::new();
        element
            .static_pad("sink")
            .filter(|sink| sink.peer_query(&mut q))
            .map(|_| q.result().1.nseconds())
            .unwrap_or(0)
    });
    let Some(late) = frames_late_by(rt, now.nseconds(), latency) else {
        return;
    };
    let diff = i64::try_from(late).unwrap_or(i64::MAX);
    let ev = gst::event::Qos::new(
        gst::QOSType::Underflow,
        1.0,
        diff,
        gst::ClockTime::from_nseconds(rt),
    );
    pad.send_event(ev);
}

/// The pause gate: drop buffers inside a pause, move later ones back by the paused time,
/// and note how far this branch has got (`seen`). On the video gates (`qos`) it also reports frames that arrive late, see `frames_late_by`.
fn gate_probe(
    pad: &gst::Pad,
    info: &mut gst::PadProbeInfo,
    timeline: &Mutex<Timeline>,
    seen: &AtomicU64,
    counter: Option<&Shared>,
    qos: Option<&Qos>,
) -> gst::PadProbeReturn {
    let Some(gst::PadProbeData::Buffer(buffer)) = &mut info.data else {
        return gst::PadProbeReturn::Ok;
    };
    let Some(pts) = buffer.pts() else {
        return gst::PadProbeReturn::Ok;
    };
    // Compare in running time, the clock the pause times are taken from.
    let rt = pad
        .sticky_event::<gst::event::Segment>(0)
        .and_then(|e| e.segment().downcast_ref::<gst::ClockTime>().cloned())
        .and_then(|s| s.to_running_time(pts))
        .unwrap_or(pts)
        .nseconds();
    seen.fetch_max(rt, Ordering::Relaxed);
    if let Some(qos) = qos {
        report_late(pad, qos, rt);
    }
    let mapped = {
        let mut tl = lock(timeline);
        let m = tl.map(rt);
        if m.is_some() {
            tl.mark_origin(rt);
        }
        m
    };
    let Some(mapped) = mapped else {
        return gst::PadProbeReturn::Drop;
    };
    let cut = rt - mapped;
    if cut > 0 {
        let b = buffer.make_mut();
        b.set_pts(pts.checked_sub(gst::ClockTime::from_nseconds(cut)));
        if let Some(dts) = b.dts() {
            b.set_dts(dts.checked_sub(gst::ClockTime::from_nseconds(cut)));
        }
    }
    if let Some(s) = counter {
        s.video_frames.fetch_add(1, Ordering::Relaxed);
    }
    gst::PadProbeReturn::Ok
}

fn on_message(sh: &Shared, msg: &gst::Message) {
    match msg.view() {
        gst::MessageView::Element(e) => {
            let Some(s) = e.structure().filter(|s| s.name() == "level") else {
                return;
            };
            let role = match msg.src().map(|o| o.name()) {
                Some(n) if n == names::level(AudioRole::System) => 0,
                Some(n) if n == names::level(AudioRole::Mic) => 1,
                _ => return,
            };
            let Ok(peaks) = s.get::<glib::ValueArray>("peak") else {
                return;
            };
            let db = peaks
                .iter()
                .filter_map(|v| v.get::<f64>().ok())
                .fold(f64::NEG_INFINITY, f64::max);
            let linear = if db.is_finite() {
                10f64.powf(db / 20.0).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };
            let mut l = lock(&sh.levels);
            l[role] = l[role].max(linear);
        }
        gst::MessageView::Error(err) => {
            let src = msg.src().map(|o| o.name().to_string()).unwrap_or_default();
            let text = format!("{} ({src})", err.error());
            eprintln!(
                "cortex: gstreamer error from {src}: {} {:?}",
                err.error(),
                err.debug()
            );
            lock(&sh.error).get_or_insert(text);
            sh.eos_cv.notify_all();
        }
        gst::MessageView::Eos(_) => {
            *lock(&sh.eos) = true;
            sh.eos_cv.notify_all();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_count_as_late_only_past_latency_and_slack() {
        const MS: u64 = 1_000_000;
        let latency = 40 * MS;
        // On time: within the compositor's latency plus the allowed lag.
        assert_eq!(frames_late_by(1000 * MS, 1000 * MS, latency), None);
        assert_eq!(frames_late_by(1000 * MS, 1140 * MS, latency), None);
        assert_eq!(frames_late_by(1000 * MS, 900 * MS, latency), None, "early");
        // A picture 2 s behind the clock is late by that minus the slack.
        assert_eq!(
            frames_late_by(1000 * MS, 3000 * MS, latency),
            Some(2000 * MS - latency - VIDEO_LAG_ALLOWED_NS)
        );
        assert_eq!(
            frames_late_by(u64::MAX - 1, 5, latency),
            None,
            "no overflow"
        );
    }

    #[test]
    fn mask_clears_corners_and_paints_the_border() {
        let spec = MaskSpec {
            shape: PipShape::Circle,
            roundness: 0.0,
            border: 3.0,
            border_rgb: [255, 0, 0],
        };
        let m = MaskCache::build(&spec, 20, 20);
        let mut px = vec![0u8; 20 * 20 * 4];
        for p in px.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[0, 255, 0, 255]);
        }
        apply_mask(&mut px, 80, &m);
        assert_eq!(px[3], 0, "corner transparent");
        let centre = (10 * 20 + 10) * 4;
        assert_eq!(
            &px[centre..centre + 4],
            &[0, 255, 0, 255],
            "inside untouched"
        );
        let edge = (10 * 20 + 1) * 4;
        assert_eq!(px[edge + 2], 255, "border is red");
        assert_eq!(px[edge + 1], 0);
    }
}
