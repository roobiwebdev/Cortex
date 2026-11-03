//! SOURCE OF TRUTH KEYWORDS: platform, DesktopEngine, PreviewSurface, WindowChrome, Hotkeys, SystemIntegration, open_engine, cfg target_os
//! WHAT:  The shell's door to the operating system. Everything the shell needs from the
//!        platform goes through these traits; `#[cfg(target_os)]` picks the implementation
//!        once, here: `macos.rs` delegates to `rec-engine-macos` and `rec-macos-ui`,
//!        `windows.rs` to `rec-engine-windows`, and `unsupported.rs` is the Linux stub
//!        (compiles, records nothing).
//! WHY:   The rest of the shell (shell.rs, ipc.rs, preview.rs, floating.rs, trim.rs, quit.rs,
//!        lib.rs) has no `rec_engine::` or `rec_macos_ui::` calls, so a Windows or Linux
//!        engine is added here without touching them. The engine contract itself
//!        (`RecorderEngine`, `MediaLibrary`, `Secrets`, `PlatformCapabilities`) is in
//!        `rec_core` because it needs no Tauri; the traits below need a Tauri window or the
//!        app process, so they live with the shell.
//! WHERE: Every shell module; `docs/engineering/platforms.md` has the full inventory.

use std::path::Path;

use rec_core::api::{EngineError, EventSink, RecorderEngine};
use rec_core::platform::{EngineOptions, MediaLibrary};
use rec_core::settings::RecorderSettings;
use rec_core::state::Permission;
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use unsupported as imp;

/// The recorder engine as the desktop shell drives it: the platform-neutral API plus the
/// native preview, which needs the main window.
pub trait DesktopEngine: RecorderEngine {
    /// Puts the native preview under the main window's (now transparent) webview and
    /// routes the engine's preview frames to it. `stats`: print the frame rate every 5 s.
    fn attach_preview(
        &self,
        window: &WebviewWindow,
        stats: bool,
    ) -> Result<Box<dyn PreviewSurface>, String>;
}

/// The native view that shows the live preview inside the main window.
pub trait PreviewSurface: Send + Sync {
    /// Place it over the page's hole: CSS px from the webview's top-left.
    fn set_frame(&self, x: f64, y: f64, w: f64, h: f64);
    fn set_corner_radius(&self, radius: f64);
    fn show(&self);
    fn hide(&self);
    /// Drop the last frame so a stale picture never flashes when it shows again.
    fn clear(&self);
    /// `f(visible)` when the window becomes visible or hidden (occlusion, minimise, Space).
    fn on_window_visibility(&self, f: Box<dyn Fn(bool) + Send>);
}

/// What a floating window is for; sets its level and behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatingKind {
    MenubarPanel,
    Hud,
    Toast,
    Countdown,
    RegionPicker,
}

/// Native window behaviour Tauri does not offer: floating panels that never take focus,
/// stay on every Space and out of captures.
pub trait WindowChrome: Sync {
    /// Level, all Spaces, non-activating, excluded from screen capture. False if it failed.
    fn configure_floating(&self, window: &WebviewWindow, kind: FloatingKind) -> bool;
    fn set_ignores_mouse_events(&self, window: &WebviewWindow, ignore: bool);
    /// Debug runs: let screenshots see a window excluded from capture.
    fn set_capturable(&self, window: &WebviewWindow);
    /// Follow the user to every Space (full-screen ones included).
    fn set_on_all_spaces(&self, window: &WebviewWindow);
    /// Recompute the shadow after a resize (transparent windows keep the old one).
    fn invalidate_shadow(&self, window: &WebviewWindow);
}

/// A key a global shortcut can use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyKey {
    R,
    S,
    Escape,
}

/// A global shortcut: the key, with the platform's primary modifier and Shift (⇧⌘ on macOS,
/// Ctrl+Shift elsewhere) or bare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub key: HotkeyKey,
    pub primary_shift: bool,
}

/// System-wide shortcuts that work while another app is in front.
pub trait Hotkeys: Sync {
    /// Register `id`, replacing any earlier one. `f` runs on the main thread. The error says
    /// why (usually another app holds the shortcut).
    fn register(&self, id: u32, hotkey: Hotkey, f: Box<dyn Fn()>) -> Result<(), String>;
    fn unregister(&self, id: u32);
}

/// The file manager, the privacy settings, app icons and process-level hooks.
pub trait SystemIntegration: Sync {
    /// Show a file selected in Finder / File Explorer / the file manager.
    fn reveal(&self, file: &Path) -> Result<(), EngineError>;
    /// Open a folder in the file manager.
    fn open_folder(&self, dir: &Path) -> Result<(), EngineError>;
    /// Open the system settings page where a permission is granted.
    fn open_privacy_settings(&self, permission: Permission) -> Result<(), EngineError>;
    /// A running app's icon as a small PNG `data:` URL, if it has one.
    fn app_icon(&self, app_id: &str) -> Option<String>;
    /// Screen capture is granted to this process now (the relaunch check's child process).
    fn screen_capture_granted(&self) -> bool;
    /// Call `f` on SIGTERM/SIGINT (or the platform's equivalent) instead of dying mid-write.
    fn on_terminate(&self, f: Box<dyn Fn() + Send>) -> std::io::Result<()>;
}

/// Start this platform's engine with `settings`; events go to `events`.
pub fn open_engine(
    opts: EngineOptions,
    settings: RecorderSettings,
    events: EventSink,
) -> Box<dyn DesktopEngine> {
    imp::open_engine(opts, settings, events)
}

pub fn chrome() -> &'static dyn WindowChrome {
    &imp::Chrome
}

pub fn hotkeys() -> &'static dyn Hotkeys {
    &imp::Keys
}

pub fn system() -> &'static dyn SystemIntegration {
    &imp::System
}

pub fn media() -> &'static dyn MediaLibrary {
    &imp::Media
}
