//! SOURCE OF TRUTH KEYWORDS: video_encoders, pick_aac, EncoderChoice, vah264enc, nvh264enc, x264enc, openh264enc, avenc_aac, encoder fallback
//! WHAT:  Which GStreamer encoders a recording uses, in order of preference, and the
//!        element text (with bitrate and keyframe interval) for each.
//! WHY:   What is installed differs per PC and distro: VA-API on Intel/AMD, NVENC on NVIDIA,
//!        x264 or OpenH264 in software, AAC from libav, fdk-aac or vo-aac. The engine asks
//!        GStreamer which factories exist and tries these candidates in order (a hardware
//!        element can exist and still fail to open), so the choice is pure and unit tested.
//! WHERE: `session` (pipeline text), `caps` (the `hevc` capability).

use rec_core::settings::Codec;

/// One video encoder candidate: the element with its properties, and the parser after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncoderChoice {
    /// The factory name (`vah264enc`).
    pub factory: &'static str,
    /// Element text for `gst::parse::launch`, properties included.
    pub element: String,
    /// `h264parse` or `h265parse`.
    pub parser: &'static str,
    pub hardware: bool,
    pub codec: Codec,
}

/// H.264 encoders, best first: VA (new `va` plugin), VA-API (old `vaapi`), NVENC, then the
/// software ones.
pub const H264_ENCODERS: [&str; 6] = [
    "vah264enc",
    "vah264lpenc",
    "vaapih264enc",
    "nvh264enc",
    "x264enc",
    "openh264enc",
];
pub const HEVC_ENCODERS: [&str; 6] = [
    "vah265enc",
    "vah265lpenc",
    "vaapih265enc",
    "nvh265enc",
    "x265enc",
    "svthevcenc",
];
/// AAC encoders, best first.
pub const AAC_ENCODERS: [&str; 4] = ["fdkaacenc", "avenc_aac", "voaacenc", "faac"];

/// The element text for `factory` at `bitrate_bps` with a keyframe every `gop` frames.
fn element_for(factory: &str, bitrate_bps: u64, gop: u32) -> Option<String> {
    let kbps = (bitrate_bps / 1000).max(1);
    let e = match factory {
        // `va`: rate-control CBR keeps the file size predictable like the Mac's encoder.
        "vah264enc" | "vah264lpenc" | "vah265enc" | "vah265lpenc" => {
            format!("{factory} bitrate={kbps} rate-control=cbr key-int-max={gop}")
        }
        "vaapih264enc" | "vaapih265enc" => {
            format!("{factory} bitrate={kbps} rate-control=cbr keyframe-period={gop}")
        }
        "nvh264enc" | "nvh265enc" => {
            format!("{factory} bitrate={kbps} rc-mode=cbr gop-size={gop} preset=low-latency-hq")
        }
        "x264enc" => format!(
            "x264enc bitrate={kbps} speed-preset=veryfast tune=zerolatency key-int-max={gop}"
        ),
        "openh264enc" => {
            format!("openh264enc bitrate={bitrate_bps} complexity=low gop-size={gop}")
        }
        "x265enc" => format!(
            "x265enc bitrate={kbps} speed-preset=veryfast tune=zerolatency key-int-max={gop}"
        ),
        "svthevcenc" => format!("svthevcenc target-bitrate={kbps} key-int-max={gop}"),
        _ => return None,
    };
    Some(e)
}

fn is_hardware(factory: &str) -> bool {
    factory.starts_with("va") || factory.starts_with("nv")
}

/**
 * SOURCE OF TRUTH KEYWORDS: video_encoders, encoder candidates, hardware first, software fallback
 * WHAT:  The encoder candidates for `codec` that `available` says exist, best first. HEVC
 *        falls back to H.264 candidates (after the HEVC ones), so a PC without an HEVC
 *        encoder still records. Auto and ProRes must be resolved before (ProRes is Apple-only).
 * WHY:   The engine tries each until a pipeline reaches PLAYING.
 * WHERE: `session::Session::start`.
 */
pub fn video_encoders(
    codec: Codec,
    bitrate_bps: u64,
    fps: u32,
    available: &dyn Fn(&str) -> bool,
) -> Vec<EncoderChoice> {
    // A keyframe every 2 s, like the Mac's writer.
    let gop = fps.max(1) * 2;
    let h264 = H264_ENCODERS.iter().map(|f| (*f, Codec::H264, "h264parse"));
    let hevc = HEVC_ENCODERS.iter().map(|f| (*f, Codec::Hevc, "h265parse"));
    let order: Vec<_> = match codec {
        Codec::Hevc => hevc.chain(h264).collect(),
        _ => h264.collect(),
    };
    order
        .into_iter()
        .filter(|(f, _, _)| available(f))
        .filter_map(|(factory, codec, parser)| {
            Some(EncoderChoice {
                factory,
                element: element_for(factory, bitrate_bps, gop)?,
                parser,
                hardware: is_hardware(factory),
                codec,
            })
        })
        .collect()
}

/// Whether any HEVC encoder exists (the `hevc` capability).
pub fn has_hevc(available: &dyn Fn(&str) -> bool) -> bool {
    HEVC_ENCODERS.iter().any(|f| available(f))
}

/// The AAC encoder element at `bitrate_bps`, or None when none is installed.
pub fn pick_aac(bitrate_bps: u32, available: &dyn Fn(&str) -> bool) -> Option<String> {
    let f = AAC_ENCODERS.iter().find(|f| available(f))?;
    Some(match *f {
        "fdkaacenc" | "avenc_aac" | "voaacenc" => format!("{f} bitrate={bitrate_bps}"),
        // faac takes bits per second too, but wants CBR set explicitly.
        _ => format!("faac bitrate={bitrate_bps} rate-control=2"),
    })
}

/// The recorded codec: `Auto` by Rec's rule (`rec_core::output::resolve_codec`), ProRes
/// replaced by H.264 (no ProRes encoder on Linux), HEVC kept only if an encoder exists.
/// The second value is a user-facing note when the choice was changed.
pub fn effective_codec(
    setting: Codec,
    size: (u32, u32),
    hevc_available: bool,
) -> (Codec, Option<String>) {
    let wanted = rec_core::output::resolve_codec(setting, size);
    match wanted {
        Codec::Prores => (
            Codec::H264,
            Some("ProRes isn't available on Linux; recording H.264".into()),
        ),
        Codec::Hevc if !hevc_available => (
            Codec::H264,
            (setting == Codec::Hevc)
                .then(|| "No HEVC encoder is installed; recording H.264".to_string()),
        ),
        c => (c, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |f| names.contains(&f)
    }

    #[test]
    fn hardware_comes_first_then_software() {
        let have = only(&["x264enc", "vah264enc", "openh264enc"]);
        let c = video_encoders(Codec::H264, 8_000_000, 60, &have);
        let names: Vec<_> = c.iter().map(|e| e.factory).collect();
        assert_eq!(names, ["vah264enc", "x264enc", "openh264enc"]);
        assert!(c[0].hardware && !c[1].hardware);
        assert_eq!(
            c[0].element,
            "vah264enc bitrate=8000 rate-control=cbr key-int-max=120"
        );
        assert_eq!(
            c[2].element,
            "openh264enc bitrate=8000000 complexity=low gop-size=120"
        );
        assert!(c.iter().all(|e| e.parser == "h264parse"));
    }

    #[test]
    fn hevc_falls_back_to_h264() {
        let have = only(&["x264enc", "nvh265enc"]);
        let c = video_encoders(Codec::Hevc, 20_000_000, 30, &have);
        assert_eq!(c[0].factory, "nvh265enc");
        assert_eq!(c[0].parser, "h265parse");
        assert_eq!(c[0].codec, Codec::Hevc);
        assert_eq!(c[1].factory, "x264enc");
        assert_eq!(c[1].codec, Codec::H264);
        assert!(c[1].element.contains("key-int-max=60"));
        assert!(has_hevc(&have));
        assert!(!has_hevc(&only(&["x264enc"])));
    }

    #[test]
    fn nothing_installed_means_no_candidates() {
        assert!(video_encoders(Codec::H264, 1, 60, &only(&[])).is_empty());
        assert_eq!(pick_aac(192_000, &only(&[])), None);
    }

    #[test]
    fn aac_prefers_fdk_then_libav() {
        assert_eq!(
            pick_aac(192_000, &only(&["avenc_aac", "fdkaacenc"])).as_deref(),
            Some("fdkaacenc bitrate=192000")
        );
        assert_eq!(
            pick_aac(128_000, &only(&["voaacenc", "avenc_aac"])).as_deref(),
            Some("avenc_aac bitrate=128000")
        );
    }

    #[test]
    fn codec_resolution_follows_recs_rule_and_what_exists() {
        assert_eq!(
            effective_codec(Codec::Auto, (1920, 1080), true),
            (Codec::H264, None)
        );
        assert_eq!(
            effective_codec(Codec::Auto, (3840, 2160), true),
            (Codec::Hevc, None)
        );
        // Auto silently records H.264 without an HEVC encoder; an explicit choice says so.
        assert_eq!(
            effective_codec(Codec::Auto, (3840, 2160), false),
            (Codec::H264, None)
        );
        let (c, note) = effective_codec(Codec::Hevc, (1280, 720), false);
        assert_eq!(c, Codec::H264);
        assert!(note.unwrap().contains("HEVC"));
        let (c, note) = effective_codec(Codec::Prores, (1280, 720), true);
        assert_eq!(c, Codec::H264);
        assert!(note.unwrap().contains("ProRes"));
    }
}
