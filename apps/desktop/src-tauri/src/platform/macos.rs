//! SOURCE OF TRUTH KEYWORDS: macOS platform, MacEngine adapter, PreviewView, Carbon hotkeys, open -R, TCC, AVFoundation media
//! WHAT:  The macOS side of `platform`: each trait delegates to the code that already did
//!        the work (`rec_engine` for capture, TCC, hotkeys, icons and AVFoundation trim;
//!        `rec_macos_ui` for the preview view, panels and signals; `/usr/bin/open`).
//! WHY:   Moving the calls behind traits must not change what the Mac app does, so this file
//!        only forwards: same calls, same order, same messages.
//! WHERE: Selected by `platform/mod.rs` on `target_os = "macos"`.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use rec_core::api::{EngineError, EventSink};
use rec_core::platform::{EngineOptions, ExportOptions, MediaError, MediaInfo, MediaLibrary};
use rec_core::settings::RecorderSettings;
use rec_core::state::Permission;
use rec_engine::engine::MacEngine;
use rec_engine::{hotkey, screen, tcc, trim};
use rec_macos_ui::{PreviewFn, PreviewView, make_window_webviews_transparent};
use tauri::WebviewWindow;

use super::{
    DesktopEngine, FloatingKind, Hotkey, HotkeyKey, Hotkeys, PreviewSurface, SystemIntegration,
    WindowChrome,
};

/// kVK_Escape.
const KEY_ESCAPE: u32 = 53;

pub fn open_engine(
    opts: EngineOptions,
    settings: RecorderSettings,
    events: EventSink,
) -> Box<dyn DesktopEngine> {
    if opts.require_self_exclusion {
        // Our windows must never show up in a screen recording: fail instead.
        screen::require_self_exclusion(true);
    }
    let opts = rec_engine::engine::EngineOptions {
        home: opts.home,
        test_pattern: opts.test_pattern,
        max_duration_secs: opts.max_duration_secs,
        withheld: opts.withheld,
    };
    Box::new(MacEngine::new(opts, settings, events))
}

impl DesktopEngine for MacEngine {
    fn attach_preview(
        &self,
        window: &WebviewWindow,
        stats: bool,
    ) -> Result<Box<dyn PreviewSurface>, String> {
        let view = PreviewView::attach_to(window).map_err(|e| e.to_string())?;
        if make_window_webviews_transparent(window) == 0 {
            eprintln!("cortex: could not make the main webview transparent");
        }
        let sink = view.sink();
        let sink = if stats { counted(sink) } else { sink };
        self.set_preview_sink(Some(sink));
        Ok(Box::new(view))
    }
}

impl PreviewSurface for PreviewView {
    fn set_frame(&self, x: f64, y: f64, w: f64, h: f64) {
        PreviewView::set_frame(self, x, y, w, h);
    }
    fn set_corner_radius(&self, radius: f64) {
        PreviewView::set_corner_radius(self, radius);
    }
    fn show(&self) {
        PreviewView::show(self);
    }
    fn hide(&self) {
        PreviewView::hide(self);
    }
    fn clear(&self) {
        PreviewView::clear(self);
    }
    fn on_window_visibility(&self, f: Box<dyn Fn(bool) + Send>) {
        PreviewView::on_window_visibility(self, f);
    }
}

/// Wraps a sink to print how many frames a second reach the view, and their size,
/// every 5 s. A ticker thread prints, so a stalled preview shows up as 0 fps.
fn counted(sink: PreviewFn) -> PreviewFn {
    let count = Arc::new(Mutex::new((0u32, 0usize, 0usize)));
    let c = count.clone();
    let _ = std::thread::Builder::new()
        .name("preview-stats".into())
        .spawn(move || {
            let mut t = std::time::Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let Ok(mut g) = c.lock() else { return };
                let secs = t.elapsed().as_secs_f64();
                let (n, w, h) = *g;
                println!("cortex: preview {:.1} fps {w}x{h}", f64::from(n) / secs);
                g.0 = 0;
                t = std::time::Instant::now();
            }
        });
    Arc::new(move |pb| {
        let (w, h) = rec_macos_ui::frame_size(&pb);
        if let Ok(mut g) = count.lock() {
            *g = (g.0 + 1, w, h);
        }
        sink(pb)
    })
}

/// AppKit panels through `rec_macos_ui`.
pub struct Chrome;

impl WindowChrome for Chrome {
    fn configure_floating(&self, window: &WebviewWindow, kind: FloatingKind) -> bool {
        let kind = match kind {
            FloatingKind::MenubarPanel => rec_macos_ui::FloatingKind::MenubarPanel,
            FloatingKind::Hud => rec_macos_ui::FloatingKind::Hud,
            FloatingKind::Toast => rec_macos_ui::FloatingKind::Toast,
            FloatingKind::Countdown => rec_macos_ui::FloatingKind::Countdown,
            FloatingKind::RegionPicker => rec_macos_ui::FloatingKind::RegionPicker,
        };
        rec_macos_ui::configure_floating_window(window, kind).is_some()
    }
    fn set_ignores_mouse_events(&self, window: &WebviewWindow, ignore: bool) {
        rec_macos_ui::set_window_ignores_mouse_events(window, ignore);
    }
    fn set_capturable(&self, window: &WebviewWindow) {
        rec_macos_ui::panel::set_window_capturable(window);
    }
    fn set_on_all_spaces(&self, window: &WebviewWindow) {
        rec_macos_ui::set_window_on_all_spaces(window);
    }
    fn invalidate_shadow(&self, window: &WebviewWindow) {
        rec_macos_ui::invalidate_window_shadow(window);
    }
}

/// Carbon hot keys (`rec_engine::hotkey`), which need no Accessibility permission.
pub struct Keys;

impl Hotkeys for Keys {
    fn register(&self, id: u32, hotkey: Hotkey, f: Box<dyn Fn()>) -> Result<(), String> {
        let key = match hotkey.key {
            HotkeyKey::R => hotkey::KEY_R,
            HotkeyKey::S => hotkey::KEY_S,
            HotkeyKey::Escape => KEY_ESCAPE,
        };
        let modifiers = if hotkey.primary_shift {
            hotkey::CMD | hotkey::SHIFT
        } else {
            0
        };
        hotkey::register(id, key, modifiers, f).map_err(|code| format!("Carbon error {code}"))
    }
    fn unregister(&self, id: u32) {
        hotkey::unregister(id);
    }
}

/// Finder, System Settings, TCC and signals.
pub struct System;

impl SystemIntegration for System {
    fn reveal(&self, file: &Path) -> Result<(), EngineError> {
        open(&["-R".as_ref(), file.as_os_str()])
    }
    fn open_folder(&self, dir: &Path) -> Result<(), EngineError> {
        open(&[dir.as_os_str()])
    }
    fn open_privacy_settings(&self, permission: Permission) -> Result<(), EngineError> {
        open(&[privacy_pane_url(permission).as_ref()])
    }
    fn app_icon(&self, app_id: &str) -> Option<String> {
        screen::app_icon(app_id)
    }
    fn screen_capture_granted(&self) -> bool {
        tcc::screen_status() == tcc::Status::Granted
    }
    fn on_terminate(&self, f: Box<dyn Fn() + Send>) -> std::io::Result<()> {
        rec_macos_ui::signals::on_terminate(f)
    }
}

fn open(args: &[&std::ffi::OsStr]) -> Result<(), EngineError> {
    let status = Command::new("/usr/bin/open")
        .args(args)
        .status()
        .map_err(|e| EngineError::failed(format!("Could not open Finder: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(EngineError::failed("Could not open Finder"))
    }
}

/// The System Settings privacy pane for a permission.
fn privacy_pane_url(p: Permission) -> &'static str {
    match p {
        Permission::Screen => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        Permission::Microphone => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        Permission::Camera => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera"
        }
    }
}

/// AVFoundation (`rec_engine::trim`). Messages keep the formatting the shell showed before:
/// `{e}` for reads, `{e:#}` (with causes) for export and trash.
pub struct Media;

fn read_err(e: impl std::fmt::Display) -> MediaError {
    MediaError::Failed(e.to_string())
}

fn write_err(e: impl std::fmt::Display) -> MediaError {
    if e.to_string() == "cancelled" {
        MediaError::Cancelled
    } else {
        MediaError::Failed(format!("{e:#}"))
    }
}

impl MediaLibrary for Media {
    fn inspect(&self, path: &Path) -> Result<MediaInfo, MediaError> {
        trim::inspect(path).map_err(read_err)
    }
    fn thumbnails(
        &self,
        path: &Path,
        count: u32,
        max_width: f64,
    ) -> Result<Vec<Vec<u8>>, MediaError> {
        trim::thumbnails(path, count, max_width).map_err(read_err)
    }
    fn waveform(&self, path: &Path, buckets: u32) -> Result<Vec<f32>, MediaError> {
        trim::waveform(path, buckets).map_err(read_err)
    }
    fn export(
        &self,
        options: &ExportOptions,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(f64),
    ) -> Result<(), MediaError> {
        trim::export(options, cancel, progress).map_err(write_err)
    }
    fn trash(&self, path: &Path) -> Result<(), MediaError> {
        trim::trash(path).map_err(write_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_panes_open_the_right_settings() {
        assert!(privacy_pane_url(Permission::Screen).ends_with("Privacy_ScreenCapture"));
        assert!(privacy_pane_url(Permission::Microphone).ends_with("Privacy_Microphone"));
        assert!(privacy_pane_url(Permission::Camera).ends_with("Privacy_Camera"));
    }

    #[test]
    fn a_cancelled_export_is_told_apart() {
        assert_eq!(write_err("cancelled"), MediaError::Cancelled);
        assert_eq!(
            write_err("disk full"),
            MediaError::Failed("disk full".into())
        );
    }
}
