//! SOURCE OF TRUTH KEYWORDS: plan_recording, Planned, Sources, settings to pipeline, output size, bitrate, gain, overlay rect
//! WHAT:  Turns the user's `RecorderSettings` and the sources that opened into a
//!        `PipelinePlan`, with every encoder candidate to try in order.
//! WHY:   All the decisions (output size, codec, bitrate, overlay rectangle, track layout,
//!        gains and offsets) come from `rec-core` rules and are made here, purely, so they
//!        are unit tested and the GStreamer code only runs what was decided.
//! WHERE: `session::Session::start` (through `engine`).

use rec_core::output::{auto_bitrate, output_size};
use rec_core::overlay::overlay_rect;
use rec_core::settings::{
    CameraResolution, Codec, Container, GAIN_DB_RANGE, RecorderSettings, Scene,
};

use crate::encoders::{EncoderChoice, effective_codec, pick_aac, video_encoders};
use crate::pipeline::{
    AudioInput, AudioRole, CAMERA_TRACK_FPS, CameraPlan, CameraUse, Crop, Muxer, PipelinePlan,
    ScreenPlan, audio_tracks,
};

/// AAC bitrate per track, as the Mac writes.
pub const AAC_BPS: u32 = 192_000;

/// The screen source as opened: element text, its size in pixels, the area crop.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenSource {
    pub src: String,
    pub size: (u32, u32),
    pub crop: Option<Crop>,
}

/// What opened for this recording (None = not wanted or not available).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sources {
    pub screen: Option<ScreenSource>,
    pub camera: Option<String>,
    pub system: Option<String>,
    pub mic: Option<String>,
}

/// A planned recording: the plan for each encoder candidate, best first.
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub plan: PipelinePlan,
    pub encoders: Vec<EncoderChoice>,
    /// The camera track's encoder for each screen encoder (same factory, its own bitrate).
    camera_encoders: Vec<Option<EncoderChoice>>,
    /// User-facing note when the codec setting could not be honoured.
    pub note: Option<String>,
}

impl Planned {
    /// The plan using encoder candidate `i` (0 = best).
    pub fn with_encoder(&self, i: usize) -> Option<PipelinePlan> {
        let e = self.encoders.get(i)?;
        let mut p = self.plan.clone();
        p.video_encoder = (e.element.clone(), e.parser.into());
        p.camera_encoder = self
            .camera_encoders
            .get(i)
            .cloned()
            .flatten()
            .map(|c| (c.element, c.parser.into()));
        if p.camera_encoder.is_none()
            && let Some(c) = &mut p.camera
        {
            c.track = None;
        }
        Some(p)
    }
}

/// The camera's capture size for the setting (the separate camera track's size too).
pub fn camera_size(r: CameraResolution) -> (u32, u32) {
    match r {
        CameraResolution::R720p => (1280, 720),
        CameraResolution::R1080p => (1920, 1080),
    }
}

/// dB to the `volume` element's linear gain: the range's floor is silence, +20 dB is the
/// element's ceiling.
pub fn db_to_linear(db: f32) -> f64 {
    if db <= GAIN_DB_RANGE.0 {
        0.0
    } else {
        10f64.powf(f64::from(db) / 20.0).min(10.0)
    }
}

fn ms_to_ns(ms: f64) -> i64 {
    if ms.is_finite() {
        (ms * 1_000_000.0).round() as i64
    } else {
        0
    }
}

/// Even, at least 2.
fn even_px(v: f64) -> u32 {
    ((v.max(2.0) as u32) / 2) * 2
}

/**
 * SOURCE OF TRUTH KEYWORDS: plan_recording, recording plan, settings to pipeline
 * WHAT:  The plan for `s` with `sources`, or a user-facing reason it cannot record (nothing
 *        to show, no encoder). `available` says which GStreamer factories exist.
 * WHY:   See the module comment.
 * WHERE: `engine` before `session::Session::start`.
 */
pub fn plan_recording(
    s: &RecorderSettings,
    sources: &Sources,
    available: &dyn Fn(&str) -> bool,
) -> Result<Planned, String> {
    let cam_size = camera_size(s.camera_resolution);
    let screen = match s.scene {
        Scene::Cam => None,
        _ => sources.screen.clone(),
    };
    let camera = match s.scene {
        Scene::Screen => None,
        _ => sources.camera.clone(),
    };
    let source_px = match (&screen, &camera) {
        (Some(sc), _) => {
            let c = sc.crop.unwrap_or_default();
            (
                sc.size.0.saturating_sub(c.left + c.right),
                sc.size.1.saturating_sub(c.top + c.bottom),
            )
        }
        (None, Some(_)) => cam_size,
        (None, None) => return Err("Nothing to record: no screen and no camera".into()),
    };
    if source_px.0 < 2 || source_px.1 < 2 {
        return Err("The chosen area is too small to record".into());
    }
    let hevc = crate::encoders::has_hevc(available);
    let size = output_size(s.resolution, source_px, s.codec);
    let (codec, note) = effective_codec(s.codec, size, hevc);
    // A codec change can change the size rule (4K H.264 never upscales).
    let out = if codec == rec_core::output::resolve_codec(s.codec, size) {
        size
    } else {
        output_size(s.resolution, source_px, codec)
    };
    let bitrate = s
        .bitrate_mbps
        .map(|m| (m * 1_000_000.0) as u64)
        .unwrap_or_else(|| auto_bitrate(codec, out, s.fps));
    let encoders = video_encoders(codec, bitrate, s.fps, available);
    if encoders.is_empty() {
        return Err(
            "No H.264 encoder is installed. Install GStreamer's x264 or OpenH264 plugin, or VA-API drivers"
                .into(),
        );
    }
    let aac = pick_aac(AAC_BPS, available);
    let system = sources.system.clone().filter(|_| s.system_audio);
    let mic = sources.mic.clone().filter(|_| s.mic_enabled);
    let (system, mic) = match aac {
        Some(_) => (system, mic),
        None => (None, None),
    };
    let note = match (aac.is_none() && (s.system_audio || s.mic_enabled), note) {
        (true, n) => Some(
            [
                n,
                Some("No AAC encoder is installed; recording without sound".into()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · "),
        ),
        (false, n) => n,
    };
    let tracks = audio_tracks(system.is_some(), mic.is_some(), s.audio_tracks);
    let mut audio = vec![];
    if let Some(src) = system {
        audio.push(AudioInput {
            role: AudioRole::System,
            src,
            volume: db_to_linear(s.system_gain_db),
            mute: s.system_muted,
            offset_ns: ms_to_ns(s.system_offset_ms.unwrap_or(0.0)),
        });
    }
    if let Some(src) = mic {
        audio.push(AudioInput {
            role: AudioRole::Mic,
            src,
            volume: db_to_linear(s.mic_gain_db),
            mute: s.mic_muted,
            offset_ns: ms_to_ns(s.mic_offset_ms),
        });
    }
    let camera_plan = camera.map(|src| {
        let place = if screen.is_some() {
            let r = overlay_rect(
                s.pip_size,
                &s.pip_place,
                s.pip_shape,
                f64::from(out.0),
                f64::from(out.1),
                f64::from(cam_size.0) / f64::from(cam_size.1),
            );
            let (w, h) = (even_px(r[2]), even_px(r[3]));
            CameraUse::Overlay([
                (r[0].max(0.0) as u32).min(out.0.saturating_sub(w)),
                (r[1].max(0.0) as u32).min(out.1.saturating_sub(h)),
                w,
                h,
            ])
        } else {
            CameraUse::Full
        };
        CameraPlan {
            src,
            mirror: s.mirror,
            place,
            // The raw camera next to the composed picture (camera-only already is the camera).
            track: (s.camera_track && screen.is_some()).then_some(cam_size),
        }
    });
    let cam_bitrate = auto_bitrate(Codec::H264, cam_size, CAMERA_TRACK_FPS);
    let camera_encoders = encoders
        .iter()
        .map(|e| {
            video_encoders(e.codec, cam_bitrate, CAMERA_TRACK_FPS, &|f| f == e.factory)
                .into_iter()
                .next()
        })
        .collect();
    let first = &encoders[0];
    let plan = PipelinePlan {
        screen: screen.map(|sc| ScreenPlan {
            src: sc.src,
            crop: sc.crop,
        }),
        camera: camera_plan,
        out,
        fps: s.fps,
        video_encoder: (first.element.clone(), first.parser.into()),
        camera_encoder: None,
        audio,
        tracks,
        aac: aac.unwrap_or_default(),
        muxer: match s.container {
            Container::Mp4 => Muxer::Mp4,
            Container::Mov => Muxer::Mov,
        },
    };
    let planned = Planned {
        plan,
        encoders,
        camera_encoders,
        note,
    };
    // Fill in the camera encoder of the best candidate.
    let plan = planned
        .with_encoder(0)
        .ok_or_else(|| "No video encoder".to_string())?;
    Ok(Planned { plan, ..planned })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::TrackKind;
    use rec_core::settings::{AudioTracks, PipCorner, PipPlace, PipShape, Resolution};

    fn have(f: &str) -> bool {
        ["x264enc", "openh264enc", "avenc_aac", "mp4mux", "qtmux"].contains(&f)
    }

    fn sources() -> Sources {
        Sources {
            screen: Some(ScreenSource {
                src: "videotestsrc is-live=true".into(),
                size: (2560, 1600),
                crop: None,
            }),
            camera: Some("videotestsrc is-live=true pattern=ball".into()),
            system: Some("audiotestsrc is-live=true".into()),
            mic: Some("audiotestsrc is-live=true freq=880".into()),
        }
    }

    #[test]
    fn defaults_record_1080p_h264_with_overlay_camera_track_and_three_tracks() {
        let s = RecorderSettings::default();
        let p = plan_recording(&s, &sources(), &have).unwrap();
        assert_eq!(p.plan.out, (1728, 1080), "16:10 source fitted in 1080p");
        assert_eq!(p.plan.fps, 60);
        assert!(p.plan.video_encoder.0.starts_with("x264enc bitrate="));
        assert_eq!(p.encoders.len(), 2, "x264 then openh264");
        assert_eq!(
            p.plan.tracks,
            [TrackKind::Mix, TrackKind::System, TrackKind::Mic]
        );
        assert_eq!(p.plan.muxer, Muxer::Mov);
        let cam = p.plan.camera.as_ref().unwrap();
        assert!(cam.mirror);
        assert_eq!(cam.track, Some((1920, 1080)));
        // 22% of 1728 = 380 square (squircle), 2% margin = 35 from the bottom-right.
        assert_eq!(cam.place, CameraUse::Overlay([1313, 665, 380, 380]));
        assert!(
            p.plan
                .camera_encoder
                .as_ref()
                .unwrap()
                .0
                .starts_with("x264enc")
        );
        assert!(p.note.is_none());
    }

    #[test]
    fn each_candidate_swaps_both_encoders() {
        let p = plan_recording(&RecorderSettings::default(), &sources(), &have).unwrap();
        let second = p.with_encoder(1).unwrap();
        assert!(second.video_encoder.0.starts_with("openh264enc"));
        assert!(second.camera_encoder.unwrap().0.starts_with("openh264enc"));
        assert!(p.with_encoder(2).is_none());
    }

    #[test]
    fn hevc_without_an_encoder_records_h264_and_says_so() {
        let s = RecorderSettings {
            codec: Codec::Hevc,
            resolution: Resolution::R4k,
            ..RecorderSettings::default()
        };
        let p = plan_recording(&s, &sources(), &have).unwrap();
        // H.264 4K never upscales: fitted inside 3840x2160 from 2560x1600.
        assert_eq!(p.plan.out, (2560, 1600));
        assert!(p.note.unwrap().contains("HEVC"));
    }

    #[test]
    fn screen_only_mix_only_mp4_with_gains() {
        let s = RecorderSettings {
            scene: Scene::Screen,
            container: Container::Mp4,
            audio_tracks: AudioTracks::Mix,
            mic_gain_db: -60.0,
            system_gain_db: 6.0,
            mic_offset_ms: 25.0,
            ..RecorderSettings::default()
        };
        let p = plan_recording(&s, &sources(), &have).unwrap();
        assert!(p.plan.camera.is_none());
        assert_eq!(p.plan.tracks, [TrackKind::Mix]);
        assert_eq!(p.plan.muxer, Muxer::Mp4);
        let mic = p
            .plan
            .audio
            .iter()
            .find(|a| a.role == AudioRole::Mic)
            .unwrap();
        assert_eq!(mic.volume, 0.0);
        assert_eq!(mic.offset_ns, 25_000_000);
        let sys = p
            .plan
            .audio
            .iter()
            .find(|a| a.role == AudioRole::System)
            .unwrap();
        assert!((sys.volume - 1.995).abs() < 0.01);
    }

    #[test]
    fn camera_only_uses_the_camera_size_and_no_track() {
        let s = RecorderSettings {
            scene: Scene::Cam,
            camera_resolution: CameraResolution::R720p,
            resolution: Resolution::Native,
            ..RecorderSettings::default()
        };
        let p = plan_recording(&s, &sources(), &have).unwrap();
        assert!(p.plan.screen.is_none());
        assert_eq!(p.plan.out, (1280, 720));
        let cam = p.plan.camera.unwrap();
        assert_eq!(cam.place, CameraUse::Full);
        assert_eq!(cam.track, None);
    }

    #[test]
    fn crop_sets_the_source_size_and_corner_placement_follows() {
        let mut src = sources();
        if let Some(sc) = &mut src.screen {
            sc.crop = Some(Crop {
                left: 0,
                right: 1280,
                top: 0,
                bottom: 880,
            });
        }
        let s = RecorderSettings {
            pip_place: PipPlace::Corner {
                corner: PipCorner::TopLeft,
            },
            pip_shape: PipShape::Rounded,
            resolution: Resolution::Native,
            ..RecorderSettings::default()
        };
        let p = plan_recording(&s, &src, &have).unwrap();
        assert_eq!(p.plan.out, (1280, 720));
        assert_eq!(
            p.plan.camera.unwrap().place,
            CameraUse::Overlay([26, 26, 282, 158])
        );
    }

    #[test]
    fn missing_pieces_are_explained() {
        let none = Sources::default();
        assert!(plan_recording(&RecorderSettings::default(), &none, &have).is_err());
        let no_enc = |f: &str| f == "avenc_aac";
        let e = plan_recording(&RecorderSettings::default(), &sources(), &no_enc).unwrap_err();
        assert!(e.contains("H.264"));
        let no_aac = |f: &str| f == "x264enc";
        let p = plan_recording(&RecorderSettings::default(), &sources(), &no_aac).unwrap();
        assert!(p.plan.tracks.is_empty());
        assert!(p.note.unwrap().contains("without sound"));
    }

    #[test]
    fn gain_floor_is_silence() {
        assert_eq!(db_to_linear(-60.0), 0.0);
        assert_eq!(db_to_linear(0.0), 1.0);
        assert_eq!(db_to_linear(24.0), 10.0);
    }
}
