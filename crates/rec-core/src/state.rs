//! SOURCE OF TRUTH KEYWORDS: EngineState, EngineStatus, Levels, Recording, Permission, PermissionState, Devices, Progress
//! WHAT:  What the engine reports to the UI: status, clock, levels, permissions, devices, recordings.
//! WHY:   Mirrors the UI's `EngineState` (types.ts) minus the UI-owned parts (toasts, and the
//!        settings, which the settings store owns), so the Tauri shell can forward it as is.
//! WHERE: `api` (events, trait), the macOS engine, the Tauri shell.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum EngineStatus {
    #[default]
    Idle,
    Countdown,
    Recording,
    Paused,
    Finalizing,
}

impl EngineStatus {
    /// A file is being written (recording or paused).
    pub fn is_capturing(self) -> bool {
        matches!(self, EngineStatus::Recording | EngineStatus::Paused)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    Screen,
    Microphone,
    Camera,
}

impl Permission {
    pub const ALL: [Permission; 3] = [
        Permission::Screen,
        Permission::Microphone,
        Permission::Camera,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum PermissionState {
    Granted,
    /// Also covers "restricted" (parental controls / MDM): the user can't grant it here.
    Denied,
    #[default]
    NotDetermined,
}

/// `Record<Permission, PermissionState>` on the UI side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Permissions {
    pub screen: PermissionState,
    pub microphone: PermissionState,
    pub camera: PermissionState,
}

impl Permissions {
    pub fn get(&self, p: Permission) -> PermissionState {
        match p {
            Permission::Screen => self.screen,
            Permission::Microphone => self.microphone,
            Permission::Camera => self.camera,
        }
    }
    pub fn set(&mut self, p: Permission, s: PermissionState) {
        match p {
            Permission::Screen => self.screen = s,
            Permission::Microphone => self.microphone = s,
            Permission::Camera => self.camera = s,
        }
    }
}

/// Linear levels 0..1 (1 = full scale); peaks hold and decay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Levels {
    pub system: f32,
    pub system_peak: f32,
    pub mic: f32,
    pub mic_peak: f32,
}

/// Peak decay per second for `Levels::feed` (in linear units, as the UI mock does).
pub const PEAK_DECAY_PER_SEC: f32 = 0.36;

impl Levels {
    /// Feed new [system, mic] peak levels measured over `dt` seconds.
    pub fn feed(&mut self, [system, mic]: [f32; 2], dt: f32) {
        let decay = PEAK_DECAY_PER_SEC * dt.max(0.0);
        let clamp = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        self.system = clamp(system);
        self.mic = clamp(mic);
        self.system_peak = self.system.max(self.system_peak - decay);
        self.mic_peak = self.mic.max(self.mic_peak - decay);
    }
}

/// A display; `id` is the 1-based display number (`CaptureTarget::Display`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Display {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CaptureWindow {
    pub id: u32,
    pub app: String,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CaptureApp {
    pub bundle_id: String,
    pub name: String,
    pub windows: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub connected: bool,
}

/// Every capture source the engine can see right now.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Devices {
    pub displays: Vec<Display>,
    pub windows: Vec<CaptureWindow>,
    pub apps: Vec<CaptureApp>,
    pub cameras: Vec<InputDevice>,
    pub mics: Vec<InputDevice>,
}

/// Group windows into apps (one entry per bundle ID, first-seen order).
pub fn apps_from_windows<'a>(
    windows: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Vec<CaptureApp> {
    let mut apps: Vec<CaptureApp> = vec![];
    for (bundle_id, name) in windows {
        if bundle_id.is_empty() {
            continue;
        }
        match apps.iter_mut().find(|a| a.bundle_id == bundle_id) {
            Some(a) => a.windows += 1,
            None => apps.push(CaptureApp {
                bundle_id: bundle_id.into(),
                name: name.into(),
                windows: 1,
            }),
        }
    }
    apps
}

/// A finished recording.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub name: String,
    pub path: String,
    pub duration_secs: f64,
    pub bytes: u64,
    /// Unix time in milliseconds.
    pub created_at: f64,
    pub width: u32,
    pub height: u32,
    pub has_camera_track: bool,
    pub fps: u32,
    /// Number of audio tracks in the file.
    pub audio_tracks: u32,
}

/// One running stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StreamStatus {
    /// Destination name (YouTube, Twitch, a host) — never the key.
    pub name: String,
    pub live: bool,
    /// Human-readable connection state.
    pub state: String,
    pub bitrate_kbps: u32,
    pub dropped_frames: u64,
}

/// The fast-changing numbers, sent up to 10 times a second while capturing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub countdown_left: u32,
    /// Recorded seconds, excluding pauses.
    pub elapsed_secs: f64,
    pub bytes_written: u64,
    pub dropped_frames: u64,
    pub replay_buffered_secs: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EngineState {
    pub status: EngineStatus,
    pub countdown_left: u32,
    pub elapsed_secs: f64,
    pub bytes_written: u64,
    pub dropped_frames: u64,
    /// Streaming to at least one destination.
    pub live: bool,
    pub streams: Vec<StreamStatus>,
    pub replay_buffered_secs: f64,
    pub levels: Levels,
    pub permissions: Permissions,
    pub displays: Vec<Display>,
    pub windows: Vec<CaptureWindow>,
    pub apps: Vec<CaptureApp>,
    pub cameras: Vec<InputDevice>,
    pub mics: Vec<InputDevice>,
    /// Recordings made in this run, newest first.
    pub recordings: Vec<Recording>,
    pub free_disk_bytes: u64,
    /// The output device's latency (the default system-audio offset).
    pub output_latency_ms: Option<f64>,
    /// A preview session is running (frames go to the preview sink).
    pub previewing: bool,
}

impl EngineState {
    pub fn set_devices(&mut self, d: Devices) {
        self.displays = d.displays;
        self.windows = d.windows;
        self.apps = d.apps;
        self.cameras = d.cameras;
        self.mics = d.mics;
    }

    pub fn set_progress(&mut self, p: &Progress) {
        self.countdown_left = p.countdown_left;
        self.elapsed_secs = p.elapsed_secs;
        self.bytes_written = p.bytes_written;
        self.dropped_frames = p.dropped_frames;
        self.replay_buffered_secs = p.replay_buffered_secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_names_match_the_ui() {
        assert_eq!(
            serde_json::to_value(EngineStatus::Finalizing).unwrap(),
            "finalizing"
        );
        assert_eq!(
            serde_json::to_value(PermissionState::NotDetermined).unwrap(),
            "not-determined"
        );
        let v = serde_json::to_value(EngineState::default()).unwrap();
        for k in [
            "countdownLeft",
            "elapsedSecs",
            "bytesWritten",
            "droppedFrames",
            "replayBufferedSecs",
            "freeDiskBytes",
        ] {
            assert!(v.get(k).is_some(), "{k}");
        }
        assert_eq!(
            v["permissions"],
            json!({"screen": "not-determined", "microphone": "not-determined", "camera": "not-determined"})
        );
        assert_eq!(
            v["levels"],
            json!({"system": 0.0, "systemPeak": 0.0, "mic": 0.0, "micPeak": 0.0})
        );
    }

    #[test]
    fn peaks_hold_then_decay() {
        let mut l = Levels::default();
        l.feed([0.8, 0.0], 0.0);
        l.feed([0.1, 2.0], 0.5);
        assert_eq!(l.system, 0.1);
        assert!((l.system_peak - (0.8 - 0.18)).abs() < 1e-6);
        assert_eq!((l.mic, l.mic_peak), (1.0, 1.0), "clamped to full scale");
        l.feed([f32::NAN, 0.0], 10.0);
        assert_eq!((l.system, l.system_peak, l.mic_peak), (0.0, 0.0, 0.0));
    }

    #[test]
    fn windows_group_into_apps() {
        let apps = apps_from_windows([("com.a", "A"), ("com.b", "B"), ("com.a", "A"), ("", "x")]);
        assert_eq!(apps.len(), 2);
        assert_eq!((apps[0].bundle_id.as_str(), apps[0].windows), ("com.a", 2));
    }

    #[test]
    fn permissions_get_set() {
        let mut p = Permissions::default();
        p.set(Permission::Camera, PermissionState::Granted);
        assert_eq!(p.get(Permission::Camera), PermissionState::Granted);
        assert_eq!(p.get(Permission::Screen), PermissionState::NotDetermined);
    }
}
