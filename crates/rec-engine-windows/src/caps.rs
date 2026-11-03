//! SOURCE OF TRUTH KEYWORDS: windows_capabilities, Probe, Windows build, WDA_EXCLUDEFROMCAPTURE, borderless capture, consent store, privacy, stream key target
//! WHAT:  What the Windows engine can do on this PC (`PlatformCapabilities` from a probe of
//!        the OS build, Windows.Graphics.Capture support and the HEVC encoder), how the
//!        privacy "consent store" values read as permission states, and the Credential
//!        Manager names for stream keys.
//! WHY:   The UI hides or explains what a platform can't do. Windows 10 before 2004 can't
//!        keep our windows out of a capture, the yellow capture border can only be turned
//!        off on Windows 11, and HEVC depends on the GPU. Pure, so it is unit-tested.
//! WHERE: `WindowsEngine::capabilities`, `win::devices` (permissions), `win::secrets`.

use rec_core::platform::{PlatformCapabilities, PlatformOs, PreviewKind, SourcePicker};
use rec_core::settings::{StreamDest, StreamService};
use rec_core::state::PermissionState;

/// Windows 10 2004: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`.
pub const BUILD_EXCLUDE_FROM_CAPTURE: u32 = 19_041;
/// Windows 11: `GraphicsCaptureSession.IsBorderRequired` can turn the yellow border off.
pub const BUILD_BORDERLESS: u32 = 22_000;

/// What was found on this PC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    /// Windows build number (19045 = Windows 10 22H2, 22631 = Windows 11 23H2).
    pub build: u32,
    /// `GraphicsCaptureSession::IsSupported()`.
    pub capture: bool,
    /// An HEVC encoder MFT is registered (hardware, usually).
    pub hevc: bool,
}

impl Probe {
    pub fn capture_exclusion(&self) -> bool {
        self.build >= BUILD_EXCLUDE_FROM_CAPTURE
    }
    pub fn borderless(&self) -> bool {
        self.build >= BUILD_BORDERLESS
    }
}

/// The Windows engine's capabilities.
///
/// - App capture is off: Windows has no "every window of one app" capture. A stored app
///   target records the app's largest window instead (see `win::devices::main_window_of`).
/// - Preview is `Fallback`: there is no native view under the webview yet, and no frames
///   are sent to the page either; the UI shows a placeholder.
/// - Replay buffer, Go live, the camera track, Quick trim and `.mov` are not built.
pub fn windows_capabilities(p: &Probe) -> PlatformCapabilities {
    PlatformCapabilities {
        os: PlatformOs::Windows,
        recording: p.capture,
        display_capture: p.capture,
        region_capture: p.capture,
        window_capture: p.capture,
        app_capture: false,
        source_picker: SourcePicker::App,
        system_audio: true,
        per_app_audio: false,
        capture_exclusion: p.capture_exclusion(),
        global_hotkeys: true,
        hevc: p.hevc,
        prores: false,
        preview: PreviewKind::Fallback,
        live_streaming: false,
        replay_buffer: false,
        quick_trim: false,
        mov_container: false,
        camera_track: false,
    }
}

/// A privacy switch as Windows stores it under
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\<cap>`:
/// `global` is that key's `Value`, `desktop` the `NonPackaged` subkey's ("Let desktop apps
/// access ..."). "Deny" in either blocks a desktop app; a missing value means allowed.
pub fn consent_state(global: Option<&str>, desktop: Option<&str>) -> PermissionState {
    let denied = |v: Option<&str>| v.is_some_and(|v| v.trim().eq_ignore_ascii_case("deny"));
    if denied(global) || denied(desktop) {
        PermissionState::Denied
    } else {
        PermissionState::Granted
    }
}

/// The ingest URLs the macOS engine keys its Keychain entries by
/// (`rec_engine::live::SERVICES`), so a destination has the same account on every OS.
const SERVICE_URLS: [(StreamService, &str); 3] = [
    (
        StreamService::Youtube,
        "rtmps://a.rtmps.youtube.com:443/live2",
    ),
    (StreamService::Twitch, "rtmps://live.twitch.tv:443/app"),
    (
        StreamService::Kick,
        "rtmps://fa723fc1b171.global-contribute.live-video.net:443/app",
    ),
];

/// The account a destination's stream key is stored under: its stream URL.
pub fn dest_url(d: &StreamDest) -> String {
    match SERVICE_URLS.iter().find(|(s, _)| *s == d.service) {
        Some((_, url)) => (*url).to_string(),
        None => d.url.clone().unwrap_or_default().trim().to_string(),
    }
}

/// The Credential Manager target name for an account (a generic credential).
pub fn credential_target(account: &str) -> String {
    format!("Cortex stream key:{account}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_10_2004_keeps_our_windows_out() {
        let old = Probe {
            build: 18_363,
            capture: true,
            hevc: false,
        };
        let c = windows_capabilities(&old);
        assert!(c.recording && c.window_capture && !c.capture_exclusion && !c.hevc);
        assert!(!old.borderless());
        let w11 = Probe {
            build: 22_631,
            capture: true,
            hevc: true,
        };
        let c = windows_capabilities(&w11);
        assert!(c.capture_exclusion && c.hevc && w11.borderless());
        assert!(!c.app_capture && !c.prores && !c.mov_container && !c.camera_track);
        assert_eq!(c.preview, PreviewKind::Fallback);
        assert_eq!(c.os, PlatformOs::Windows);
    }

    #[test]
    fn no_capture_api_means_no_recording() {
        let c = windows_capabilities(&Probe::default());
        assert!(!c.recording && !c.display_capture);
    }

    #[test]
    fn consent_store_values() {
        assert_eq!(consent_state(None, None), PermissionState::Granted);
        assert_eq!(
            consent_state(Some("Allow"), Some("Allow")),
            PermissionState::Granted
        );
        assert_eq!(consent_state(Some("Deny"), None), PermissionState::Denied);
        assert_eq!(
            consent_state(Some("Allow"), Some("deny")),
            PermissionState::Denied
        );
    }

    #[test]
    fn stream_key_accounts_match_the_mac() {
        let yt = StreamDest {
            service: StreamService::Youtube,
            ..StreamDest::default()
        };
        assert_eq!(dest_url(&yt), "rtmps://a.rtmps.youtube.com:443/live2");
        let custom = StreamDest {
            service: StreamService::Custom,
            url: Some(" rtmp://host/app ".into()),
            ..StreamDest::default()
        };
        assert_eq!(dest_url(&custom), "rtmp://host/app");
        assert_eq!(
            credential_target("rtmp://host/app"),
            "Cortex stream key:rtmp://host/app"
        );
    }
}
