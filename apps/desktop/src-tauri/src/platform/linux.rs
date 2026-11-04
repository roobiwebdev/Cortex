//! SOURCE OF TRUTH KEYWORDS: Linux platform, LinuxEngine adapter, GlobalShortcuts portal, FileManager1, xdg-open, Wayland floating windows, SIGTERM
//! WHAT:  The Linux side of `platform`: the engine is `rec_engine_linux::LinuxEngine`
//!        (ScreenCast portal + PipeWire + GStreamer); the file manager is reached over D-Bus
//!        (`FileManager1.ShowItems`, else `xdg-open`); global shortcuts go through the
//!        GlobalShortcuts portal; floating windows get what GTK offers.
//! WHY:   Same traits as `macos.rs`, so the rest of the shell is unchanged. D-Bus, portals
//!        and GStreamer stay in the engine crate; this file forwards and moves shortcut
//!        activations onto the GTK main thread, where the shell's callbacks must run.
//!        Wayland limits: no capture exclusion, clients cannot place their own windows, and
//!        always-on-top is a hint the compositor may ignore. `capabilities` says so.
//! WHERE: Selected by `platform/mod.rs` on `target_os = "linux"`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use rec_core::api::{EngineError, EventSink};
use rec_core::platform::EngineOptions;
use rec_core::settings::RecorderSettings;
use rec_core::state::Permission;
use rec_engine_linux::LinuxEngine;
use rec_engine_linux::desktop::{self, ShortcutSpec};
use tauri::WebviewWindow;

pub use rec_engine_linux::GstMedia as Media;

use super::{
    DesktopEngine, FloatingKind, Hotkey, HotkeyKey, Hotkeys, PreviewSurface, SystemIntegration,
    WindowChrome,
};

pub fn open_engine(
    opts: EngineOptions,
    settings: RecorderSettings,
    events: EventSink,
) -> Box<dyn DesktopEngine> {
    Box::new(LinuxEngine::new(opts, settings, events))
}

impl DesktopEngine for LinuxEngine {
    fn attach_preview(
        &self,
        _window: &WebviewWindow,
        _stats: bool,
    ) -> Result<Box<dyn PreviewSurface>, String> {
        // `capabilities.preview` is `none`: the page shows its placeholder instead.
        Err("there is no live preview on Linux yet".into())
    }
}

/// GTK hints: on top, on every workspace, out of the taskbar. Under Wayland the compositor
/// decides; nothing keeps these windows out of a portal capture.
pub struct Chrome;

impl WindowChrome for Chrome {
    fn configure_floating(&self, window: &WebviewWindow, kind: FloatingKind) -> bool {
        let top = window.set_always_on_top(true).is_ok();
        let _ = window.set_skip_taskbar(true);
        if matches!(
            kind,
            FloatingKind::Hud | FloatingKind::Toast | FloatingKind::Countdown
        ) {
            let _ = window.set_visible_on_all_workspaces(true);
        }
        top
    }
    fn set_ignores_mouse_events(&self, window: &WebviewWindow, ignore: bool) {
        // tao sets the input shape on the GDK window, which only exists once the GTK window
        // is realized; floating windows are configured before they are first shown.
        if let Ok(gtk_window) = window.gtk_window() {
            use gtk::prelude::WidgetExt;
            gtk_window.realize();
        }
        let _ = window.set_ignore_cursor_events(ignore);
    }
    fn set_capturable(&self, _window: &WebviewWindow) {}
    fn set_on_all_spaces(&self, window: &WebviewWindow) {
        let _ = window.set_visible_on_all_workspaces(true);
    }
    fn invalidate_shadow(&self, _window: &WebviewWindow) {}
}

/// The portal shortcut id for a key, and its default trigger (XDG shortcuts syntax).
fn shortcut_for(key: HotkeyKey) -> Option<(&'static str, &'static str, &'static str)> {
    match key {
        HotkeyKey::R => Some(("record", "Start or stop recording", "CTRL+SHIFT+r")),
        HotkeyKey::S => Some(("save-replay", "Save the replay", "CTRL+SHIFT+s")),
        // A global Esc would steal the key from every app; it only works in Cortex.
        HotkeyKey::Escape => None,
    }
}

/// The shell's hotkey id and its callback.
type Registered = (u32, Box<dyn Fn()>);

thread_local! {
    /// Registered callbacks by portal shortcut id. Lives on the main thread, where
    /// `register` is called and where activations are delivered.
    static CALLBACKS: RefCell<HashMap<&'static str, Registered>> = RefCell::new(HashMap::new());
}

/// The portal binding, made once for the process (the desktop may show a dialog).
static BINDING: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn bind_once() {
    if BINDING.get().is_some() {
        return;
    }
    let state = BINDING.get_or_init(|| Mutex::new(None));
    let specs: Vec<ShortcutSpec> = [HotkeyKey::R, HotkeyKey::S]
        .into_iter()
        .filter_map(shortcut_for)
        .map(|(id, description, trigger)| ShortcutSpec {
            id: id.into(),
            description: description.into(),
            trigger: trigger.into(),
        })
        .collect();
    let on_activate: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|id: &str| {
        let id = id.to_string();
        glib::MainContext::default().invoke(move || {
            CALLBACKS.with(|c| {
                if let Some((_, f)) = c.borrow().get(id.as_str()) {
                    f();
                }
            });
        });
    });
    // Binding waits for the desktop's dialog; never block the main thread on it.
    std::thread::spawn(move || {
        let result = desktop::bind_global_shortcuts(specs, on_activate);
        match &result {
            Ok(assigned) => eprintln!("cortex: global shortcuts bound: {assigned:?}"),
            Err(e) => eprintln!("cortex: global shortcuts unavailable: {e}"),
        }
        if let Ok(mut g) = state.lock() {
            *g = result.err();
        }
    });
}

/// The GlobalShortcuts portal (KDE Plasma 5.27+, GNOME 48+). Without it, shortcuts only
/// work inside Cortex and `capabilities.globalHotkeys` is false.
pub struct Keys;

impl Hotkeys for Keys {
    fn register(&self, id: u32, hotkey: Hotkey, f: Box<dyn Fn()>) -> Result<(), String> {
        let Some((name, _, _)) = shortcut_for(hotkey.key).filter(|_| hotkey.primary_shift) else {
            return Err("only Ctrl+Shift+R and Ctrl+Shift+S are system-wide on Linux".into());
        };
        if let Some(Some(e)) = BINDING.get().and_then(|m| m.lock().ok().map(|g| g.clone())) {
            return Err(e);
        }
        CALLBACKS.with(|c| c.borrow_mut().insert(name, (id, f)));
        bind_once();
        Ok(())
    }
    fn unregister(&self, id: u32) {
        CALLBACKS.with(|c| c.borrow_mut().retain(|_, (i, _)| *i != id));
    }
}

/// File manager over D-Bus, SIGTERM through GLib.
pub struct System;

impl SystemIntegration for System {
    fn reveal(&self, file: &Path) -> Result<(), EngineError> {
        desktop::reveal(file).map_err(EngineError::failed)
    }
    fn open_folder(&self, dir: &Path) -> Result<(), EngineError> {
        desktop::open_folder(dir).map_err(EngineError::failed)
    }
    fn open_url(&self, url: &str) -> Result<(), EngineError> {
        // Detached: xdg-open can block until the browser exits on some desktops.
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(drop)
            .map_err(|e| EngineError::failed(format!("Could not run xdg-open: {e}")))
    }
    fn open_privacy_settings(&self, _permission: Permission) -> Result<(), EngineError> {
        Err(EngineError::Unsupported {
            message: "Linux asks for the screen when a recording starts; microphones and cameras need no permission".into(),
        })
    }
    fn app_icon(&self, _app_id: &str) -> Option<String> {
        None
    }
    fn screen_capture_granted(&self) -> bool {
        // Nothing to grant ahead of time: the portal asks at use.
        true
    }
    fn on_terminate(&self, f: Box<dyn Fn() + Send>) -> std::io::Result<()> {
        let f = Arc::new(Mutex::new(f));
        // SIGTERM and SIGINT (Linux numbers), delivered on the GTK main loop.
        for signum in [15, 2] {
            let f = f.clone();
            glib::source::unix_signal_add(signum, move || {
                if let Ok(f) = f.lock() {
                    f();
                }
                glib::ControlFlow::Continue
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_record_and_replay_are_global() {
        assert_eq!(
            shortcut_for(HotkeyKey::R).map(|s| s.2),
            Some("CTRL+SHIFT+r")
        );
        assert_eq!(shortcut_for(HotkeyKey::S).map(|s| s.0), Some("save-replay"));
        assert_eq!(shortcut_for(HotkeyKey::Escape), None);
        let esc = Hotkey {
            key: HotkeyKey::Escape,
            primary_shift: false,
        };
        assert!(Keys.register(9, esc, Box::new(|| {})).is_err());
    }

    #[test]
    fn privacy_settings_explain_the_portal() {
        let e = System
            .open_privacy_settings(Permission::Screen)
            .unwrap_err()
            .to_string();
        assert!(e.contains("when a recording starts"));
    }
}
