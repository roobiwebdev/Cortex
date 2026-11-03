//! SOURCE OF TRUTH KEYWORDS: Windows platform, WindowsEngine adapter, WDA_EXCLUDEFROMCAPTURE, RegisterHotKey, explorer /select, ms-settings privacy, Credential Manager
//! WHAT:  The Windows side of `platform`: the engine is `rec_engine_windows::WindowsEngine`
//!        (Windows.Graphics.Capture, Media Foundation, WASAPI); window behaviour, hotkeys
//!        and settings pages go through `rec_engine_windows::win::shell`; the file manager
//!        is Explorer.
//! WHY:   Same traits as `macos.rs`, so the rest of the shell is unchanged. The native
//!        calls (and their `unsafe`) stay in the engine crate; this file only forwards.
//!        There is no native preview yet (`capabilities.preview` is `fallback` and the page
//!        shows a placeholder), so `attach_preview` only hides the main window from
//!        captures and reports that the view is missing.
//! WHERE: Selected by `platform/mod.rs` on `target_os = "windows"`.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use rec_core::api::{EngineError, EventSink};
use rec_core::platform::EngineOptions;
use rec_core::settings::RecorderSettings;
use rec_core::state::Permission;
use rec_engine_windows::WindowsEngine;
use rec_engine_windows::win::{WinBackend, privacy_uri, shell};
use tauri::WebviewWindow;

use super::{
    DesktopEngine, FloatingKind, Hotkey, HotkeyKey, Hotkeys, PreviewSurface, SystemIntegration,
    WindowChrome,
};

/// The raw handle of a Tauri window.
fn raw(window: &WebviewWindow) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

pub fn open_engine(
    opts: EngineOptions,
    settings: RecorderSettings,
    events: EventSink,
) -> Box<dyn DesktopEngine> {
    // Fragmented MP4 survives a crash, but Media Foundation's fragmented sink is not
    // verified with three audio tracks yet: opt in while testing.
    let fragmented = std::env::var("CORTEX_DEBUG_FRAGMENTED_MP4").is_ok_and(|v| v == "1");
    Box::new(WindowsEngine::new(
        WinBackend::new(fragmented),
        opts,
        settings,
        events,
    ))
}

impl DesktopEngine for WindowsEngine {
    fn attach_preview(
        &self,
        window: &WebviewWindow,
        _stats: bool,
    ) -> Result<Box<dyn PreviewSurface>, String> {
        // Runs once at setup on the main thread: the main window stays out of recordings
        // (the engine also hides every Cortex window when a recording starts).
        if let Some(h) = raw(window)
            && let Err(e) = shell::exclude_from_capture(h)
        {
            eprintln!("cortex: {e}");
        }
        Err("there is no native preview on Windows yet".into())
    }
}

/// Always-on-top, non-activating panels kept out of captures.
pub struct Chrome;

impl WindowChrome for Chrome {
    fn configure_floating(&self, window: &WebviewWindow, _kind: FloatingKind) -> bool {
        let Some(h) = raw(window) else {
            return false;
        };
        let _ = window.set_skip_taskbar(true);
        let _ = window.set_always_on_top(true);
        let floating = shell::make_floating(h);
        let hidden = shell::exclude_from_capture(h);
        for e in [&floating, &hidden]
            .into_iter()
            .filter_map(|r| r.as_ref().err())
        {
            eprintln!("cortex: {e}");
        }
        floating.is_ok() && hidden.is_ok()
    }
    fn set_ignores_mouse_events(&self, window: &WebviewWindow, ignore: bool) {
        let _ = window.set_ignore_cursor_events(ignore);
    }
    fn set_capturable(&self, window: &WebviewWindow) {
        if let Some(h) = raw(window) {
            shell::allow_capture(h);
        }
    }
    fn set_on_all_spaces(&self, window: &WebviewWindow) {
        // Windows virtual desktops: Tauri's best effort.
        let _ = window.set_visible_on_all_workspaces(true);
    }
    fn invalidate_shadow(&self, _window: &WebviewWindow) {}
}

/// `RegisterHotKey` on a message-only window: Ctrl+Shift where macOS uses ⇧⌘.
pub struct Keys;

impl Hotkeys for Keys {
    fn register(&self, id: u32, hotkey: Hotkey, f: Box<dyn Fn()>) -> Result<(), String> {
        let vk = match hotkey.key {
            HotkeyKey::R => shell::VK_R,
            HotkeyKey::S => shell::VK_S,
            HotkeyKey::Escape => shell::VK_ESCAPE,
        };
        shell::register_hotkey(id, vk, hotkey.primary_shift, f)
    }
    fn unregister(&self, id: u32) {
        shell::unregister_hotkey(id);
    }
}

/// File Explorer, Settings › Privacy and the console's Ctrl+C.
pub struct System;

/// Run Explorer. It exits with 1 even when it worked, so only a failure to start counts.
fn explorer(args: &str) -> Result<(), EngineError> {
    Command::new("explorer.exe")
        .raw_arg(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| EngineError::failed(format!("Could not open File Explorer: {e}")))
}

impl SystemIntegration for System {
    fn reveal(&self, file: &Path) -> Result<(), EngineError> {
        explorer(&format!("/select,\"{}\"", file.display()))
    }
    fn open_folder(&self, dir: &Path) -> Result<(), EngineError> {
        explorer(&format!("\"{}\"", dir.display()))
    }
    fn open_privacy_settings(&self, permission: Permission) -> Result<(), EngineError> {
        shell::open_uri(privacy_uri(permission)).map_err(EngineError::failed)
    }
    fn app_icon(&self, _app_id: &str) -> Option<String> {
        // Not built yet: the app list shows names without icons.
        None
    }
    fn screen_capture_granted(&self) -> bool {
        // Windows has no screen-capture grant.
        true
    }
    fn on_terminate(&self, f: Box<dyn Fn() + Send>) -> std::io::Result<()> {
        shell::on_terminate(f)
    }
}

/// Media Foundation `inspect` and the Recycle Bin; trim export is not built yet.
pub use rec_engine_windows::win::media::WinMedia as Media;
