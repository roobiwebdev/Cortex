//! SOURCE OF TRUTH KEYWORDS: EncoderConfig, VideoCodec, encoder_config, crop_pixels, CropRect, HEVC fallback, MP4 only, Windows plan
//! WHAT:  Settings to what the Windows encoder writes: codec (Auto → H.264/HEVC, HEVC falls
//!        back to H.264 when no HEVC encoder exists, ProRes refused), frame size, bitrate,
//!        the region's crop in capture pixels, and the file extension.
//! WHY:   Frame size, Auto codec and bitrate come from `rec_core::output` (the rules the UI
//!        mirrors), in the same order as the macOS engine's `args::prepare`: the size is
//!        computed with the codec setting, then Auto is resolved on that size. Pure, so the
//!        whole mapping is unit-tested on any OS.
//! WHERE: `win::session` (when a capture starts and its source size is known).

use rec_core::api::EngineError;
use rec_core::output::{auto_bitrate, output_size, resolve_codec};
use rec_core::settings::{Codec, RecorderSettings};
use rec_core::state::Rect;

/// Media Foundation writes MPEG-4; there is no QuickTime sink.
pub const FILE_EXT: &str = "mp4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    Hevc,
}

/// What the video encoder is set up with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderConfig {
    pub codec: VideoCodec,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: u32,
}

/// The encoder setup for `s` and a source of `source` pixels. `hevc` says whether an HEVC
/// encoder exists on this PC. The second value is a user-facing note when the choice was
/// changed (HEVC asked for, H.264 recorded).
pub fn encoder_config(
    s: &RecorderSettings,
    source: (u32, u32),
    hevc: bool,
) -> Result<(EncoderConfig, Option<String>), EngineError> {
    if s.codec == Codec::Prores {
        return Err(EngineError::Unsupported {
            message: "ProRes is not available on Windows. Choose H.264 or HEVC.".into(),
        });
    }
    let mut size = output_size(s.resolution, source, s.codec);
    let mut codec = resolve_codec(s.codec, size);
    let mut note = None;
    if codec == Codec::Hevc && !hevc {
        if s.codec == Codec::Hevc {
            note = Some("This PC has no HEVC encoder; recording H.264 instead".to_string());
        }
        codec = Codec::H264;
        // H.264 keeps the earlier 4K rule.
        size = output_size(s.resolution, source, Codec::H264);
    }
    let bitrate = s
        .bitrate_mbps
        .map(|m| (m * 1e6) as u64)
        .unwrap_or_else(|| auto_bitrate(codec, size, s.fps))
        .clamp(1_000_000, 400_000_000);
    Ok((
        EncoderConfig {
            codec: if codec == Codec::Hevc {
                VideoCodec::Hevc
            } else {
                VideoCodec::H264
            },
            width: size.0,
            height: size.1,
            fps: s.fps,
            bitrate: u32::try_from(bitrate).unwrap_or(u32::MAX),
        },
        note,
    ))
}

/// A rectangle of whole pixels inside a captured frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl CropRect {
    pub fn full(w: u32, h: u32) -> CropRect {
        CropRect { x: 0, y: 0, w, h }
    }
}

/// A region in points (`CaptureTarget::Region`) as capture pixels on a display of
/// `pixels` size and `scale` pixels per point: clipped to the display, even width and
/// height (the encoder needs them). None when nothing is left.
pub fn crop_pixels(region: Rect, scale: f64, pixels: (u32, u32)) -> Option<CropRect> {
    let r = region
        .scaled(if scale > 0.0 { scale } else { 1.0 })
        .clipped_to(pixels.0 as f64, pixels.1 as f64)?;
    let x = r.x.round().max(0.0) as u32;
    let y = r.y.round().max(0.0) as u32;
    let w = (r.w.round() as u32).min(pixels.0.saturating_sub(x)) & !1;
    let h = (r.h.round() as u32).min(pixels.1.saturating_sub(y)) & !1;
    (w >= 2 && h >= 2).then_some(CropRect { x, y, w, h })
}

/// The part of a `frame`-sized capture that `crop` asks for, clipped to the frame (a
/// window may shrink while it is recorded).
pub fn clip_crop(crop: CropRect, frame: (u32, u32)) -> Option<CropRect> {
    let x1 = (crop.x + crop.w).min(frame.0);
    let y1 = (crop.y + crop.h).min(frame.1);
    (x1 > crop.x && y1 > crop.y).then(|| CropRect {
        x: crop.x,
        y: crop.y,
        w: x1 - crop.x,
        h: y1 - crop.y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rec_core::settings::Resolution;

    fn settings(codec: Codec, resolution: Resolution) -> RecorderSettings {
        RecorderSettings {
            codec,
            resolution,
            fps: 60,
            ..RecorderSettings::default()
        }
    }

    #[test]
    fn auto_follows_rec_core() {
        let (c, note) = encoder_config(
            &settings(Codec::Auto, Resolution::R1080p),
            (2560, 1440),
            true,
        )
        .unwrap();
        assert_eq!((c.codec, c.width, c.height), (VideoCodec::H264, 1920, 1080));
        assert_eq!(
            c.bitrate as u64,
            auto_bitrate(Codec::H264, (1920, 1080), 60)
        );
        assert_eq!(note, None);
        let (c, _) =
            encoder_config(&settings(Codec::Auto, Resolution::R4k), (2560, 1440), true).unwrap();
        assert_eq!((c.codec, c.width, c.height), (VideoCodec::Hevc, 3840, 2160));
    }

    #[test]
    fn without_hevc_4k_records_h264_at_its_size() {
        let (c, note) =
            encoder_config(&settings(Codec::Auto, Resolution::R4k), (3456, 2234), false).unwrap();
        assert_eq!((c.codec, c.width, c.height), (VideoCodec::H264, 3342, 2160));
        assert_eq!(note, None, "Auto changes silently");
        let (c, note) = encoder_config(
            &settings(Codec::Hevc, Resolution::R1080p),
            (1920, 1080),
            false,
        )
        .unwrap();
        assert_eq!(c.codec, VideoCodec::H264);
        assert!(note.unwrap().contains("no HEVC encoder"));
    }

    #[test]
    fn prores_is_refused_and_manual_bitrate_wins() {
        assert!(matches!(
            encoder_config(
                &settings(Codec::Prores, Resolution::R1080p),
                (1920, 1080),
                true
            ),
            Err(EngineError::Unsupported { .. })
        ));
        let s = RecorderSettings {
            bitrate_mbps: Some(12.5),
            ..settings(Codec::H264, Resolution::Native)
        };
        let (c, _) = encoder_config(&s, (1921, 1081), true).unwrap();
        assert_eq!((c.width, c.height, c.bitrate), (1920, 1080, 12_500_000));
    }

    #[test]
    fn regions_become_even_pixel_crops() {
        let r = Rect {
            x: 100.0,
            y: 50.5,
            w: 401.0,
            h: 301.0,
        };
        assert_eq!(
            crop_pixels(r, 1.5, (2880, 1620)),
            Some(CropRect {
                x: 150,
                y: 76,
                w: 602,
                h: 452
            })
        );
        // Clipped to the display.
        let r = Rect {
            x: 1800.0,
            y: 1000.0,
            w: 400.0,
            h: 400.0,
        };
        assert_eq!(
            crop_pixels(r, 1.0, (1920, 1080)),
            Some(CropRect {
                x: 1800,
                y: 1000,
                w: 120,
                h: 80
            })
        );
        let off = Rect {
            x: 3000.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        assert_eq!(crop_pixels(off, 1.0, (1920, 1080)), None);
    }

    #[test]
    fn crops_follow_a_shrinking_window() {
        let c = CropRect::full(1000, 800);
        assert_eq!(
            clip_crop(c, (900, 900)),
            Some(CropRect {
                x: 0,
                y: 0,
                w: 900,
                h: 800
            })
        );
        assert_eq!(
            clip_crop(
                CropRect {
                    x: 10,
                    y: 10,
                    w: 5,
                    h: 5
                },
                (5, 5)
            ),
            None
        );
    }
}
