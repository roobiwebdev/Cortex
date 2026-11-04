//! SOURCE OF TRUTH KEYWORDS: tour, onboarding, first run, onboardingCompleted, Show the Tour, hotkey practice, platform
//! WHAT:  The first-run tour's shell side: open it on first launch, count closing it as done,
//!        "Help › Show the Tour" in the app menu, the `platform` command, and hotkey practice
//!        (⇧⌘R reaches the tour instead of starting a recording while its shortcut step shows).
//! WHY:   The tour is the onboarding window (`AppWindow::Onboarding`). Whether it has run lives
//!        in `RecorderSettings::onboarding_completed`, so it survives relaunches (Screen
//!        Recording may need one) and is shared by every window.
//! WHERE: `lib.rs` (menu, setup), `windows::ensure` (window events), `floating::toggle_record`
//!        (hotkey), `ipc.rs` (commands).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rec_core::platform::PlatformOs;
use rec_core::state::EngineStatus;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::menu::{HELP_SUBMENU_ID, Menu, MenuEvent, MenuItem, MenuItemKind, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Runtime, WebviewWindow, WindowEvent};
use tauri_specta::Event;

use crate::shell::Shell;
use crate::windows::{self, AppWindow, ONBOARDING};

/// Menu item id for "Help › Show the Tour".
pub(crate) const SHOW_TOUR: &str = "show-tour";

/// A global shortcut the tour can catch while it is practising.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum TourHotkey {
    /// ⇧⌘R (start or stop recording).
    Record,
}

/// The user pressed a global shortcut while the tour was practising it.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HotkeyPressedEvent {
    pub hotkey: TourHotkey,
}
impl tauri_specta::Event for HotkeyPressedEvent {
    const NAME: &'static str = "tour://hotkey-pressed";
}

/// Whether the tour's shortcut step is on screen and wants ⇧⌘R for itself.
#[derive(Default)]
pub(crate) struct Practice(AtomicBool);

/// The platform this build runs on, for the screens that differ per platform.
#[tauri::command]
#[specta::specta]
pub fn platform() -> PlatformOs {
    PlatformOs::current()
}

/// Onboarding window only: while on, ⇧⌘R is sent to the tour (`tour://hotkey-pressed`)
/// instead of starting a recording. Turned off when the step changes or the window goes.
#[tauri::command]
#[specta::specta]
pub fn set_hotkey_practice(app: AppHandle, on: bool) {
    if let Some(p) = app.try_state::<Practice>() {
        p.0.store(on, Ordering::SeqCst);
    }
}

/// Called by the ⇧⌘R handler on the main thread. True when the tour took the press: it
/// is practising, nothing is recording, and its window is on screen.
pub(crate) fn catch_record_hotkey(app: &AppHandle, status: EngineStatus) -> bool {
    let practising = app
        .try_state::<Practice>()
        .is_some_and(|p| p.0.load(Ordering::SeqCst));
    if !practising || status != EngineStatus::Idle {
        return false;
    }
    let shown = app
        .get_webview_window(ONBOARDING)
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    if !shown {
        return false;
    }
    let event = HotkeyPressedEvent {
        hotkey: TourHotkey::Record,
    };
    if let Err(e) = event.emit_to(app, ONBOARDING) {
        log_line!("could not tell the tour about the shortcut: {e}");
        return false;
    }
    true
}

pub(crate) fn setup(app: &tauri::App) {
    app.manage(Practice::default());
}

/// Opens the tour at launch until it has been finished, skipped or closed once.
pub(crate) fn open_on_first_run(app: &AppHandle, completed: bool) {
    if completed {
        return;
    }
    if let Err(e) = windows::open_window(app.clone(), AppWindow::Onboarding) {
        log_line!("could not open the tour: {e}");
    }
}

/// Window events of the onboarding window (wired by `windows::ensure` when it is built).
/// Closing it counts as done, and practice never outlives the window.
pub(crate) fn on_window_created(app: &AppHandle, win: &WebviewWindow) {
    let h = app.clone();
    win.on_window_event(move |e| match e {
        WindowEvent::CloseRequested { .. } => {
            set_hotkey_practice(h.clone(), false);
            mark_completed(&h);
        }
        WindowEvent::Destroyed => set_hotkey_practice(h.clone(), false),
        _ => {}
    });
}

/// Stores `onboardingCompleted = true`, off the main thread (the settings store may block).
fn mark_completed(app: &AppHandle) {
    let Some(shell) = app.try_state::<Arc<Shell>>().map(|s| s.inner().clone()) else {
        return;
    };
    std::thread::spawn(move || {
        let s = shell.settings();
        if s.onboarding_completed {
            return;
        }
        let next = rec_core::RecorderSettings {
            onboarding_completed: true,
            ..s
        };
        if let Err(e) = shell.update_settings(next) {
            log_line!("could not store that the tour ran: {e}");
        }
    });
}

/**
 * SOURCE OF TRUTH KEYWORDS: app menu, Help menu, Show the Tour, default menu, menu bar
 * WHAT:  Tauri's default app menu with "Show the Tour" added under Help.
 * WHY:   The tour can be reopened at any time, where people look for help. The default
 *        menu keeps the standard Edit (copy, paste) and Window items working.
 * WHERE: `lib.rs`, `Builder::menu`.
 */
pub(crate) fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    if let Some(MenuItemKind::Submenu(help)) = menu.get(HELP_SUBMENU_ID) {
        let item = MenuItem::with_id(app, SHOW_TOUR, "Show the Tour", true, None::<&str>)?;
        // On Windows and Linux the Help menu already holds About; keep the tour first.
        if help.items()?.is_empty() {
            help.append(&item)?;
        } else {
            help.prepend(&PredefinedMenuItem::separator(app)?)?;
            help.prepend(&item)?;
        }
    }
    Ok(menu)
}

/// App menu events: "Show the Tour" opens the onboarding window.
pub(crate) fn on_menu_event(app: &AppHandle, event: &MenuEvent) {
    if event.id().as_ref() == SHOW_TOUR
        && let Err(e) = windows::open_window(app.clone(), AppWindow::Onboarding)
    {
        log_line!("could not open the tour: {e}");
    }
}
