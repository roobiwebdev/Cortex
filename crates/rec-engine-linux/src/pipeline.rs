//! SOURCE OF TRUTH KEYWORDS: PipelinePlan, describe, gst-launch text, audio_tracks, TrackKind, source_for_id, crop_for, compositor, fragmented mp4
//! WHAT:  A recording as data (`PipelinePlan`: sources, sizes, encoders, tracks) and the
//!        GStreamer pipeline text it becomes, plus the small rules around it: the audio track
//!        layout, device ids to source elements, the region crop.
//! WHY:   The same text runs in the engine (`gst::parse::launch`) and in `gst-launch-1.0`
//!        when debugging on a user's PC, and it is unit tested here without GStreamer.
//!        Shape of every recording:
//!        - video: screen (cropped, scaled to the output size) and the camera overlay go
//!          through one `compositor`, which also turns the portal's damage-driven frames
//!          into a constant frame rate; then the pause gate, the encoder, the muxer;
//!        - the raw camera as a second video track when asked;
//!        - audio: mix, system, mic as separate AAC tracks (the Mac's layout), each source
//!          with its own volume and `level` meter;
//!        - a fragmented MP4/MOV, so a crash leaves a playable file.
//!        Gates sit after the aggregators (compositor, audiomixer): those are live elements
//!        that would drop buffers whose timestamps moved back by a pause.
//!        A slow machine must not cost audio. The muxer interleaves by time and waits for
//!        its slowest track; when the CPU can't compose and encode the picture in real time,
//!        that held back every audio track, stalled the live sources, and the live
//!        audiomixer (past its deadline) mixed silence and dropped its late input. So video
//!        drops frames rather than fall far behind (`FRAME_QUEUE` in front of each video
//!        encoder; past a second of lag the video gates' QoS reports,
//!        `session::frames_late_by`, make the compositor and the camera track's converters
//!        skip frames), and each audio track may run `AUDIO_AHEAD_SECS` ahead of the picture
//!        in the queue before the muxer.
//! WHERE: `session::Session::start`.

use rec_core::settings::AudioTracks;

/// Element names the session looks up after parsing.
pub mod names {
    pub const VIDEO_GATE: &str = "gate_v";
    pub const CAMERA_GATE: &str = "gate_c";
    pub const CAMERA_MASK: &str = "cammask";
    pub const MUX: &str = "mux";
    pub const SINK: &str = "sink";
    pub const COMPOSITOR: &str = "comp";
    /// `gate_a0`, `gate_a1`, ... in track order.
    pub fn audio_gate(track: usize) -> String {
        format!("gate_a{track}")
    }
    pub fn volume(role: super::AudioRole) -> String {
        format!("vol_{}", role.tag())
    }
    pub fn level(role: super::AudioRole) -> String {
        format!("lvl_{}", role.tag())
    }
}

/// The audio sample format every branch is converted to before mixing.
pub const AUDIO_CAPS: &str = "audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved";
/// The separate camera track's frame rate.
pub const CAMERA_TRACK_FPS: u32 = 30;
/// Fragment length of the crash-safe MP4/MOV.
pub const FRAGMENT_MS: u32 = 1000;
/// How far the audio tracks may run ahead of a late picture before the muxer holds them back.
pub const AUDIO_AHEAD_SECS: u64 = 10;
/// The queue in front of each video encoder (and at the head of the camera track): drops
/// the oldest frame only when 10 are waiting, about a third of a second at 30 fps, so an
/// encoder's ups and downs cost nothing and only one that is slower than real time loses
/// frames. Counted in frames, not bytes: the default 10 MB limit held just two 1080p frames.
pub const FRAME_QUEUE: &str =
    "queue max-size-buffers=10 max-size-bytes=0 max-size-time=0 leaky=downstream";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioRole {
    System,
    Mic,
}

impl AudioRole {
    fn tag(self) -> &'static str {
        match self {
            AudioRole::System => "sys",
            AudioRole::Mic => "mic",
        }
    }
}

/// One audio track of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    /// System and mic mixed.
    Mix,
    System,
    Mic,
}

/**
 * SOURCE OF TRUTH KEYWORDS: audio_tracks, track layout, mix, stems, AudioTracks
 * WHAT:  The audio tracks a file gets, in order, for the sources that run.
 * WHY:   Matches the Mac: with both sources, `Both` writes mix + system + mic, `Mix` only the
 *        mix, `Stems` system + mic; one source is one track; none is none.
 * WHERE: `describe`, and the finished recording's `audio_tracks` count.
 */
pub fn audio_tracks(system: bool, mic: bool, layout: AudioTracks) -> Vec<TrackKind> {
    match (system, mic) {
        (true, true) => match layout {
            AudioTracks::Both => vec![TrackKind::Mix, TrackKind::System, TrackKind::Mic],
            AudioTracks::Mix => vec![TrackKind::Mix],
            AudioTracks::Stems => vec![TrackKind::System, TrackKind::Mic],
        },
        (true, false) => vec![TrackKind::System],
        (false, true) => vec![TrackKind::Mic],
        (false, false) => vec![],
    }
}

/// A value for pipeline text: quoted, with `"` and `\` escaped.
pub fn quote(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Device id prefixes. Ids are what `devices()` lists and what settings store.
pub const PULSE_ID: &str = "pulse:";
pub const PIPEWIRE_ID: &str = "pipewire:";
pub const V4L2_ID: &str = "v4l2:";

/**
 * SOURCE OF TRUTH KEYWORDS: source_for_id, device id, pulsesrc, pipewiresrc, v4l2src
 * WHAT:  The source element text for a device id from `devices()`: `pulse:<source name>`,
 *        `pipewire:<node name>` or `v4l2:<device path>`.
 * WHY:   Settings keep the id across launches; the PulseAudio name, the PipeWire node name
 *        and the V4L2 path are the stable handles each API offers.
 * WHERE: `session` (mic and camera sources), `devices` (makes the ids).
 */
pub fn source_for_id(id: &str) -> Option<String> {
    let non_empty = |s: &str| (!s.trim().is_empty()).then(|| quote(s));
    if let Some(name) = id.strip_prefix(PULSE_ID) {
        return Some(format!("pulsesrc device={}", non_empty(name)?));
    }
    if let Some(name) = id.strip_prefix(PIPEWIRE_ID) {
        return Some(format!("pipewiresrc target-object={}", non_empty(name)?));
    }
    if let Some(path) = id.strip_prefix(V4L2_ID) {
        return Some(format!("v4l2src device={}", non_empty(path)?));
    }
    None
}

/// Pixels cut from each edge of the screen stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crop {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

impl Crop {
    pub fn is_empty(&self) -> bool {
        *self == Crop::default()
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: crop_for, region capture, area crop, points to pixels
 * WHAT:  The crop for a `Region` target: `region` (x, y, w, h in the display's points) on a
 *        display `display_pts` points large, applied to a stream `stream_px` pixels large.
 *        Clipped to the stream and kept even (encoders need even sizes). None when nothing
 *        is left.
 * WHY:   The portal gives a whole monitor; the area is cut out of it, scaled by however
 *        many pixels the stream has per point.
 * WHERE: `session` (videocrop).
 */
pub fn crop_for(region: [f64; 4], display_pts: (f64, f64), stream_px: (u32, u32)) -> Option<Crop> {
    let (sw, sh) = (f64::from(stream_px.0), f64::from(stream_px.1));
    if display_pts.0 <= 0.0 || display_pts.1 <= 0.0 || sw <= 0.0 || sh <= 0.0 {
        return None;
    }
    let (kx, ky) = (sw / display_pts.0, sh / display_pts.1);
    let even_down = |v: f64| ((v.max(0.0) as u32) / 2) * 2;
    let x0 = even_down((region[0] * kx).clamp(0.0, sw));
    let y0 = even_down((region[1] * ky).clamp(0.0, sh));
    let x1 = even_down(((region[0] + region[2]) * kx).clamp(0.0, sw));
    let y1 = even_down(((region[1] + region[3]) * ky).clamp(0.0, sh));
    if x1 <= x0 + 1 || y1 <= y0 + 1 {
        return None;
    }
    Some(Crop {
        left: x0,
        right: stream_px.0 - x1,
        top: y0,
        bottom: stream_px.1 - y1,
    })
}

/// The screen (or test pattern) branch.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenPlan {
    /// Source element text (`pipewiresrc fd=… path=…`, `videotestsrc …`).
    pub src: String,
    pub crop: Option<Crop>,
}

/// Where the camera goes in the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraUse {
    /// Over the screen at `[x, y, w, h]` output pixels, masked to the shape.
    Overlay([u32; 4]),
    /// The whole frame (camera-only scene).
    Full,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CameraPlan {
    pub src: String,
    pub mirror: bool,
    pub place: CameraUse,
    /// Also write the unmirrored camera as a second video track at this size.
    pub track: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInput {
    pub role: AudioRole,
    pub src: String,
    /// Linear gain (1.0 = unchanged).
    pub volume: f64,
    pub mute: bool,
    /// Shifts this source's timestamps (sync with the picture); 0 = none.
    pub offset_ns: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Muxer {
    Mp4,
    Mov,
}

/// Everything one recording pipeline needs.
#[derive(Clone, Debug, PartialEq)]
pub struct PipelinePlan {
    pub screen: Option<ScreenPlan>,
    pub camera: Option<CameraPlan>,
    pub out: (u32, u32),
    pub fps: u32,
    /// Encoder element text and its parser (`EncoderChoice::element`, `parser`).
    pub video_encoder: (String, String),
    /// For the camera track; the same encoder family as the screen.
    pub camera_encoder: Option<(String, String)>,
    pub audio: Vec<AudioInput>,
    pub tracks: Vec<TrackKind>,
    /// AAC encoder element text (`encoders::pick_aac`).
    pub aac: String,
    pub muxer: Muxer,
}

impl PipelinePlan {
    fn input(&self, role: AudioRole) -> Option<&AudioInput> {
        self.audio.iter().find(|a| a.role == role)
    }
}

/// The video aggregator: always present, so the frame rate is constant even when the screen
/// only sends damaged frames. It composes in BGRA (the shaped camera needs alpha): left to
/// negotiate, it picked 16-bit A444_16LE, twice the bytes per pixel to blend and convert.
fn compositor(plan: &PipelinePlan) -> String {
    let mut s = format!(
        "compositor name={} background=black start-time-selection=first",
        names::COMPOSITOR
    );
    if let Some(CameraPlan {
        place: CameraUse::Overlay([x, y, _, _]),
        ..
    }) = &plan.camera
        && plan.screen.is_some()
    {
        s.push_str(&format!(
            " sink_1::xpos={x} sink_1::ypos={y} sink_1::zorder=1"
        ));
    }
    let (w, h) = plan.out;
    s.push_str(&format!(
        " ! video/x-raw,format=BGRA,width={w},height={h},framerate={}/1 ! identity name={} silent=true ! videoconvert ! {FRAME_QUEUE} ! {} ! {} ! queue ! {}.video_0",
        plan.fps,
        names::VIDEO_GATE,
        plan.video_encoder.0,
        plan.video_encoder.1,
        names::MUX
    ));
    s
}

fn screen_branch(screen: &ScreenPlan, out: (u32, u32)) -> String {
    let mut s = format!("{} ! queue max-size-buffers=3 leaky=downstream", screen.src);
    if let Some(c) = screen.crop.filter(|c| !c.is_empty()) {
        s.push_str(&format!(
            " ! videocrop left={} right={} top={} bottom={}",
            c.left, c.right, c.top, c.bottom
        ));
    }
    s.push_str(&format!(
        " ! videoconvert ! videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1 ! {}.sink_0",
        out.0,
        out.1,
        names::COMPOSITOR
    ));
    s
}

fn camera_branches(cam: &CameraPlan, plan: &PipelinePlan) -> Vec<String> {
    let mut out = vec![];
    let head = format!(
        "{} ! queue max-size-buffers=3 leaky=downstream ! videoconvert",
        cam.src
    );
    let flip = if cam.mirror {
        " ! videoflip method=horizontal-flip"
    } else {
        ""
    };
    let picture = match (cam.place, plan.screen.is_some()) {
        (CameraUse::Overlay([_, _, w, h]), true) => format!(
            " ! aspectratiocrop aspect-ratio={w}/{h} ! videoscale ! video/x-raw,width={w},height={h},pixel-aspect-ratio=1/1{flip} ! videoconvert ! video/x-raw,format=BGRA ! identity name={} ! {}.sink_1",
            names::CAMERA_MASK,
            names::COMPOSITOR
        ),
        // Camera only: it fills the frame (letterboxed if its shape differs).
        _ => format!(
            " ! videoscale ! video/x-raw,width={},height={},pixel-aspect-ratio=1/1{flip} ! {}.sink_0",
            plan.out.0,
            plan.out.1,
            names::COMPOSITOR
        ),
    };
    match (&cam.track, &plan.camera_encoder) {
        (Some((w, h)), Some((enc, parse))) => {
            out.push(format!("{head} ! tee name=camtee"));
            out.push(format!(
                "camtee. ! queue max-size-buffers=3 leaky=downstream{picture}"
            ));
            out.push(format!(
                "camtee. ! {FRAME_QUEUE} ! videoconvert ! videoscale ! video/x-raw,width={w},height={h},pixel-aspect-ratio=1/1 ! videorate ! video/x-raw,framerate={}/1 ! identity name={} silent=true ! {FRAME_QUEUE} ! {enc} ! {parse} ! queue ! {}.video_1",
                CAMERA_TRACK_FPS,
                names::CAMERA_GATE,
                names::MUX
            ));
        }
        _ => out.push(format!("{head}{picture}")),
    }
    out
}

fn audio_input(a: &AudioInput) -> String {
    let mut s = format!(
        "{} ! queue ! audioconvert ! audioresample ! {AUDIO_CAPS}",
        a.src
    );
    if a.offset_ns != 0 {
        s.push_str(&format!(" ! identity ts-offset={}", a.offset_ns));
    }
    s.push_str(&format!(
        " ! volume name={} volume={} mute={} ! level name={} interval=33333333 post-messages=true ! tee name=t_{}",
        names::volume(a.role),
        a.volume,
        a.mute,
        names::level(a.role),
        a.role.tag()
    ));
    s
}

/// The queue between each AAC encoder and the muxer: up to `AUDIO_AHEAD_SECS` of encoded
/// audio (a few hundred KB), however many buffers or bytes that is.
fn audio_mux_queue() -> String {
    format!(
        "queue max-size-time={} max-size-bytes=0 max-size-buffers=0",
        AUDIO_AHEAD_SECS * 1_000_000_000
    )
}

fn audio_track(n: usize, kind: TrackKind, plan: &PipelinePlan) -> Vec<String> {
    let tail = format!(
        "identity name={} silent=true ! audioconvert ! {} ! {} ! {}.audio_{n}",
        names::audio_gate(n),
        plan.aac,
        audio_mux_queue(),
        names::MUX
    );
    match kind {
        TrackKind::Mix => vec![
            format!("audiomixer name=amix start-time-selection=first ! {AUDIO_CAPS} ! {tail}"),
            "t_sys. ! queue ! amix.".into(),
            "t_mic. ! queue ! amix.".into(),
        ],
        TrackKind::System => vec![format!("t_sys. ! queue ! {tail}")],
        TrackKind::Mic => vec![format!("t_mic. ! queue ! {tail}")],
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: describe, pipeline text, gst::parse::launch, recording pipeline
 * WHAT:  The pipeline text for `plan`, one branch per line. The file location is set on the
 *        `sink` element after parsing (paths need no quoting that way).
 * WHY:   See the module comment.
 * WHERE: `session::Session::start`; printed with `CORTEX_DEBUG_PIPELINE=1`.
 */
pub fn describe(plan: &PipelinePlan) -> String {
    let mux = match plan.muxer {
        Muxer::Mp4 => "mp4mux",
        Muxer::Mov => "qtmux",
    };
    let mut lines = vec![format!(
        "{mux} name={} fragment-duration={FRAGMENT_MS} ! filesink name={} sync=false",
        names::MUX,
        names::SINK
    )];
    lines.push(compositor(plan));
    if let Some(screen) = &plan.screen {
        lines.push(screen_branch(screen, plan.out));
    }
    if let Some(cam) = &plan.camera {
        lines.extend(camera_branches(cam, plan));
    }
    // A source only runs when a track uses it.
    let used = |role: AudioRole| {
        plan.tracks.iter().any(|t| {
            matches!(
                (t, role),
                (TrackKind::Mix, _)
                    | (TrackKind::System, AudioRole::System)
                    | (TrackKind::Mic, AudioRole::Mic)
            )
        })
    };
    for role in [AudioRole::System, AudioRole::Mic] {
        if let Some(a) = plan.input(role).filter(|_| used(role)) {
            lines.push(audio_input(a));
        }
    }
    for (n, kind) in plan.tracks.iter().enumerate() {
        lines.extend(audio_track(n, *kind, plan));
    }
    lines.join("\n")
}

/// Every gate's element name in `plan` (the session installs the pause probe on each).
pub fn gate_names(plan: &PipelinePlan) -> Vec<String> {
    let mut g = vec![names::VIDEO_GATE.to_string()];
    if plan.camera.as_ref().is_some_and(|c| c.track.is_some()) && plan.camera_encoder.is_some() {
        g.push(names::CAMERA_GATE.into());
    }
    g.extend((0..plan.tracks.len()).map(names::audio_gate));
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> PipelinePlan {
        PipelinePlan {
            screen: Some(ScreenPlan {
                src: "videotestsrc is-live=true".into(),
                crop: None,
            }),
            camera: None,
            out: (1920, 1080),
            fps: 60,
            video_encoder: ("x264enc".into(), "h264parse".into()),
            camera_encoder: None,
            audio: vec![
                AudioInput {
                    role: AudioRole::System,
                    src: "audiotestsrc is-live=true".into(),
                    volume: 1.0,
                    mute: false,
                    offset_ns: 0,
                },
                AudioInput {
                    role: AudioRole::Mic,
                    src: "audiotestsrc is-live=true freq=880".into(),
                    volume: 0.5,
                    mute: true,
                    offset_ns: -20_000_000,
                },
            ],
            tracks: audio_tracks(true, true, AudioTracks::Both),
            aac: "avenc_aac bitrate=192000".into(),
            muxer: Muxer::Mp4,
        }
    }

    #[test]
    fn track_layout_matches_the_mac() {
        use TrackKind::*;
        assert_eq!(
            audio_tracks(true, true, AudioTracks::Both),
            [Mix, System, Mic]
        );
        assert_eq!(audio_tracks(true, true, AudioTracks::Mix), [Mix]);
        assert_eq!(audio_tracks(true, true, AudioTracks::Stems), [System, Mic]);
        assert_eq!(audio_tracks(false, true, AudioTracks::Both), [Mic]);
        assert_eq!(audio_tracks(true, false, AudioTracks::Mix), [System]);
        assert!(audio_tracks(false, false, AudioTracks::Both).is_empty());
    }

    #[test]
    fn three_audio_tracks_in_order() {
        let d = describe(&plan());
        assert!(d.starts_with("mp4mux name=mux fragment-duration=1000 ! filesink name=sink"));
        assert!(d.contains("audiomixer name=amix start-time-selection=first ! audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved ! identity name=gate_a0 silent=true ! audioconvert ! avenc_aac bitrate=192000 ! queue max-size-time=10000000000 max-size-bytes=0 max-size-buffers=0 ! mux.audio_0"));
        assert!(d.contains("t_sys. ! queue ! identity name=gate_a1"));
        assert!(d.contains("t_mic. ! queue ! identity name=gate_a2"));
        assert!(d.contains("volume name=vol_mic volume=0.5 mute=true ! level name=lvl_mic"));
        assert!(d.contains("identity ts-offset=-20000000"));
        assert_eq!(
            gate_names(&plan()),
            ["gate_v", "gate_a0", "gate_a1", "gate_a2"]
        );
    }

    #[test]
    fn unused_sources_are_left_out() {
        let mut p = plan();
        p.tracks = audio_tracks(true, false, AudioTracks::Both);
        let d = describe(&p);
        assert!(d.contains("tee name=t_sys"));
        assert!(!d.contains("t_mic"));
        assert!(!d.contains("audiomixer"));
    }

    #[test]
    fn video_goes_through_the_compositor_and_gate() {
        let mut p = plan();
        p.screen = Some(ScreenPlan {
            src: "pipewiresrc fd=7 path=42".into(),
            crop: Some(Crop {
                left: 100,
                right: 20,
                top: 0,
                bottom: 40,
            }),
        });
        p.muxer = Muxer::Mov;
        let d = describe(&p);
        assert!(d.starts_with("qtmux name=mux"));
        assert!(d.contains("pipewiresrc fd=7 path=42 ! queue max-size-buffers=3 leaky=downstream ! videocrop left=100 right=20 top=0 bottom=40 ! videoconvert ! videoscale ! video/x-raw,width=1920,height=1080,pixel-aspect-ratio=1/1 ! comp.sink_0"));
        assert!(d.contains("compositor name=comp background=black start-time-selection=first ! video/x-raw,format=BGRA,width=1920,height=1080,framerate=60/1 ! identity name=gate_v silent=true ! videoconvert ! queue max-size-buffers=10 max-size-bytes=0 max-size-time=0 leaky=downstream ! x264enc ! h264parse ! queue ! mux.video_0"));
    }

    #[test]
    fn camera_overlay_and_track() {
        let mut p = plan();
        p.camera = Some(CameraPlan {
            src: "v4l2src device=\"/dev/video0\"".into(),
            mirror: true,
            place: CameraUse::Overlay([1460, 806, 422, 238]),
            track: Some((1280, 720)),
        });
        p.camera_encoder = Some(("x264enc".into(), "h264parse".into()));
        let d = describe(&p);
        assert!(d.contains("compositor name=comp background=black start-time-selection=first sink_1::xpos=1460 sink_1::ypos=806 sink_1::zorder=1"));
        assert!(d.contains("tee name=camtee"));
        assert!(d.contains("aspectratiocrop aspect-ratio=422/238 ! videoscale ! video/x-raw,width=422,height=238,pixel-aspect-ratio=1/1 ! videoflip method=horizontal-flip ! videoconvert ! video/x-raw,format=BGRA ! identity name=cammask ! comp.sink_1"));
        assert!(d.contains("framerate=30/1 ! identity name=gate_c silent=true ! queue max-size-buffers=10 max-size-bytes=0 max-size-time=0 leaky=downstream ! x264enc ! h264parse ! queue ! mux.video_1"));
        assert_eq!(gate_names(&p)[1], "gate_c");
    }

    #[test]
    fn camera_only_fills_the_frame() {
        let mut p = plan();
        p.screen = None;
        p.camera = Some(CameraPlan {
            src: "videotestsrc is-live=true pattern=ball".into(),
            mirror: false,
            place: CameraUse::Full,
            track: None,
        });
        let d = describe(&p);
        assert!(d.contains("pattern=ball ! queue max-size-buffers=3 leaky=downstream ! videoconvert ! videoscale ! video/x-raw,width=1920,height=1080,pixel-aspect-ratio=1/1 ! comp.sink_0"));
        assert!(!d.contains("sink_1"));
        assert!(!d.contains("cammask"));
    }

    #[test]
    fn device_ids_become_sources() {
        assert_eq!(
            source_for_id("pulse:alsa_input.usb-mic").as_deref(),
            Some("pulsesrc device=\"alsa_input.usb-mic\"")
        );
        assert_eq!(
            source_for_id("pipewire:my node").as_deref(),
            Some("pipewiresrc target-object=\"my node\"")
        );
        assert_eq!(
            source_for_id("v4l2:/dev/video2").as_deref(),
            Some("v4l2src device=\"/dev/video2\"")
        );
        assert_eq!(source_for_id("v4l2:"), None);
        assert_eq!(source_for_id("BuiltInMic"), None);
        assert_eq!(quote(r#"a"b\c"#), r#""a\"b\\c""#);
    }

    #[test]
    fn region_crop_scales_points_to_stream_pixels() {
        // A 1440x900-point display streamed at 2880x1800: 2 px per point.
        let c = crop_for([100.0, 50.0, 640.0, 360.0], (1440.0, 900.0), (2880, 1800)).unwrap();
        assert_eq!(
            c,
            Crop {
                left: 200,
                right: 2880 - 1480,
                top: 100,
                bottom: 1800 - 820
            }
        );
        // Clipped to the stream; odd edges rounded to even.
        let c = crop_for([-10.0, 0.0, 101.0, 2000.0], (1000.0, 1000.0), (1000, 1000)).unwrap();
        assert_eq!(c.left, 0);
        assert_eq!(c.right, 1000 - 90);
        assert_eq!(c.bottom, 0);
        assert_eq!(
            crop_for([5000.0, 0.0, 10.0, 10.0], (1000.0, 1000.0), (1000, 1000)),
            None
        );
        assert_eq!(
            crop_for([0.0, 0.0, 10.0, 10.0], (0.0, 1000.0), (1000, 1000)),
            None
        );
    }
}
