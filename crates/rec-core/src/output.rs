//! SOURCE OF TRUTH KEYWORDS: output_size, auto_bitrate, resolve_codec, bits_per_pixel, Resolution, 4K, upscale, UHD_WIDTH
//! WHAT:  The recorded frame's size for a Resolution setting and a source, the codec that
//!        "Auto" becomes, and the automatic bitrate for a size, frame rate and codec.
//! WHY:   One rule for the engine (what gets written) and the UI (what the pickers say).
//!        "4K" means a 3840-pixel frame at the source's shape: on a 16:10 Retina display
//!        that is 3840×2482, not the 3342×2160 a fit inside 3840×2160 gives. It may
//!        upscale, through the engine's Metal Lanczos scaler; the other presets only
//!        shrink. 4K-class frames get more bits per pixel so screen text stays crisp.
//!        `apps/desktop/src/lib/output-size.ts` mirrors these functions; both are
//!        tested against `tests/fixtures/output-sizes.json`.
//! WHERE: `rec-engine-macos::args` (record arguments to session options), the desktop
//!        Resolution pickers (through the TypeScript mirror).

use crate::settings::{Codec, Resolution};

/// The long side of a "4K" frame.
pub const UHD_WIDTH: u32 = 3840;

/// Frames larger than this many pixels (1440p) count as 4K-class for the bitrate.
pub const UHD_CLASS_MIN_PIXELS: u64 = 2560 * 1440;

/// Bits per pixel per frame, tuned for screen content (sharp text).
/// ProRes is fixed quality: no bitrate.
pub fn bits_per_pixel(codec: Codec, size: (u32, u32)) -> f64 {
    let uhd = size.0 as u64 * size.1 as u64 > UHD_CLASS_MIN_PIXELS;
    match (codec, uhd) {
        (Codec::H264, false) => 0.16,
        (Codec::H264, true) => 0.20,
        // Auto is resolved before a bitrate is chosen; treat it as HEVC.
        (Codec::Hevc | Codec::Auto, false) => 0.10,
        (Codec::Hevc | Codec::Auto, true) => 0.14,
        (Codec::Prores, _) => 0.0,
    }
}

/// The automatic video bitrate in bits per second, clamped to the settings' range
/// (1 to 400 Mb/s). Pixels × frame rate × `bits_per_pixel`.
pub fn auto_bitrate(codec: Codec, size: (u32, u32), fps: u32) -> u64 {
    let px = size.0 as u64 * size.1 as u64;
    ((px as f64 * fps as f64 * bits_per_pixel(codec, size)) as u64).clamp(1_000_000, 400_000_000)
}

/// What `Codec::Auto` records: H.264 up to 1080p, HEVC above (Rec's rule).
pub fn resolve_codec(codec: Codec, size: (u32, u32)) -> Codec {
    match codec {
        Codec::Auto if size.0 as u64 * size.1 as u64 > 1920 * 1080 => Codec::Hevc,
        Codec::Auto => Codec::H264,
        c => c,
    }
}

/// Round to the nearest even number, at least 2 (encoders need even sizes).
fn even(x: f64) -> u32 {
    (((x / 2.0).round() as u32) * 2).max(2)
}

/// The recorded frame size for `res`, a source of `source` pixels and the codec
/// setting. Always even and always the source's shape (no bars, nothing cropped).
///
/// - `Native`: the source's pixels, rounded down to even.
/// - `R720p`, `R1080p`, `R1440p`: fitted inside 1280×720, 1920×1080 or 2560×1440
///   (swapped for a portrait source, so 1080p portrait is 1080 wide), never upscaled.
/// - `R4k`: the long side is 3840, up or down. A landscape source is 3840 wide
///   (3456×2234 → 3840×2482, 5120×2880 → 3840×2160, 3840×2160 unchanged); a portrait
///   one is 3840 tall. The same as fitting inside 3840×3840 with upscaling allowed.
///   With the H.264 codec, 4K keeps the earlier rule: fitted inside 3840×2160, never
///   upscaled (3456×2234 → 3342×2160). Apple's hardware H.264 encoder dropped 5 to 9%
///   of frames at 60 fps with 3582×2314 and 3840×2482 on an M-series MacBook Pro, and
///   none at 3456×2234 or below; HEVC kept every frame at 3840×2482. Auto picks HEVC.
pub fn output_size(res: Resolution, source: (u32, u32), codec: Codec) -> (u32, u32) {
    // The capture's own size is the source rounded down to even.
    let (nw, nh) = ((source.0 & !1).max(2), (source.1 & !1).max(2));
    let (fw, fh) = (nw as f64, nh as f64);
    let k = match res {
        Resolution::Native => return (nw, nh),
        Resolution::R4k if codec != Codec::H264 => UHD_WIDTH as f64 / fw.max(fh),
        _ => {
            let (bw, bh) = match res {
                Resolution::R720p => (1280.0, 720.0),
                Resolution::R1080p => (1920.0, 1080.0),
                Resolution::R1440p => (2560.0, 1440.0),
                _ => (3840.0, 2160.0),
            };
            let (bw, bh) = if nh > nw { (bh, bw) } else { (bw, bh) };
            (bw / fw).min(bh / fh).min(1.0)
        }
    };
    (even(fw * k), even(fh * k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        sizes: Vec<SizeCase>,
        bitrates: Vec<BitrateCase>,
        codecs: Vec<CodecCase>,
    }
    #[derive(Deserialize)]
    struct SizeCase {
        resolution: Resolution,
        #[serde(default)]
        codec: Codec,
        source: (u32, u32),
        output: (u32, u32),
    }
    #[derive(Deserialize)]
    struct BitrateCase {
        codec: Codec,
        size: (u32, u32),
        fps: u32,
        bps: u64,
    }
    #[derive(Deserialize)]
    struct CodecCase {
        codec: Codec,
        size: (u32, u32),
        resolved: Codec,
    }

    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("../tests/fixtures/output-sizes.json")).unwrap()
    }

    /// The cases the TypeScript mirror is checked against, so both agree.
    #[test]
    fn shared_fixture_cases() {
        let f = fixture();
        for c in &f.sizes {
            assert_eq!(
                output_size(c.resolution, c.source, c.codec),
                c.output,
                "{:?} {:?} {:?}",
                c.resolution,
                c.source,
                c.codec
            );
        }
        for c in &f.bitrates {
            assert_eq!(
                auto_bitrate(c.codec, c.size, c.fps),
                c.bps,
                "{:?} {:?} {}",
                c.codec,
                c.size,
                c.fps
            );
        }
        for c in &f.codecs {
            assert_eq!(
                resolve_codec(c.codec, c.size),
                c.resolved,
                "{:?} {:?}",
                c.codec,
                c.size
            );
        }
    }

    #[test]
    fn four_k_is_3840_on_the_long_side() {
        use Resolution::R4k;
        // The 16:10 MacBook Pro display: upscaled to 3840 wide, height follows.
        assert_eq!(output_size(R4k, (3456, 2234), Codec::Hevc), (3840, 2482));
        // A 4K monitor is left alone; a 5K one is fitted to 3840 wide.
        assert_eq!(output_size(R4k, (3840, 2160), Codec::Hevc), (3840, 2160));
        assert_eq!(output_size(R4k, (5120, 2880), Codec::Hevc), (3840, 2160));
        // 1080p doubles.
        assert_eq!(output_size(R4k, (1920, 1080), Codec::Hevc), (3840, 2160));
        // Portrait: 3840 tall.
        assert_eq!(output_size(R4k, (1080, 1920), Codec::Hevc), (2160, 3840));
        assert_eq!(output_size(R4k, (1200, 1601), Codec::Hevc), (2880, 3840));
        // Odd sources: even output (1001×601 → 1000×600 → ×3.84).
        assert_eq!(output_size(R4k, (1001, 601), Codec::Hevc), (3840, 2304));
        assert_eq!(output_size(R4k, (1235, 777), Codec::Hevc), (3840, 2414));
        // H.264: fitted inside 3840×2160, never upscaled (the encoder's real-time limit).
        assert_eq!(output_size(R4k, (3456, 2234), Codec::H264), (3342, 2160));
        assert_eq!(output_size(R4k, (5120, 2880), Codec::H264), (3840, 2160));
        assert_eq!(output_size(R4k, (1920, 1080), Codec::H264), (1920, 1080));
        // Auto and ProRes get the full frame.
        assert_eq!(output_size(R4k, (3456, 2234), Codec::Auto), (3840, 2482));
        assert_eq!(output_size(R4k, (3456, 2234), Codec::Prores), (3840, 2482));
    }

    #[test]
    fn other_presets_only_shrink() {
        use Resolution::*;
        assert_eq!(output_size(R1080p, (3456, 2234), Codec::Hevc), (1670, 1080));
        assert_eq!(output_size(R1440p, (3456, 2234), Codec::Hevc), (2228, 1440));
        assert_eq!(output_size(R720p, (3456, 2234), Codec::Hevc), (1114, 720));
        // Smaller than the box: kept (rounded down to even).
        assert_eq!(output_size(R1080p, (1281, 721), Codec::Hevc), (1280, 720));
        assert_eq!(output_size(R1080p, (1080, 1920), Codec::Hevc), (1080, 1920));
        assert_eq!(output_size(Native, (3457, 2235), Codec::Hevc), (3456, 2234));
    }

    #[test]
    fn four_k_class_gets_more_bits() {
        // About 70 Mb/s HEVC and 100 Mb/s H.264 at 3840×2160 60 fps.
        assert_eq!(auto_bitrate(Codec::Hevc, (3840, 2160), 60) / 1_000_000, 69);
        assert_eq!(auto_bitrate(Codec::H264, (3840, 2160), 60) / 1_000_000, 99);
        // 1440p and below keep Rec's rates.
        assert_eq!(
            auto_bitrate(Codec::Hevc, (2560, 1440), 60),
            (2560.0 * 1440.0 * 60.0 * 0.10) as u64
        );
        assert_eq!(auto_bitrate(Codec::Prores, (3840, 2160), 60), 1_000_000);
    }
}
