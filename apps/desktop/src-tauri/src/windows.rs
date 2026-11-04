//! Window and menu bar behaviour.
//!
//! Only the main window exists at launch. Every other window in `tauri.conf.json` is
//! marked `"create": false` and built on first use by [`ensure`], from that same config.
//! A hidden one is destroyed after [`IDLE_CLOSE`] (see [`reap`]), so it has no WebKit
//! process while nobody needs it. This module also wires the menu bar (tray) icon to the
//! "menubar" panel and keeps the main window alive on close.
//!
//! The floating windows' native behaviour (levels, non-activation, capture
//! exclusion), placement and engine-driven visibility live in `floating.rs`.
//! Still to come: one region overlay per display and a native `NSStatusItem`
//! (recording clock, drag target) replacing the tray icon.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use rec_core::api::EngineError;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    App, AppHandle, Manager, PhysicalPosition, Rect, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::floating::{self, COUNTDOWN, HUD, REGION, TOASTS};

pub(crate) const MENUBAR: &str = "menubar";
pub(crate) const MAIN: &str = "main";
pub(crate) const SETTINGS: &str = "settings";
pub(crate) const ONBOARDING: &str = "onboarding";
pub(crate) const TRIM: &str = "trim";

/// Windows built on first use and destroyed once hidden for [`IDLE_CLOSE`].
pub(crate) const LAZY: [&str; 8] = [
    SETTINGS, ONBOARDING, TRIM, MENUBAR, HUD, TOASTS, COUNTDOWN, REGION,
];
/// A lazy window hidden this long is destroyed, so its WebKit process exits.
const IDLE_CLOSE: Duration = Duration::from_secs(30);
/// How often hidden windows are checked.
const REAP_EVERY: Duration = Duration::from_secs(5);
/// Logical width of the menu bar panel; its height follows the content (set from the webview).
const MENUBAR_WIDTH: f64 = 340.0;
/// Gap between the menu bar and the panel, in logical points.
const MENUBAR_GAP: f64 = 6.0;

/// When the panel last hid itself on blur. A tray click right after must not reopen it.
#[derive(Default)]
struct PanelHiddenAt(Mutex<Option<Instant>>);

/// When each lazy window was built, and since when it has been hidden.
#[derive(Default)]
struct Lifecycle(Mutex<HashMap<&'static str, Life>>);

struct Life {
    created: Instant,
    hidden_since: Option<Instant>,
}

impl Lifecycle {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<&'static str, Life>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn lazy_label(label: &str) -> Option<&'static str> {
    LAZY.into_iter().find(|l| *l == label)
}

/**
 * SOURCE OF TRUTH KEYWORDS: ensure window, lazy window, create on demand, from_config, WebviewWindowBuilder
 * WHAT:  Returns the window with this label, building it from its `tauri.conf.json` entry
 *        (marked `"create": false`) if it does not exist yet, and marks it as in use.
 * WHY:   Each window is its own WebKit process (40 to 200 MB, and CPU while it animates).
 *        Building on demand keeps launch to the main window only.
 * WHERE: every place that shows a window: floating.rs (countdown, HUD, toasts, region),
 *        the menu bar panel below, and `open_window`. Call on the main thread.
 */
pub(crate) fn ensure(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    let lazy = lazy_label(label);
    if let Some(win) = app.get_webview_window(label) {
        if let (Some(l), Some(life)) = (lazy, app.try_state::<Lifecycle>())
            && let Some(entry) = life.lock().get_mut(l)
        {
            entry.hidden_since = None;
        }
        return Some(win);
    }
    let lazy = lazy?;
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|c| c.label == label)?
        .clone();
    let started = Instant::now();
    let win = match WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build()) {
        Ok(w) => w,
        Err(e) => {
            log_line!("could not create the {label} window: {e}");
            return None;
        }
    };
    if let Some(life) = app.try_state::<Lifecycle>() {
        life.lock().insert(
            lazy,
            Life {
                created: started,
                hidden_since: None,
            },
        );
    }
    if label == MENUBAR {
        let handle = app.clone();
        win.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                hide_menubar(&handle);
            }
        });
    }
    if label == ONBOARDING {
        crate::tour::on_window_created(app, &win);
    }
    floating::on_created(app, &win);
    log_line!(
        "created the {label} window in {} ms",
        started.elapsed().as_millis()
    );
    Some(win)
}

/// Logs how long a lazy window took from creation to its first render (`window_ready`).
pub(crate) fn log_ready(app: &AppHandle, label: &str) {
    let Some(life) = app.try_state::<Lifecycle>() else {
        return;
    };
    if let Some(entry) = lazy_label(label).and_then(|l| life.lock().get(l).map(|e| e.created)) {
        log_line!(
            "{label} window ready {} ms after creation",
            entry.elapsed().as_millis()
        );
    }
}

/// Destroys lazy windows that have been hidden for [`IDLE_CLOSE`], unless floating.rs
/// still needs them (HUD and menu bar panel while recording, the countdown while warm).
fn reap(app: &AppHandle) {
    let Some(life) = app.try_state::<Lifecycle>() else {
        return;
    };
    let now = Instant::now();
    for label in LAZY {
        let Some(win) = app.get_webview_window(label) else {
            life.lock().remove(label);
            continue;
        };
        let shown = win.is_visible().unwrap_or(true) || win.is_minimized().unwrap_or(false);
        let expired = {
            let mut map = life.lock();
            let entry = map.entry(label).or_insert(Life {
                created: now,
                hidden_since: None,
            });
            if shown {
                entry.hidden_since = None;
                false
            } else {
                now.duration_since(*entry.hidden_since.get_or_insert(now)) >= IDLE_CLOSE
            }
        };
        if !expired || floating::keep_warm(app, label) {
            continue;
        }
        life.lock().remove(label);
        floating::before_destroy(app, label);
        match win.destroy() {
            Ok(()) => log_line!("closed the hidden {label} window"),
            Err(e) => log_line!("could not close the {label} window: {e}"),
        }
    }
}

/// A window `open_window` may show. The floating ones open through `open_floating`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AppWindow {
    Main,
    Settings,
    Onboarding,
    Trim,
}

/// Show (creating it if needed) and focus one of the app's normal windows.
#[tauri::command]
#[specta::specta]
pub fn open_window(app: AppHandle, window: AppWindow) -> Result<(), EngineError> {
    let label = match window {
        AppWindow::Main => {
            show_main(&app);
            return Ok(());
        }
        AppWindow::Settings => SETTINGS,
        AppWindow::Onboarding => ONBOARDING,
        AppWindow::Trim => TRIM,
    };
    let win = ensure(&app, label)
        .ok_or_else(|| EngineError::failed(format!("Could not open the {label} window")))?;
    let fail = |e: tauri::Error| EngineError::failed(format!("Could not show the window: {e}"));
    win.unminimize().map_err(fail)?;
    win.show().map_err(fail)?;
    win.set_focus().map_err(fail)
}

/// The status icon: the black template on macOS, which tints it for the menu bar; the
/// colour mark on Windows and Linux, where a black icon disappears on a dark taskbar.
fn tray_icon() -> tauri::image::Image<'static> {
    if cfg!(target_os = "macos") {
        tauri::include_image!("icons/tray-template.png")
    } else {
        tauri::include_image!("icons/tray-color.png")
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    app.manage(PanelHiddenAt::default());
    app.manage(Lifecycle::default());
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(REAP_EVERY);
            let h = handle.clone();
            if handle.run_on_main_thread(move || reap(&h)).is_err() {
                break;
            }
        }
    });

    let open = MenuItem::with_id(app, "open", "Open Cortex", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Cortex", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id("status")
        .icon(tray_icon())
        // Only macOS tints template images for light and dark menu bars.
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Cortex")
        .menu(&menu)
        // Left click opens our panel; right click shows the small native menu.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                toggle_menubar(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    // Closing the main window hides it; the app keeps running in the menu bar.
    if let Some(main) = app.get_webview_window(MAIN) {
        let win = main.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = win.hide();
            }
        });
    }
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

pub(crate) fn hide_menubar(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(MENUBAR)
        && panel.is_visible().unwrap_or(false)
    {
        let _ = panel.hide();
        if let Ok(mut at) = app.state::<PanelHiddenAt>().0.lock() {
            *at = Some(Instant::now());
        }
    }
}

fn toggle_menubar(app: &AppHandle, tray_rect: Rect) {
    if app
        .get_webview_window(MENUBAR)
        .is_some_and(|p| p.is_visible().unwrap_or(false))
    {
        hide_menubar(app);
        return;
    }
    // The click that blurred (and so hid) the panel also lands here: treat it as "close".
    let just_hidden = app
        .state::<PanelHiddenAt>()
        .0
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|at| at.elapsed() < Duration::from_millis(250));
    if just_hidden {
        return;
    }
    let Some(panel) = ensure(app, MENUBAR) else {
        return;
    };
    if let Err(err) = place_under(&panel, tray_rect) {
        log_line!("could not position the menu bar panel: {err}");
    }
    let _ = panel.show();
    let _ = panel.set_focus();
}

/// Opens the menu bar panel under the status item (from the HUD or a hotkey, not a click).
pub(crate) fn open_menubar(app: &AppHandle) {
    let (Some(panel), Some(tray)) = (ensure(app, MENUBAR), app.tray_by_id("status")) else {
        return;
    };
    if let Ok(Some(rect)) = tray.rect()
        && let Err(err) = place_under(&panel, rect)
    {
        log_line!("could not position the menu bar panel: {err}");
    }
    let _ = panel.show();
    let _ = panel.set_focus();
}

/// Centres the panel under the tray icon, kept inside the icon's monitor.
fn place_under(panel: &WebviewWindow, tray_rect: Rect) -> tauri::Result<()> {
    let scale = panel.scale_factor()?;
    let icon_pos = tray_rect.position.to_physical::<f64>(scale);
    let icon_size = tray_rect.size.to_physical::<f64>(scale);
    let monitor = panel.monitor_from_point(icon_pos.x, icon_pos.y)?;
    let scale = monitor.as_ref().map_or(scale, |m| m.scale_factor());

    let width = MENUBAR_WIDTH * scale;
    let mut x = icon_pos.x + icon_size.width / 2.0 - width / 2.0;
    let y = icon_pos.y + icon_size.height + MENUBAR_GAP * scale;
    if let Some(m) = monitor {
        let left = f64::from(m.position().x);
        let right = left + f64::from(m.size().width);
        let margin = 8.0 * scale;
        x = x.clamp(left + margin, (right - width - margin).max(left + margin));
    }
    panel.set_position(PhysicalPosition::new(x.round(), y.round()))
}
