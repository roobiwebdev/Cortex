//! SOURCE OF TRUTH KEYWORDS: linux_capabilities, PlatformCapabilities, Wayland, X11, portal, system picker, capture exclusion
//! WHAT:  What the Linux engine reports it can do, from what was probed on this PC.
//! WHY:   The UI hides or explains what Linux cannot do instead of failing when it is picked:
//!        the system picker chooses windows and screens (`SourcePicker::System`), app capture
//!        and per-app audio do not exist, nothing keeps our windows out of a portal capture,
//!        and there is no native preview. Pure, so the honest answers are unit tested.
//! WHERE: `engine::LinuxEngine::capabilities`.

use rec_core::platform::{PlatformCapabilities, PlatformOs, PreviewKind, SourcePicker};

/// What was found on this PC at start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    /// GStreamer runs and an H.264 encoder, an AAC encoder and the muxers exist.
    pub can_record: bool,
    pub hevc: bool,
    /// A PulseAudio or PipeWire source element exists (sink monitor capture).
    pub system_audio: bool,
    /// The GlobalShortcuts portal answered.
    pub global_shortcuts: bool,
    /// The recordings list can read files (GStreamer's discoverer and decoders exist).
    pub media: bool,
}

pub fn linux_capabilities(p: &Probe) -> PlatformCapabilities {
    PlatformCapabilities {
        os: PlatformOs::Linux,
        recording: p.can_record,
        display_capture: p.can_record,
        // An area is a crop of a portal-picked monitor.
        region_capture: p.can_record,
        // The portal lists windows in its own picker; Cortex cannot.
        window_capture: p.can_record,
        app_capture: false,
        source_picker: SourcePicker::System,
        system_audio: p.can_record && p.system_audio,
        per_app_audio: false,
        // Wayland has no way to exclude a window from a portal capture.
        capture_exclusion: false,
        global_hotkeys: p.global_shortcuts,
        hevc: p.can_record && p.hevc,
        prores: false,
        // No native view under WebKitGTK and no frame path to the webview yet.
        preview: PreviewKind::None,
        live_streaming: false,
        replay_buffer: false,
        quick_trim: p.media,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_working_pc_reports_the_portal_limits() {
        let c = linux_capabilities(&Probe {
            can_record: true,
            hevc: true,
            system_audio: true,
            global_shortcuts: false,
            media: true,
        });
        assert!(c.recording && c.display_capture && c.window_capture && c.region_capture);
        assert_eq!(c.source_picker, SourcePicker::System);
        assert!(!c.app_capture && !c.per_app_audio && !c.capture_exclusion && !c.prores);
        assert!(!c.global_hotkeys);
        assert_eq!(c.preview, PreviewKind::None);
        assert!(c.hevc && c.system_audio && c.quick_trim);
        assert_eq!(c.os, PlatformOs::Linux);
    }

    #[test]
    fn without_gstreamer_nothing_records() {
        let c = linux_capabilities(&Probe {
            hevc: true,
            system_audio: true,
            ..Probe::default()
        });
        assert!(!c.recording && !c.hevc && !c.system_audio && !c.display_capture);
    }
}
