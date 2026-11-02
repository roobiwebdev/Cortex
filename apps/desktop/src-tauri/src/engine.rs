//! Engine state as the UI sees it, plus a stub engine that fakes transitions.
//!
//! These types mirror `apps/desktop/src/lib/types.ts` field for field so the
//! generated TypeScript bindings line up with the UI. They move into the
//! platform-neutral `rec-core` crate (engineering plan §3) once it exists, and
//! the stub below is replaced by `rec-engine-macos`.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Container {
    Mov,
    Mp4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Auto,
    H264,
    Hevc,
    Prores,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Resolution {
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "4k")]
    K4,
    #[serde(rename = "native")]
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum CameraResolution {
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Scene {
    Screen,
    ScreenCam,
    Cam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum PipShape {
    Rounded,
    Circle,
    Square,
    Pill,
    Squircle,
    Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum PipCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum AudioTracks {
    Both,
    Mix,
    Stems,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum StreamService {
    Youtube,
    Twitch,
    Kick,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum StreamCodec {
    Auto,
    H264,
    Hevc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum MicBackend {
    Sck,
    Avcapture,
}

/// What is being captured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
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
    App {
        #[serde(rename = "bundleId")]
        bundle_id: String,
    },
    Region {
        #[serde(rename = "displayId")]
        display_id: u32,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

/// Overlay placement: snapped to a corner, or free (centre as 0..1 of the frame).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(untagged)]
pub enum PipPlace {
    Corner { corner: PipCorner },
    Free { x: f64, y: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StreamDest {
    pub service: StreamService,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub url: Option<String>,
    pub enabled: bool,
    pub key_stored: bool,
}

/// Every recorder option. `fps` (24|30|60) and `countdown` (0|3|5) are plain
/// numbers on the wire; the UI narrows them (see `src/lib/engine.ts`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecorderSettings {
    pub output_dir: String,
    pub container: Container,
    pub codec: Codec,
    pub resolution: Resolution,
    pub fps: u32,
    pub bitrate_mbps: Option<f64>,
    pub target: CaptureTarget,
    pub scene: Scene,
    pub camera_id: Option<String>,
    pub camera_track: bool,
    pub camera_resolution: CameraResolution,
    pub mic_enabled: bool,
    pub mic_id: Option<String>,
    pub system_audio: bool,
    pub audio_tracks: AudioTracks,
    pub system_gain_db: f64,
    pub mic_gain_db: f64,
    pub system_muted: bool,
    pub mic_muted: bool,
    pub system_offset_ms: Option<f64>,
    pub mic_offset_ms: f64,
    pub mic_backend: MicBackend,
    pub pip_size: f64,
    pub pip_place: PipPlace,
    pub pip_shape: PipShape,
    pub pip_roundness: f64,
    pub pip_border: f64,
    pub pip_border_color: String,
    pub pip_shadow: bool,
    pub mirror: bool,
    pub cursor: bool,
    pub clicks: bool,
    pub countdown: u32,
    pub preview_low_power: bool,
    pub replay_enabled: bool,
    pub replay_secs: f64,
    pub stream_dests: Vec<StreamDest>,
    pub live_resolution: Resolution,
    pub stream_bitrate_kbps: f64,
    pub stream_bitrate_auto: bool,
    pub stream_codec: StreamCodec,
    pub keyframe_secs: f64,
    pub stream_cbr: bool,
    pub stream_audio_kbps: f64,
    pub theme: ThemePref,
    pub show_hud: bool,
    pub open_at_login: bool,
    pub show_in_dock: bool,
    pub auto_update: bool,
    pub accent_hue: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Display {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct CaptureWindow {
    pub id: u32,
    pub app: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureApp {
    pub bundle_id: String,
    pub name: String,
    pub windows: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionState {
    Granted,
    Denied,
    NotDetermined,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Permissions {
    pub screen: PermissionState,
    pub microphone: PermissionState,
    pub camera: PermissionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum EngineStatus {
    Idle,
    Countdown,
    Recording,
    Paused,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Levels {
    pub system: f64,
    pub system_peak: f64,
    pub mic: f64,
    pub mic_peak: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub name: String,
    pub path: String,
    pub duration_secs: f64,
    pub bytes: f64,
    /// Unix epoch milliseconds.
    pub created_at: f64,
    pub width: u32,
    pub height: u32,
    pub has_camera_track: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum ToastKind {
    Saved,
    Replay,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Toast {
    pub id: String,
    pub kind: ToastKind,
    pub title: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub recording_id: Option<String>,
}

/// Everything the UI renders. Sent whole on every change (`engine://state`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EngineState {
    pub status: EngineStatus,
    pub countdown_left: u32,
    pub elapsed_secs: f64,
    pub bytes_written: f64,
    pub dropped_frames: u32,
    pub live: bool,
    pub replay_buffered_secs: f64,
    pub levels: Levels,
    pub permissions: Permissions,
    pub displays: Vec<Display>,
    pub windows: Vec<CaptureWindow>,
    pub apps: Vec<CaptureApp>,
    pub cameras: Vec<InputDevice>,
    pub mics: Vec<InputDevice>,
    pub recordings: Vec<Recording>,
    pub toasts: Vec<Toast>,
    pub settings: RecorderSettings,
    pub free_disk_bytes: f64,
}

impl Default for RecorderSettings {
    fn default() -> Self {
        Self {
            output_dir: "~/Movies/Cortex".into(),
            container: Container::Mov,
            codec: Codec::Auto,
            resolution: Resolution::P1080,
            fps: 60,
            bitrate_mbps: None,
            target: CaptureTarget::Display { display_id: 1 },
            scene: Scene::ScreenCam,
            camera_id: None,
            camera_track: true,
            camera_resolution: CameraResolution::P1080,
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
            pip_place: PipPlace::Corner {
                corner: PipCorner::BottomRight,
            },
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
            replay_secs: 60.0,
            stream_dests: Vec::new(),
            live_resolution: Resolution::P1080,
            stream_bitrate_kbps: 6000.0,
            stream_bitrate_auto: true,
            stream_codec: StreamCodec::Auto,
            keyframe_secs: 2.0,
            stream_cbr: true,
            stream_audio_kbps: 192.0,
            theme: ThemePref::System,
            show_hud: true,
            open_at_login: false,
            show_in_dock: true,
            auto_update: true,
            accent_hue: 188.0,
        }
    }
}

impl Default for EngineState {
    /// Stub snapshot: no devices are enumerated yet and permissions are unknown.
    fn default() -> Self {
        Self {
            status: EngineStatus::Idle,
            countdown_left: 0,
            elapsed_secs: 0.0,
            bytes_written: 0.0,
            dropped_frames: 0,
            live: false,
            replay_buffered_secs: 0.0,
            levels: Levels {
                system: 0.0,
                system_peak: 0.0,
                mic: 0.0,
                mic_peak: 0.0,
            },
            permissions: Permissions {
                screen: PermissionState::NotDetermined,
                microphone: PermissionState::NotDetermined,
                camera: PermissionState::NotDetermined,
            },
            displays: vec![Display {
                id: 1,
                name: "Main display".into(),
                width: 1920,
                height: 1080,
            }],
            windows: Vec::new(),
            apps: Vec::new(),
            cameras: Vec::new(),
            mics: Vec::new(),
            recordings: Vec::new(),
            toasts: Vec::new(),
            settings: RecorderSettings::default(),
            free_disk_bytes: 0.0,
        }
    }
}

/// Stub engine: owns the state and applies the transitions the real engine will.
/// Nothing is captured; `tick` only advances the clock while "recording".
#[derive(Default)]
pub struct StubEngine(Mutex<EngineState>);

impl StubEngine {
    pub fn snapshot(&self) -> EngineState {
        self.lock().clone()
    }

    /// Applies `f` and returns the new state.
    pub fn apply(&self, f: impl FnOnce(&mut EngineState)) -> EngineState {
        let mut s = self.lock();
        f(&mut s);
        s.clone()
    }

    /// One-second clock. Returns the new state only when something changed.
    pub fn tick(&self) -> Option<EngineState> {
        let mut s = self.lock();
        if s.status != EngineStatus::Recording {
            return None;
        }
        s.elapsed_secs += 1.0;
        s.bytes_written += 1_100_000.0;
        Some(s.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, EngineState> {
        // A panic while holding the lock leaves plain data behind; keep serving it.
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
