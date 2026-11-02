//! Window and menu bar behaviour.
//!
//! All windows are declared in `tauri.conf.json`; this module wires the menu bar
//! (tray) icon to the "menubar" panel and keeps the main window alive on close.
//!
//! The floating windows' native behaviour (levels, non-activation, capture
//! exclusion), placement and engine-driven visibility live in `floating.rs`.
//! Still to come: one region overlay per display and a native `NSStatusItem`
//! (recording clock, drag target) replacing the tray icon.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

use tauri::{
    App, AppHandle, Manager, PhysicalPosition, Rect, WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub(crate) const MENUBAR: &str = "menubar";
pub(crate) const MAIN: &str = "main";
/// Logical width of the menu bar panel; its height follows the content (set from the webview).
const MENUBAR_WIDTH: f64 = 340.0;
/// Gap between the menu bar and the panel, in logical points.
const MENUBAR_GAP: f64 = 6.0;

/// When the panel last hid itself on blur. A tray click right after must not reopen it.
#[derive(Default)]
struct PanelHiddenAt(Mutex<Option<Instant>>);

pub fn setup(app: &mut App) -> tauri::Result<()> {
    app.manage(PanelHiddenAt::default());

    let open = MenuItem::with_id(app, "open", "Open Cortex", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Cortex", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id("status")
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true)
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

    if let Some(panel) = app.get_webview_window(MENUBAR) {
        let handle = app.handle().clone();
        panel.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                hide_menubar(&handle);
            }
        });
    }

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
    let Some(panel) = app.get_webview_window(MENUBAR) else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
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
    if let Err(err) = place_under(&panel, tray_rect) {
        eprintln!("cortex: could not position the menu bar panel: {err}");
    }
    let _ = panel.show();
    let _ = panel.set_focus();
}

/// Opens the menu bar panel under the status item (from the HUD or a hotkey, not a click).
pub(crate) fn open_menubar(app: &AppHandle) {
    let (Some(panel), Some(tray)) = (app.get_webview_window(MENUBAR), app.tray_by_id("status"))
    else {
        return;
    };
    if let Ok(Some(rect)) = tray.rect()
        && let Err(err) = place_under(&panel, rect)
    {
        eprintln!("cortex: could not position the menu bar panel: {err}");
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
