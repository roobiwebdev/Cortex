//! SOURCE OF TRUTH KEYWORDS: stub engine, Windows, Linux, not available yet, UnsupportedEngine, cross-compile
//! WHAT:  The Windows and Linux side of `platform` until their real engines exist: the app
//!        starts, every window works, no devices are listed and every capture, trim or
//!        system call returns a clear "not available yet" error.
//! WHY:   The shell must compile for every target so platform work can start from a building
//!        app, and the UI already learns from `capabilities` (all off) what to hide.
//! WHERE: Selected by `platform/mod.rs` when `target_os` is not macOS.
//!        `docs/engineering/platforms.md` describes the engines that replace it.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use rec_core::api::{EngineError, EngineEvent, EventSink, RecorderEngine};
use rec_core::platform::{
    EngineOptions, ExportOptions, MediaError, MediaInfo, MediaLibrary, PlatformCapabilities,
    PlatformOs,
};
use rec_core::settings::{RecorderSettings, StreamDest};
use rec_core::state::{Devices, EngineState, Permission, PermissionState, Permissions};
use tauri::WebviewWindow;

use super::{
    DesktopEngine, FloatingKind, Hotkey, Hotkeys, PreviewSurface, SystemIntegration, WindowChrome,
};

const OS: PlatformOs = PlatformOs::current();

fn os_name() -> &'static str {
    match OS {
        PlatformOs::Windows => "Windows",
        PlatformOs::Linux => "Linux",
        PlatformOs::Macos => "this system",
    }
}

fn not_yet(what: &str) -> String {
    format!("{what} on {} is not available yet", os_name())
}

fn unsupported(what: &str) -> EngineError {
    EngineError::Unsupported {
        message: not_yet(what),
    }
}

pub fn open_engine(
    opts: EngineOptions,
    settings: RecorderSettings,
    events: EventSink,
) -> Box<dyn DesktopEngine> {
    let _ = (opts, settings);
    Box::new(UnsupportedEngine { events })
}

/// Records nothing; says so.
pub struct UnsupportedEngine {
    events: EventSink,
}

impl UnsupportedEngine {
    fn refuse(&self) -> Result<(), EngineError> {
        let e = unsupported("Recording");
        (self.events)(EngineEvent::Error {
            message: e.to_string(),
            recoverable: true,
        });
        Err(e)
    }
}

impl RecorderEngine for UnsupportedEngine {
    fn state(&self) -> EngineState {
        EngineState {
            capabilities: self.capabilities(),
            permissions: Permissions {
                screen: PermissionState::Denied,
                microphone: PermissionState::Denied,
                camera: PermissionState::Denied,
            },
            ..EngineState::default()
        }
    }
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::unavailable(OS)
    }
    fn start(&self, _settings: &RecorderSettings) -> Result<(), EngineError> {
        self.refuse()
    }
    fn cancel_countdown(&self) -> Result<(), EngineError> {
        Err(unsupported("Recording"))
    }
    fn stop(&self) -> Result<(), EngineError> {
        Err(unsupported("Recording"))
    }
    fn pause(&self) -> Result<(), EngineError> {
        Err(unsupported("Recording"))
    }
    fn resume(&self) -> Result<(), EngineError> {
        Err(unsupported("Recording"))
    }
    fn discard(&self) -> Result<(), EngineError> {
        Err(unsupported("Recording"))
    }
    fn update_settings(&self, _settings: &RecorderSettings) -> Result<(), EngineError> {
        // Settings are still stored by the shell; nothing to apply.
        Ok(())
    }
    fn set_preview_enabled(&self, enabled: bool) -> Result<(), EngineError> {
        if enabled {
            Err(unsupported("The live preview"))
        } else {
            Ok(())
        }
    }
    fn save_replay(&self) -> Result<(), EngineError> {
        Err(unsupported("The replay buffer"))
    }
    fn go_live(&self) -> Result<(), EngineError> {
        Err(unsupported("Going live"))
    }
    fn stop_live(&self) -> Result<(), EngineError> {
        Ok(())
    }
    fn devices(&self) -> Devices {
        Devices::default()
    }
    fn permission(&self, _p: Permission) -> PermissionState {
        PermissionState::Denied
    }
    fn request_permission(&self, _p: Permission) -> PermissionState {
        PermissionState::Denied
    }
    fn shutdown(&self) {}
    fn set_preview_pixel_size(&self, _width: u32, _height: u32) {}
    fn watch_devices(&self) {}
    fn set_stream_key(&self, _dest: &StreamDest, _key: &str) -> Result<(), EngineError> {
        Err(unsupported("Saving stream keys"))
    }
    fn remove_stream_key(&self, _dest: &StreamDest) {}
    fn with_key_status(&self, settings: RecorderSettings) -> RecorderSettings {
        settings
    }
}

impl DesktopEngine for UnsupportedEngine {
    fn attach_preview(
        &self,
        _window: &WebviewWindow,
        _stats: bool,
    ) -> Result<Box<dyn PreviewSurface>, String> {
        Err(not_yet("The live preview"))
    }
}

/// Plain Tauri windows: no panel levels, no capture exclusion.
pub struct Chrome;

impl WindowChrome for Chrome {
    fn configure_floating(&self, _window: &WebviewWindow, _kind: FloatingKind) -> bool {
        false
    }
    fn set_ignores_mouse_events(&self, window: &WebviewWindow, ignore: bool) {
        let _ = window.set_ignore_cursor_events(ignore);
    }
    fn set_capturable(&self, _window: &WebviewWindow) {}
    fn set_on_all_spaces(&self, window: &WebviewWindow) {
        let _ = window.set_visible_on_all_workspaces(true);
    }
    fn invalidate_shadow(&self, _window: &WebviewWindow) {}
}

pub struct Keys;

impl Hotkeys for Keys {
    fn register(&self, _id: u32, _hotkey: Hotkey, _f: Box<dyn Fn()>) -> Result<(), String> {
        Err(not_yet("Global shortcuts"))
    }
    fn unregister(&self, _id: u32) {}
}

pub struct System;

impl SystemIntegration for System {
    fn reveal(&self, _file: &Path) -> Result<(), EngineError> {
        Err(unsupported("Showing a recording in the file manager"))
    }
    fn open_folder(&self, _dir: &Path) -> Result<(), EngineError> {
        Err(unsupported("Opening the recordings folder"))
    }
    fn open_privacy_settings(&self, _permission: Permission) -> Result<(), EngineError> {
        Err(unsupported("Opening the privacy settings"))
    }
    fn app_icon(&self, _app_id: &str) -> Option<String> {
        None
    }
    fn screen_capture_granted(&self) -> bool {
        false
    }
    fn on_terminate(&self, _f: Box<dyn Fn() + Send>) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            not_yet("Quitting cleanly on a termination signal"),
        ))
    }
}

pub struct Media;

impl MediaLibrary for Media {
    fn inspect(&self, _path: &Path) -> Result<MediaInfo, MediaError> {
        Err(MediaError::Failed(not_yet("Reading recordings")))
    }
    fn thumbnails(
        &self,
        _path: &Path,
        _count: u32,
        _max_width: f64,
    ) -> Result<Vec<Vec<u8>>, MediaError> {
        Err(MediaError::Failed(not_yet("Timeline thumbnails")))
    }
    fn waveform(&self, _path: &Path, _buckets: u32) -> Result<Vec<f32>, MediaError> {
        Err(MediaError::Failed(not_yet("The waveform")))
    }
    fn export(
        &self,
        _options: &ExportOptions,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(f64),
    ) -> Result<(), MediaError> {
        Err(MediaError::Failed(not_yet("Quick trim")))
    }
    fn trash(&self, _path: &Path) -> Result<(), MediaError> {
        Err(MediaError::Failed(not_yet("Moving files to the trash")))
    }
}
