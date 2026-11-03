//! SOURCE OF TRUTH KEYWORDS: RecorderSettings, CaptureTarget, PipPlace, StreamDest, SETTINGS_VERSION, migrate, validate
//! WHAT:  The recorder's persisted settings, field for field what the desktop UI edits
//!        (`apps/desktop/src/lib/types.ts` today, generated bindings later).
//! WHY:   One schema shared by the engine, the settings store, the Tauri commands and
//!        the generated TypeScript types. Values are clamped here (`validate`), so every
//!        consumer sees the same legal ranges.
//! WHERE: `store` (load/save/migrate), the macOS engine (`rec-engine-macos::engine`),
//!        the Tauri shell.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bump when the stored shape changes, and add a step to `migrate`.
/// 2: `onboardingCompleted`, `seenHints` and `sounds` (files from version 1 count as
/// onboarded, since their owner already used the app).
pub const SETTINGS_VERSION: u32 = 2;

/// The frame rates Rec's pacer supports (they divide 60000).
pub const FPS_CHOICES: [u32; 5] = [24, 25, 30, 50, 60];
pub const COUNTDOWN_CHOICES: [u32; 3] = [0, 3, 5];
pub const PIP_SIZE_RANGE: (f64, f64) = (0.1, 0.5);
/// dB; `GAIN_DB_RANGE.0` is silence.
pub const GAIN_DB_RANGE: (f32, f32) = (-60.0, 24.0);
pub const REPLAY_SECS_RANGE: (u32, u32) = (15, 300);
pub const OFFSET_MS_RANGE: (f64, f64) = (-1000.0, 1000.0);
pub const BITRATE_MBPS_RANGE: (f64, f64) = (1.0, 400.0);
pub const STREAM_KBPS_RANGE: (u32, u32) = (500, 80_000);
pub const STREAM_AUDIO_KBPS_RANGE: (u32, u32) = (64, 320);
pub const KEYFRAME_SECS_RANGE: (f64, f64) = (0.5, 4.0);
pub const PIP_BORDER_MAX: f64 = 64.0;

macro_rules! string_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "specta", derive(specta::Type))]
        pub enum $name {
            $($(#[$vm])* #[serde(rename = $s)] $v,)+
        }
        impl $name {
            pub const ALL: &[$name] = &[$($name::$v),+];
            /// The serialized name (also Rec's CLI value where one exists).
            pub fn as_str(self) -> &'static str {
                match self { $($name::$v => $s),+ }
            }
            pub fn parse(s: &str) -> Option<Self> {
                match s { $($s => Some($name::$v),)+ _ => None }
            }
        }
    };
}

string_enum!(Container { #[default] Mov = "mov", Mp4 = "mp4" });
string_enum!(
    /// `Auto`: H.264 up to 1080p, HEVC above (Rec's rule).
    Codec { #[default] Auto = "auto", H264 = "h264", Hevc = "hevc", Prores = "prores" }
);
string_enum!(Resolution { R720p = "720p", #[default] R1080p = "1080p", R1440p = "1440p", R4k = "4k", Native = "native" });
string_enum!(
    /// What the frame shows: the screen, the screen with the camera overlay, or the camera.
    Scene { Screen = "screen", #[default] ScreenCam = "screen-cam", Cam = "cam" }
);
string_enum!(PipShape { Rounded = "rounded", Circle = "circle", Square = "square", Pill = "pill", #[default] Squircle = "squircle", Rect = "rect" });
string_enum!(PipCorner { TopLeft = "top-left", TopRight = "top-right", BottomLeft = "bottom-left", #[default] BottomRight = "bottom-right" });
string_enum!(
    /// Tracks when both system audio and mic are recorded: mix + stems, mix only, stems only.
    AudioTracks { #[default] Both = "both", Mix = "mix", Stems = "stems" }
);
string_enum!(StreamService { #[default] Youtube = "youtube", Twitch = "twitch", Kick = "kick", Custom = "custom" });
string_enum!(ThemePref { #[default] System = "system", Light = "light", Dark = "dark" });
string_enum!(CameraResolution { R720p = "720p", #[default] R1080p = "1080p" });
string_enum!(MicBackend { #[default] Sck = "sck", Avcapture = "avcapture" });
string_enum!(StreamCodec { #[default] Auto = "auto", H264 = "h264", Hevc = "hevc" });
string_enum!(
    /// A one-time coach mark, shown the first time its feature is opened.
    CoachHint { #[default] AreaPicker = "area-picker", QuickTrim = "quick-trim", GoLive = "go-live" }
);

/// What is being captured. `displayId` is the 1-based display number (`rec devices`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CaptureTarget {
    Display {
        #[serde(rename = "displayId")]
        display_id: u32,
    },
    Window {
        #[serde(rename = "windowId")]
        window_id: u32,
    },
    /// Every window of one app (new ones too), on the display holding its frontmost window.
    App {
        #[serde(rename = "bundleId")]
        bundle_id: String,
    },
    /// Part of a display, in points (not pixels) from the display's top-left
    /// corner, like ScreenCaptureKit's `sourceRect` and `rec-rs --region`.
    /// Pixels = points × the display's `scale` (`state::Display`).
    Region {
        #[serde(rename = "displayId")]
        display_id: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

impl Default for CaptureTarget {
    fn default() -> Self {
        CaptureTarget::Display { display_id: 1 }
    }
}

/// Where recordings go by default: the platform's movies folder (`~/Movies` on macOS,
/// `~/Videos` on Windows and Linux).
pub const DEFAULT_OUTPUT_DIR: &str = if cfg!(target_os = "macos") {
    "~/Movies/Cortex"
} else {
    "~/Videos/Cortex"
};

/// The default container: QuickTime on macOS; MP4 elsewhere, because Media Foundation
/// and GStreamer's common muxers write MP4 (see `PlatformCapabilities::mov_container`).
pub const DEFAULT_CONTAINER: Container = if cfg!(target_os = "macos") {
    Container::Mov
} else {
    Container::Mp4
};

/// The last area chosen on one display, in points (as `CaptureTarget::Region`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RegionMemory {
    pub display_id: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// At most this many displays keep a remembered area.
pub const REGION_MEMORY_MAX: usize = 8;

/// Overlay placement: snapped to a corner, or free (centre as 0..1 of the frame).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(untagged)]
pub enum PipPlace {
    Corner { corner: PipCorner },
    Free { x: f64, y: f64 },
}

impl Default for PipPlace {
    fn default() -> Self {
        PipPlace::Corner {
            corner: PipCorner::BottomRight,
        }
    }
}

/// A stream destination. Its key lives in the Keychain only; `key_stored` is
/// filled in by the engine so the UI can show "key stored".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct StreamDest {
    pub service: StreamService,
    /// rtmp(s):// or WHIP http(s):// URL, for `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub enabled: bool,
    pub key_stored: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct RecorderSettings {
    /// May start with `~/` (expanded by the engine).
    pub output_dir: String,
    pub container: Container,
    pub codec: Codec,
    pub resolution: Resolution,
    /// One of `FPS_CHOICES`.
    pub fps: u32,
    /// None = automatic.
    pub bitrate_mbps: Option<f64>,
    pub target: CaptureTarget,
    /// Keep the last area per display, so the region picker reopens on it.
    pub remember_region: bool,
    /// Filled in by `validate` from a `Region` target while `remember_region` is on.
    pub last_regions: Vec<RegionMemory>,
    pub scene: Scene,
    /// Camera unique ID; None = system default.
    pub camera_id: Option<String>,
    /// Also record the raw camera: a second video track in a .mov, a
    /// `<name>.camera.mov` sidecar for .mp4.
    pub camera_track: bool,
    pub camera_resolution: CameraResolution,
    pub mic_enabled: bool,
    /// Microphone unique ID; None = system default.
    pub mic_id: Option<String>,
    pub system_audio: bool,
    pub audio_tracks: AudioTracks,
    pub system_gain_db: f32,
    pub mic_gain_db: f32,
    pub system_muted: bool,
    pub mic_muted: bool,
    /// None = the output device's latency.
    pub system_offset_ms: Option<f64>,
    pub mic_offset_ms: f64,
    pub mic_backend: MicBackend,
    /// Overlay width as a fraction of the frame width.
    pub pip_size: f64,
    pub pip_place: PipPlace,
    pub pip_shape: PipShape,
    pub pip_roundness: f64,
    /// Border width in pixels at 1080p (0 = none).
    pub pip_border: f64,
    /// RRGGBB hex, no `#`.
    pub pip_border_color: String,
    pub pip_shadow: bool,
    pub mirror: bool,
    pub cursor: bool,
    pub clicks: bool,
    /// Seconds; one of `COUNTDOWN_CHOICES`.
    pub countdown: u32,
    pub preview_low_power: bool,
    pub replay_enabled: bool,
    pub replay_secs: u32,
    pub stream_dests: Vec<StreamDest>,
    pub live_resolution: Resolution,
    pub stream_bitrate_kbps: u32,
    pub stream_bitrate_auto: bool,
    pub stream_codec: StreamCodec,
    pub keyframe_secs: f64,
    pub stream_cbr: bool,
    pub stream_audio_kbps: u32,
    // UI-only preferences (the engine ignores them).
    pub theme: ThemePref,
    pub show_hud: bool,
    pub open_at_login: bool,
    pub show_in_dock: bool,
    pub auto_update: bool,
    pub accent_hue: f64,
    /// The first-run tour was finished, skipped or closed. Until then it opens at launch.
    pub onboarding_completed: bool,
    /// Coach marks already dismissed (each shows once).
    pub seen_hints: Vec<CoachHint>,
    /// Soft interface sounds (tour steps). Never played while capturing.
    pub sounds: bool,
}

impl Default for RecorderSettings {
    /// The UI prototype's DEFAULT_SETTINGS, except that devices default to the
    /// system default (None) and no stream key is assumed stored.
    fn default() -> Self {
        let dest = |service, enabled| StreamDest {
            service,
            url: None,
            enabled,
            key_stored: false,
        };
        RecorderSettings {
            output_dir: DEFAULT_OUTPUT_DIR.into(),
            container: DEFAULT_CONTAINER,
            codec: Codec::Auto,
            resolution: Resolution::R1080p,
            fps: 60,
            bitrate_mbps: None,
            target: CaptureTarget::default(),
            remember_region: true,
            last_regions: vec![],
            scene: Scene::ScreenCam,
            camera_id: None,
            camera_track: true,
            camera_resolution: CameraResolution::R1080p,
            mic_enabled: true,
            mic_id: None,
            system_audio: true,
            audio_tracks: AudioTracks::Both,
            system_gain_db: 0.0,
            mic_gain_db: 0.0,
            system_muted: false,
            mic_muted: false,
            system_offset_ms: None,
            mic_offset_ms: 0.0,
            mic_backend: MicBackend::Sck,
            pip_size: 0.22,
            pip_place: PipPlace::default(),
            pip_shape: PipShape::Squircle,
            pip_roundness: 0.35,
            pip_border: 0.0,
            pip_border_color: "ffffff".into(),
            pip_shadow: true,
            mirror: true,
            cursor: true,
            clicks: false,
            countdown: 3,
            preview_low_power: false,
            replay_enabled: false,
            replay_secs: 60,
            stream_dests: vec![
                dest(StreamService::Youtube, true),
                dest(StreamService::Twitch, false),
                dest(StreamService::Kick, false),
            ],
            live_resolution: Resolution::R1080p,
            stream_bitrate_kbps: 6000,
            stream_bitrate_auto: true,
            stream_codec: StreamCodec::Auto,
            keyframe_secs: 2.0,
            stream_cbr: true,
            stream_audio_kbps: 192,
            theme: ThemePref::System,
            show_hud: true,
            open_at_login: false,
            show_in_dock: true,
            auto_update: true,
            accent_hue: 188.0,
            onboarding_completed: false,
            seen_hints: vec![],
            sounds: true,
        }
    }
}

fn clamp_f64(
    v: &mut f64,
    (lo, hi): (f64, f64),
    field: &'static str,
    fixed: &mut Vec<&'static str>,
) {
    let c = if v.is_finite() { v.clamp(lo, hi) } else { lo };
    if c != *v {
        *v = c;
        fixed.push(field);
    }
}

fn clamp_f32(
    v: &mut f32,
    (lo, hi): (f32, f32),
    field: &'static str,
    fixed: &mut Vec<&'static str>,
) {
    let c = if v.is_finite() { v.clamp(lo, hi) } else { 0.0 };
    if c != *v {
        *v = c;
        fixed.push(field);
    }
}

fn clamp_u32(
    v: &mut u32,
    (lo, hi): (u32, u32),
    field: &'static str,
    fixed: &mut Vec<&'static str>,
) {
    let c = (*v).clamp(lo, hi);
    if c != *v {
        *v = c;
        fixed.push(field);
    }
}

/// `RRGGBB` (an optional `#` is dropped, case is lowered), or None if invalid.
pub fn normalize_hex(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches('#');
    (s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit())).then(|| s.to_ascii_lowercase())
}

impl RecorderSettings {
    /// Clamp every value into its legal range, in place. Returns the names of
    /// the fields that changed (empty when the settings were already valid).
    pub fn validate(&mut self) -> Vec<&'static str> {
        let mut fixed = vec![];
        let f = &mut fixed;
        if !FPS_CHOICES.contains(&self.fps) {
            self.fps = 60;
            f.push("fps");
        }
        if !COUNTDOWN_CHOICES.contains(&self.countdown) {
            self.countdown = 3;
            f.push("countdown");
        }
        if let Some(b) = &mut self.bitrate_mbps {
            clamp_f64(b, BITRATE_MBPS_RANGE, "bitrateMbps", f);
        }
        match &mut self.target {
            CaptureTarget::Display { display_id } | CaptureTarget::Region { display_id, .. }
                if *display_id == 0 =>
            {
                *display_id = 1;
                f.push("target");
            }
            _ => {}
        }
        if let CaptureTarget::Region {
            display_id,
            x,
            y,
            w,
            h,
        } = &self.target
        {
            let bad = ![x, y, w, h].iter().all(|v| v.is_finite()) || *w <= 0.0 || *h <= 0.0;
            if bad {
                self.target = CaptureTarget::Display {
                    display_id: *display_id,
                };
                f.push("target");
            }
        }
        self.remember_last_region(f);
        clamp_f32(&mut self.system_gain_db, GAIN_DB_RANGE, "systemGainDb", f);
        clamp_f32(&mut self.mic_gain_db, GAIN_DB_RANGE, "micGainDb", f);
        if let Some(o) = &mut self.system_offset_ms {
            clamp_f64(o, OFFSET_MS_RANGE, "systemOffsetMs", f);
        }
        clamp_f64(&mut self.mic_offset_ms, OFFSET_MS_RANGE, "micOffsetMs", f);
        clamp_f64(&mut self.pip_size, PIP_SIZE_RANGE, "pipSize", f);
        if let PipPlace::Free { x, y } = &mut self.pip_place {
            clamp_f64(x, (0.0, 1.0), "pipPlace", f);
            clamp_f64(y, (0.0, 1.0), "pipPlace", f);
        }
        clamp_f64(&mut self.pip_roundness, (0.0, 1.0), "pipRoundness", f);
        clamp_f64(&mut self.pip_border, (0.0, PIP_BORDER_MAX), "pipBorder", f);
        match normalize_hex(&self.pip_border_color) {
            Some(h) if h == self.pip_border_color => {}
            Some(h) => {
                self.pip_border_color = h;
                f.push("pipBorderColor");
            }
            None => {
                self.pip_border_color = "ffffff".into();
                f.push("pipBorderColor");
            }
        }
        clamp_u32(&mut self.replay_secs, REPLAY_SECS_RANGE, "replaySecs", f);
        clamp_u32(
            &mut self.stream_bitrate_kbps,
            STREAM_KBPS_RANGE,
            "streamBitrateKbps",
            f,
        );
        clamp_u32(
            &mut self.stream_audio_kbps,
            STREAM_AUDIO_KBPS_RANGE,
            "streamAudioKbps",
            f,
        );
        clamp_f64(
            &mut self.keyframe_secs,
            KEYFRAME_SECS_RANGE,
            "keyframeSecs",
            f,
        );
        if self.live_resolution == Resolution::Native {
            self.live_resolution = Resolution::R1080p;
            f.push("liveResolution");
        }
        // Blank custom URLs are dropped (they can't stream anywhere).
        let before = self.stream_dests.len();
        self.stream_dests.retain(|d| {
            d.service != StreamService::Custom
                || d.url.as_deref().is_some_and(|u| !u.trim().is_empty())
        });
        for d in &mut self.stream_dests {
            if d.service != StreamService::Custom && d.url.is_some() {
                d.url = None;
                f.push("streamDests");
            }
        }
        if self.stream_dests.len() != before {
            f.push("streamDests");
        }
        clamp_f64(&mut self.accent_hue, (0.0, 360.0), "accentHue", f);
        let hints = self.seen_hints.len();
        let mut seen = Vec::with_capacity(hints);
        self.seen_hints.retain(|h| {
            let first = !seen.contains(h);
            seen.push(*h);
            first
        });
        if self.seen_hints.len() != hints {
            f.push("seenHints");
        }
        if self.output_dir.trim().is_empty() {
            self.output_dir = RecorderSettings::default().output_dir;
            f.push("outputDir");
        }
        fixed.dedup();
        fixed
    }

    /// `validate` by value.
    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// The output folder with a leading `~` replaced by `home`.
    /// Keep `last_regions` sane, and record a `Region` target as its display's
    /// last area (most recent first). Clears the list when remembering is off.
    fn remember_last_region(&mut self, f: &mut Vec<&'static str>) {
        if !self.remember_region {
            if !self.last_regions.is_empty() {
                self.last_regions.clear();
            }
            return;
        }
        let before = self.last_regions.len();
        self.last_regions.retain(|r| {
            r.display_id > 0
                && [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite())
                && r.w > 0.0
                && r.h > 0.0
        });
        if self.last_regions.len() != before {
            f.push("lastRegions");
        }
        if let CaptureTarget::Region {
            display_id,
            x,
            y,
            w,
            h,
        } = self.target
        {
            self.last_regions.retain(|r| r.display_id != display_id);
            self.last_regions.insert(
                0,
                RegionMemory {
                    display_id,
                    x,
                    y,
                    w,
                    h,
                },
            );
        }
        self.last_regions.truncate(REGION_MEMORY_MAX);
    }

    pub fn output_dir_in(&self, home: &std::path::Path) -> std::path::PathBuf {
        match self.output_dir.strip_prefix('~') {
            Some(rest) => home.join(rest.trim_start_matches(['/', '\\'])),
            None => std::path::PathBuf::from(&self.output_dir),
        }
    }
}

/// Turn any stored settings JSON into current settings.
///
/// - A Rec `settings.json` (snake_case) is imported (`store::rec_value_to_settings`).
/// - `version` older than `SETTINGS_VERSION` runs the migration steps. A file without
///   `version` was written before versioning and counts as version 1.
/// - A newer `version` (a downgrade) keeps every field this build knows.
/// - Unknown fields are dropped; a field with a bad value falls back to its default
///   on its own, without losing the others; then everything is `validate`d.
pub fn migrate(value: Value) -> RecorderSettings {
    let Value::Object(mut map) = value else {
        return RecorderSettings::default();
    };
    if map.contains_key("output_dir")
        || map.contains_key("stream_dests")
        || map.contains_key("pip_place")
    {
        return crate::store::rec_value_to_settings(Value::Object(map));
    }
    let version = map.remove("version").and_then(|v| v.as_u64()).unwrap_or(1);
    if version < 2 {
        // Whoever wrote this file has used Cortex already: no first-run tour.
        map.entry("onboardingCompleted")
            .or_insert(Value::Bool(true));
    }
    lenient(map).validated()
}

/// Deserialize field by field: each known key is taken when its value parses.
fn lenient(map: serde_json::Map<String, Value>) -> RecorderSettings {
    let Ok(Value::Object(mut acc)) = serde_json::to_value(RecorderSettings::default()) else {
        return RecorderSettings::default();
    };
    for (k, v) in map {
        // Unknown keys are dropped (`Option` fields serialize as null, so they are listed).
        if !acc.contains_key(&k) {
            continue;
        }
        let old = acc.insert(k.clone(), v);
        if serde_json::from_value::<RecorderSettings>(Value::Object(acc.clone())).is_err() {
            match old {
                Some(o) => acc.insert(k, o),
                None => acc.remove(&k),
            };
        }
    }
    serde_json::from_value(Value::Object(acc)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_is_camel_case_and_matches_the_ui_shape() {
        let v = serde_json::to_value(RecorderSettings::default()).unwrap();
        assert_eq!(v["outputDir"], "~/Movies/Cortex");
        assert_eq!(v["target"], json!({"kind": "display", "displayId": 1}));
        assert_eq!(v["pipPlace"], json!({"corner": "bottom-right"}));
        assert_eq!(v["scene"], "screen-cam");
        assert_eq!(v["resolution"], "1080p");
        assert_eq!(
            v["streamDests"][0],
            json!({"service": "youtube", "enabled": true, "keyStored": false})
        );
        assert_eq!(v["bitrateMbps"], Value::Null);
        assert_eq!(v["micBackend"], "sck");
        assert_eq!(
            v.as_object().unwrap().len(),
            55,
            "one key per types.ts field"
        );
    }

    #[test]
    fn every_variant_round_trips() {
        for t in [
            CaptureTarget::Display { display_id: 2 },
            CaptureTarget::Window { window_id: 12075 },
            CaptureTarget::App {
                bundle_id: "com.apple.Safari".into(),
            },
            CaptureTarget::Region {
                display_id: 1,
                x: 10.0,
                y: 20.0,
                w: 640.0,
                h: 360.0,
            },
        ] {
            let s = RecorderSettings {
                target: t.clone(),
                pip_place: PipPlace::Free { x: 0.3, y: 0.6 },
                ..Default::default()
            };
            let back: RecorderSettings =
                serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
            assert_eq!(back, s);
        }
        let r: CaptureTarget =
            serde_json::from_value(json!({"kind":"region","displayId":2,"x":0,"y":0,"w":5,"h":5}))
                .unwrap();
        assert!(matches!(r, CaptureTarget::Region { display_id: 2, .. }));
        assert_eq!(
            serde_json::from_value::<PipPlace>(json!({"x": 0.1, "y": 0.9})).unwrap(),
            PipPlace::Free { x: 0.1, y: 0.9 }
        );
        for s in Resolution::ALL {
            assert_eq!(Resolution::parse(s.as_str()), Some(*s));
        }
    }

    #[test]
    fn defaults_are_valid() {
        let mut s = RecorderSettings::default();
        assert!(s.validate().is_empty());
    }

    #[test]
    fn validate_clamps_out_of_range_values() {
        let mut s = RecorderSettings {
            pip_size: 0.9,
            system_gain_db: -100.0,
            mic_gain_db: 40.0,
            replay_secs: 5,
            fps: 45,
            countdown: 7,
            pip_border_color: "#FF8800".into(),
            pip_place: PipPlace::Free { x: 1.5, y: -0.2 },
            target: CaptureTarget::Region {
                display_id: 0,
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 10.0,
            },
            keyframe_secs: f64::NAN,
            stream_dests: vec![StreamDest {
                service: StreamService::Custom,
                url: Some("  ".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let fixed = s.validate();
        assert_eq!(s.pip_size, 0.5);
        assert_eq!((s.system_gain_db, s.mic_gain_db), (-60.0, 24.0));
        assert_eq!(s.replay_secs, 15);
        assert_eq!((s.fps, s.countdown), (60, 3));
        assert_eq!(s.pip_border_color, "ff8800");
        assert_eq!(s.pip_place, PipPlace::Free { x: 1.0, y: 0.0 });
        assert_eq!(s.target, CaptureTarget::Display { display_id: 1 });
        assert_eq!(s.keyframe_secs, 0.5);
        assert!(s.stream_dests.is_empty());
        for f in [
            "pipSize",
            "systemGainDb",
            "micGainDb",
            "replaySecs",
            "fps",
            "countdown",
            "pipBorderColor",
            "pipPlace",
            "target",
            "streamDests",
        ] {
            assert!(fixed.contains(&f), "{f} in {fixed:?}");
        }
        assert!(s.clone().validate().is_empty(), "validate is idempotent");
        s.pip_size = 0.01;
        s.replay_secs = 900;
        s.validate();
        assert_eq!((s.pip_size, s.replay_secs), (0.1, 300));
    }

    #[test]
    fn region_targets_are_remembered_per_display() {
        let region = |display_id, x| CaptureTarget::Region {
            display_id,
            x,
            y: 10.0,
            w: 640.0,
            h: 360.0,
        };
        let mut s = RecorderSettings {
            target: region(1, 0.0),
            ..Default::default()
        };
        assert!(s.validate().is_empty(), "remembering is not a repair");
        s.target = region(2, 5.0);
        s.validate();
        s.target = region(1, 20.0);
        s.validate();
        let got: Vec<(u32, f64)> = s.last_regions.iter().map(|r| (r.display_id, r.x)).collect();
        assert_eq!(got, vec![(1, 20.0), (2, 5.0)]);
        // A bad entry from disk is dropped; turning remembering off forgets all.
        s.last_regions.push(RegionMemory {
            display_id: 3,
            x: 0.0,
            y: 0.0,
            w: -1.0,
            h: 1.0,
        });
        assert_eq!(s.validate(), vec!["lastRegions"]);
        s.remember_region = false;
        s.validate();
        assert!(s.last_regions.is_empty());
    }

    #[test]
    fn migrate_keeps_good_fields_and_repairs_bad_ones() {
        let s = migrate(json!({
            "version": 1,
            "fps": 30,
            "scene": "nope",
            "pipSize": 2.0,
            "micId": "BuiltInMic",
            "somethingNew": true,
            "target": {"kind": "window", "windowId": 7}
        }));
        assert_eq!(s.fps, 30);
        assert_eq!(s.scene, Scene::ScreenCam, "bad enum value falls back alone");
        assert_eq!(s.pip_size, 0.5, "validated");
        assert_eq!(s.mic_id.as_deref(), Some("BuiltInMic"));
        assert_eq!(s.target, CaptureTarget::Window { window_id: 7 });
        // No version (written before versioning) and a newer one load the same way.
        assert_eq!(migrate(json!({"fps": 24})).fps, 24);
        assert_eq!(migrate(json!({"version": 99, "fps": 24})).fps, 24);
        assert_eq!(migrate(json!("garbage")), RecorderSettings::default());
    }

    #[test]
    fn version_1_files_count_as_onboarded() {
        // Existing users skip the first-run tour; new files keep what they stored.
        assert!(migrate(json!({"version": 1, "fps": 30})).onboarding_completed);
        assert!(migrate(json!({"fps": 30})).onboarding_completed);
        assert!(!migrate(json!({"version": 2, "fps": 30})).onboarding_completed);
        assert!(!migrate(json!({"version": 1, "onboardingCompleted": false})).onboarding_completed);
        assert!(!RecorderSettings::default().onboarding_completed);
    }

    #[test]
    fn seen_hints_round_trip_and_dedupe() {
        let s = migrate(json!({
            "version": 2,
            "seenHints": ["go-live", "area-picker", "go-live"],
            "sounds": false
        }));
        assert_eq!(s.seen_hints, vec![CoachHint::GoLive, CoachHint::AreaPicker]);
        assert!(!s.sounds);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["seenHints"], json!(["go-live", "area-picker"]));
        // A hint this build does not know drops the list, never the other settings.
        let s = migrate(json!({"version": 2, "seenHints": ["future"], "fps": 30}));
        assert!(s.seen_hints.is_empty());
        assert_eq!(s.fps, 30);
    }

    #[test]
    fn output_dir_expands_home() {
        let s = RecorderSettings::default();
        assert_eq!(
            s.output_dir_in("/Users/a".as_ref()),
            std::path::PathBuf::from("/Users/a/Movies/Cortex")
        );
        let s = RecorderSettings {
            output_dir: "/tmp/x".into(),
            ..s
        };
        assert_eq!(
            s.output_dir_in("/Users/a".as_ref()),
            std::path::PathBuf::from("/tmp/x")
        );
    }
}
