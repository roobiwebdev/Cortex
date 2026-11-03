//! SOURCE OF TRUTH KEYWORDS: PlatformCapabilities, PlatformOs, PreviewKind, SourcePicker, EngineOptions, MediaLibrary, MediaInfo, Secrets, platform contract
//! WHAT:  The platform-neutral half of the contract a platform implements: what the platform
//!        can do (`PlatformCapabilities`, sent to the UI inside `EngineState`), how the shell
//!        starts an engine (`EngineOptions`), the recordings-library media backend
//!        (`MediaLibrary`) and the secret store (`Secrets`). The recorder itself is
//!        `api::RecorderEngine`.
//! WHY:   macOS, Windows and Linux engines implement the same traits over the same types, so
//!        the Tauri shell, the IPC bindings and the React surfaces do not change per platform.
//!        Only std and rec-core types appear here, so an engine crate needs nothing else.
//!        Window-handle work (preview view, floating panels, hotkeys) needs Tauri and lives
//!        shell-side in `apps/desktop/src-tauri/src/platform`.
//! WHERE: `rec-engine-macos` (`MacEngine`), the shell's `platform` module and its stub
//!        engines; `docs/engineering/platforms.md` maps every capability.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::library::{KeepTracks, TrimMode};
use crate::state::Permission;

/// The operating system an engine runs on (UI copy: "Finder" vs "File Explorer").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum PlatformOs {
    #[default]
    Macos,
    Windows,
    Linux,
}

impl PlatformOs {
    /// The OS this build targets.
    pub const fn current() -> PlatformOs {
        if cfg!(target_os = "windows") {
            PlatformOs::Windows
        } else if cfg!(target_os = "linux") {
            PlatformOs::Linux
        } else {
            PlatformOs::Macos
        }
    }
}

/// Who picks what is captured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum SourcePicker {
    /// The app lists displays, windows and apps and the user picks in Cortex.
    #[default]
    App,
    /// The system shows its own picker (Wayland's ScreenCast portal); the app cannot list
    /// or name windows and gets whatever the user chose there.
    System,
}

/// How the live preview reaches the main window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "kebab-case")]
pub enum PreviewKind {
    /// A native view under a transparent hole in the webview (macOS today).
    Native,
    /// Frames are drawn by the webview (lower rate), for webviews that cannot be transparent.
    Fallback,
    /// No preview.
    #[default]
    None,
}

/**
 * SOURCE OF TRUTH KEYWORDS: PlatformCapabilities, capabilities, feature availability, prores, hevc, capture exclusion, global hotkeys, per-app audio
 * WHAT:  What the running platform engine can do. Part of `EngineState`, so every window
 *        knows before it offers a choice.
 * WHY:   Windows and Linux differ from macOS (Wayland only captures what its portal picked,
 *        ProRes is Apple-only, some compositors can't hide our windows). The UI hides or
 *        explains an unavailable feature instead of failing when the user picks it.
 * WHERE: Built by each engine (`rec_engine::engine::mac_capabilities`, the shell's stub
 *        engines); read in React through `useEngine((s) => s.capabilities)`.
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub os: PlatformOs,
    /// The engine can record at all (false for the not-yet-built Windows and Linux engines).
    pub recording: bool,
    /// Whole display, a region of it, one window, every window of one app.
    pub display_capture: bool,
    pub region_capture: bool,
    pub window_capture: bool,
    pub app_capture: bool,
    pub source_picker: SourcePicker,
    /// Everything the computer plays, as its own track.
    pub system_audio: bool,
    /// Only the captured app's sound (app capture).
    pub per_app_audio: bool,
    /// Cortex's own windows (HUD, toasts, countdown) are kept out of recordings and screenshots.
    pub capture_exclusion: bool,
    /// Record/stop and save-replay shortcuts work while another app is in front.
    pub global_hotkeys: bool,
    /// Hardware or software encoders for these codecs exist (H.264 is always required).
    pub hevc: bool,
    pub prores: bool,
    pub preview: PreviewKind,
    /// Go live (RTMP/WHIP) and the replay buffer.
    pub live_streaming: bool,
    pub replay_buffer: bool,
    /// The recordings library can inspect, thumbnail and trim files.
    pub quick_trim: bool,
}

impl PlatformCapabilities {
    /// Nothing available: what an engine that cannot record yet reports.
    pub const fn unavailable(os: PlatformOs) -> PlatformCapabilities {
        PlatformCapabilities {
            os,
            recording: false,
            display_capture: false,
            region_capture: false,
            window_capture: false,
            app_capture: false,
            source_picker: SourcePicker::App,
            system_audio: false,
            per_app_audio: false,
            capture_exclusion: false,
            global_hotkeys: false,
            hevc: false,
            prores: false,
            preview: PreviewKind::None,
            live_streaming: false,
            replay_buffer: false,
            quick_trim: false,
        }
    }
}

/// How the shell runs an engine; not user settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngineOptions {
    /// Expands `~` in the output folder.
    pub home: PathBuf,
    /// Record the generated test pattern instead of the screen (self-tests; needs no
    /// screen-capture permission).
    pub test_pattern: bool,
    /// Stop recordings automatically after this many recorded seconds.
    pub max_duration_secs: Option<f64>,
    /// Treat these grants as missing (self-tests of the no-permission paths).
    pub withheld: Vec<Permission>,
    /// Fail a capture rather than record the app's own windows (the desktop app; the CLI
    /// has no windows and leaves it off).
    pub require_self_exclusion: bool,
}

/// One audio track as a recording file stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioTrackInfo {
    pub channels: u32,
    pub enabled: bool,
}

/// What the recordings list shows about a file.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// Video tracks: the screen first, then a separate camera track if there is one.
    pub video_tracks: u32,
    pub audio: Vec<AudioTrackInfo>,
    /// The first video track is HEVC (`hvc1`/`hev1`).
    pub hevc: bool,
    pub playable: bool,
}

/// What `MediaLibrary::export` writes.
pub struct ExportOptions<'a> {
    pub src: &'a Path,
    /// Must not exist yet; `.mp4` writes MPEG-4, anything else QuickTime.
    pub dst: &'a Path,
    pub start_secs: f64,
    pub end_secs: f64,
    pub mode: TrimMode,
    pub keep: KeepTracks,
}

/// Why a media call failed. `Failed` carries a user-facing message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaError {
    /// The export was cancelled through its flag (the partial file is removed).
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaError::Cancelled => f.write_str("cancelled"),
            MediaError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for MediaError {}

/**
 * SOURCE OF TRUTH KEYWORDS: MediaLibrary, inspect, thumbnails, waveform, export, trash, quick trim backend
 * WHAT:  The platform media backend behind the recordings list and Quick trim.
 * WHY:   The library rules (`library.rs`) are neutral; reading tracks, decoding frames and
 *        exporting a range are not (AVFoundation on macOS, Media Foundation or GStreamer
 *        elsewhere). Any thread may call; calls block.
 * WHERE: The shell's `trim.rs` through `platform::media()`.
 */
pub trait MediaLibrary: Send + Sync {
    /// Duration, size, frame rate and tracks of a recording.
    fn inspect(&self, path: &Path) -> Result<MediaInfo, MediaError>;
    /// `count` JPEG frames spread over the file, at most `max_width` pixels wide.
    fn thumbnails(
        &self,
        path: &Path,
        count: u32,
        max_width: f64,
    ) -> Result<Vec<Vec<u8>>, MediaError>;
    /// Peak level (0..1) of the played audio in `buckets` equal slices; empty without audio.
    fn waveform(&self, path: &Path, buckets: u32) -> Result<Vec<f32>, MediaError>;
    /// Write `start..end` of `src` to `dst` with the kept tracks. `progress` gets 0..1;
    /// setting `cancel` stops the export with `MediaError::Cancelled`.
    fn export(
        &self,
        options: &ExportOptions,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(f64),
    ) -> Result<(), MediaError>;
    /// Move a file to the Trash / Recycle Bin (never delete outright).
    fn trash(&self, path: &Path) -> Result<(), MediaError>;
}

/**
 * SOURCE OF TRUTH KEYWORDS: Secrets, stream key store, Keychain, Credential Manager, Secret Service
 * WHAT:  The OS secret store for stream keys, keyed by account (the destination's URL).
 * WHY:   Keys never go to the settings file, the webview or logs. Each OS has its own store:
 *        macOS Keychain, Windows Credential Manager, Linux Secret Service.
 * WHERE: `rec_engine::live::keychain::Keychain` on macOS (the engine reads keys from it to
 *        go live); future engines take one at construction.
 */
pub trait Secrets: Send + Sync {
    fn get(&self, account: &str) -> Option<String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_use_camel_case_on_the_wire() {
        let c = PlatformCapabilities::unavailable(PlatformOs::Linux);
        let v = serde_json::to_value(c).unwrap();
        assert_eq!(v["os"], "linux");
        assert_eq!(v["sourcePicker"], "app");
        assert_eq!(v["preview"], "none");
        assert_eq!(v["prores"], false);
        assert_eq!(v["globalHotkeys"], false);
    }

    #[test]
    fn the_default_is_nothing_available() {
        let c = PlatformCapabilities::default();
        assert_eq!(c, PlatformCapabilities::unavailable(PlatformOs::Macos));
        assert!(!c.recording);
    }

    #[test]
    fn media_errors_read_like_the_old_messages() {
        assert_eq!(MediaError::Cancelled.to_string(), "cancelled");
        assert_eq!(
            MediaError::Failed("no video".into()).to_string(),
            "no video"
        );
    }
}
