//! SOURCE OF TRUTH KEYWORDS: floating windows, HUD, toasts, countdown, region picker, global hotkeys, fit_floating
//! WHAT:  Owns the recorder's floating windows: gives them native panel behaviour at start,
//!        shows and hides them from engine state (countdown, HUD), places them on the right
//!        display, sizes them to their web content, and registers the global hotkeys
//!        (⇧⌘R record/stop, ⇧⌘S save replay, Esc during the countdown).
//! WHY:   The engine is the source of truth for "are we recording", so the windows follow its
//!        events in Rust instead of each webview guessing. Hotkeys use Carbon
//!        (`rec_engine::hotkey`), which needs no Accessibility permission.
//! WHERE: `lib.rs` builds it in `setup` and feeds it every outbound event; `ipc.rs` exposes
//!        the three commands; the floating surfaces call them through `app/tauri-window.tsx`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use rec_core::api::EngineError;
use rec_core::settings::RecorderSettings;
use rec_core::state::EngineStatus;
use rec_engine::hotkey;
use rec_macos_ui::{
    FloatingKind, configure_floating_window, invalidate_window_shadow,
    set_window_ignores_mouse_events,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    App, AppHandle, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewWindow, WindowEvent,
};

use crate::shell::{Outbound, Shell};
use crate::windows::{self, MAIN, MENUBAR};

pub const HUD: &str = "hud";
pub const TOASTS: &str = "toasts";
pub const COUNTDOWN: &str = "countdown";
pub const REGION: &str = "region";

/// Every floating window and the AppKit behaviour it gets.
const KINDS: [(&str, FloatingKind); 5] = [
    (MENUBAR, FloatingKind::MenubarPanel),
    (HUD, FloatingKind::Hud),
    (TOASTS, FloatingKind::Toast),
    (COUNTDOWN, FloatingKind::Countdown),
    (REGION, FloatingKind::RegionPicker),
];

/// Logical gap between the toasts and the menu bar / right screen edge.
const TOAST_MARGIN: f64 = 10.0;
/// Logical distance from the bottom of the display to the HUD's default spot.
const HUD_BOTTOM: f64 = 48.0;
/// The HUD stays this far (logical) inside the display's visible area.
const EDGE: f64 = 16.0;
/// Content sizes beyond this are a bug in the page, not a real request.
const MAX_SIDE: f64 = 8192.0;

const HOTKEY_RECORD: u32 = 1;
const HOTKEY_REPLAY: u32 = 2;
const HOTKEY_ESCAPE: u32 = 3;
/// kVK_Escape.
const KEY_ESCAPE: u32 = 53;

/// What the orchestrator remembers between events. Only touched briefly; window work
/// runs on the main thread.
#[derive(Default)]
struct Inner {
    status: EngineStatus,
    show_hud: bool,
    replay_enabled: bool,
    /// The user hid the HUD for this session ("Hide HUD"); reset by the next recording.
    hud_dismissed: bool,
    main_focused: bool,
    /// The HUD's bottom-centre per display, in logical points from the display's top-left.
    hud_spots: HashMap<String, (f64, f64)>,
    hud_spots_path: Option<PathBuf>,
}

/// Shared orchestrator state (managed by Tauri, also held by the event observer).
pub struct Floating(Mutex<Inner>);

impl Floating {
    pub fn new() -> Arc<Floating> {
        Arc::new(Floating(Mutex::new(Inner::default())))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Which floating surface `open_floating` shows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum FloatingSurface {
    /// The menu bar panel, under the status item.
    Menubar,
    /// The region picker, over the whole display under the cursor.
    Region,
    /// The recording HUD, after the user hid it.
    Hud,
}

/**
 * SOURCE OF TRUTH KEYWORDS: floating::init, configure floating windows, hotkey registration, HUD positions
 * WHAT:  Configures every floating window (level, all Spaces, non-activating, excluded from
 *        sharing), wires the HUD/main window events and registers the global hotkeys.
 * WHY:   Native behaviour must be in place before any of these windows is first shown.
 * WHERE: `lib.rs` setup, after the shell is managed. Runs on the main thread.
 */
pub fn init(app: &mut App, floating: Arc<Floating>, settings: &RecorderSettings) {
    let spots_path = app
        .path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("hud-positions.json"));
    let hud_spots = spots_path
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let main_focused = app
        .get_webview_window(MAIN)
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    {
        let mut g = floating.lock();
        g.show_hud = settings.show_hud;
        g.main_focused = main_focused;
        g.hud_spots = hud_spots;
        g.hud_spots_path = spots_path;
    }
    app.manage(floating);

    for (label, kind) in KINDS {
        let Some(win) = app.get_webview_window(label) else {
            continue;
        };
        match configure_floating_window(&win, kind) {
            Some(how) => eprintln!("cortex: {label} window floats ({how:?})"),
            None => eprintln!("cortex: could not configure the {label} window"),
        }
    }
    // The countdown never takes clicks meant for the app being recorded.
    if let Some(win) = app.get_webview_window(COUNTDOWN) {
        set_window_ignores_mouse_events(&win, true);
    }

    let handle = app.handle().clone();
    if let Some(hud) = app.get_webview_window(HUD) {
        let (h, win) = (handle.clone(), hud.clone());
        hud.on_window_event(move |e| {
            if let WindowEvent::Moved(_) = e {
                remember_hud_spot(&h, &win);
            }
        });
    }
    // The HUD steps aside while the main window is frontmost (it has the same controls).
    if let Some(main) = app.get_webview_window(MAIN) {
        let h = handle.clone();
        main.on_window_event(move |e| {
            if let WindowEvent::Focused(focused) = e {
                state(&h).lock().main_focused = *focused;
                refresh_hud(&h);
            }
        });
    }

    let h = handle.clone();
    register_hotkey(HOTKEY_RECORD, hotkey::KEY_R, move || toggle_record(&h));
    apply_settings(&handle, settings.show_hud, settings.replay_enabled);
}

fn state(app: &AppHandle) -> Arc<Floating> {
    app.state::<Arc<Floating>>().inner().clone()
}

/**
 * SOURCE OF TRUTH KEYWORDS: floating observer, engine status, countdown window, HUD visibility, settings hotkeys
 * WHAT:  Watches outbound engine events and moves the floating windows when the engine
 *        status or the HUD/replay settings change.
 * WHY:   One place maps engine state to window state. Window work hops to the main thread,
 *        so the engine's event thread never waits on AppKit.
 * WHERE: `lib.rs` wraps the IPC emitter with it.
 */
pub fn observer(app: AppHandle, floating: Arc<Floating>) -> impl Fn(&Outbound) + Send + Sync {
    move |o| match o {
        Outbound::State(s) => {
            let from = {
                let mut g = floating.lock();
                std::mem::replace(&mut g.status, s.status)
            };
            if from != s.status {
                let (a, to) = (app.clone(), s.status);
                let _ = app.run_on_main_thread(move || on_status(&a, from, to));
            }
        }
        Outbound::Settings(s) => {
            let (a, hud, replay) = (app.clone(), s.show_hud, s.replay_enabled);
            let _ = app.run_on_main_thread(move || apply_settings(&a, hud, replay));
        }
        _ => {}
    }
}

fn on_status(app: &AppHandle, from: EngineStatus, to: EngineStatus) {
    if from == EngineStatus::Idle {
        state(app).lock().hud_dismissed = false;
    }
    if to == EngineStatus::Countdown {
        show_countdown(app);
        let a = app.clone();
        register_hotkey(HOTKEY_ESCAPE, KEY_ESCAPE, move || cancel_countdown(&a));
    } else if from == EngineStatus::Countdown {
        hotkey::unregister(HOTKEY_ESCAPE);
        hide(app, COUNTDOWN);
    }
    refresh_hud(app);
}

/// HUD on/off setting and the replay hotkey (only while the replay buffer is enabled).
fn apply_settings(app: &AppHandle, show_hud: bool, replay_enabled: bool) {
    let replay_changed = {
        let s = state(app);
        let mut g = s.lock();
        g.show_hud = show_hud;
        std::mem::replace(&mut g.replay_enabled, replay_enabled) != replay_enabled
    };
    if replay_changed {
        if replay_enabled {
            let a = app.clone();
            register_hotkey(HOTKEY_REPLAY, hotkey::KEY_S, move || save_replay(&a));
        } else {
            hotkey::unregister(HOTKEY_REPLAY);
        }
    }
    refresh_hud(app);
}

fn register_hotkey(id: u32, key: u32, f: impl Fn() + 'static) {
    let modifiers = if id == HOTKEY_ESCAPE {
        0
    } else {
        hotkey::CMD | hotkey::SHIFT
    };
    if let Err(code) = hotkey::register(id, key, modifiers, f) {
        eprintln!("cortex: hotkey {id} is taken by another app (Carbon error {code})");
    }
}

/// Runs an engine call off the main thread (it can block); failures become toasts.
fn engine_call(
    app: &AppHandle,
    f: impl FnOnce(&Shell) -> Result<(), EngineError> + Send + 'static,
) {
    let Some(shell) = app.try_state::<Arc<Shell>>().map(|s| s.inner().clone()) else {
        return;
    };
    let emit = crate::ipc::emitter(app.clone());
    std::thread::spawn(move || {
        if let Err(e) = f(&shell) {
            emit(Outbound::Error {
                message: e.to_string(),
                recoverable: true,
            });
        }
    });
}

/// ⇧⌘R: start, cancel the countdown, or stop and save.
fn toggle_record(app: &AppHandle) {
    engine_call(app, |s| match s.state().status {
        EngineStatus::Idle => s.start(),
        EngineStatus::Countdown => s.cancel_countdown(),
        EngineStatus::Recording | EngineStatus::Paused => s.stop(),
        EngineStatus::Finalizing => Ok(()),
    });
}

fn save_replay(app: &AppHandle) {
    engine_call(app, Shell::save_replay);
}

fn cancel_countdown(app: &AppHandle) {
    engine_call(app, |s| {
        if s.state().status == EngineStatus::Countdown {
            s.cancel_countdown()
        } else {
            Ok(())
        }
    });
}

fn hide(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.hide();
    }
}

fn visible(w: &WebviewWindow) -> bool {
    w.is_visible().unwrap_or(false)
}

/// The display under the mouse pointer, else the main display.
fn active_monitor(app: &AppHandle) -> Option<Monitor> {
    app.cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
}

fn monitor_key(m: &Monitor) -> String {
    m.name()
        .cloned()
        .unwrap_or_else(|| format!("{},{}", m.position().x, m.position().y))
}

/// The countdown, centred on the main display (the one with the menu bar).
fn show_countdown(app: &AppHandle) {
    let Some(win) = app.get_webview_window(COUNTDOWN) else {
        return;
    };
    let monitor = app.primary_monitor().ok().flatten();
    if let (Some(m), Ok(size)) = (monitor, win.outer_size()) {
        let x =
            f64::from(m.position().x) + (f64::from(m.size().width) - f64::from(size.width)) / 2.0;
        let y =
            f64::from(m.position().y) + (f64::from(m.size().height) - f64::from(size.height)) / 2.0;
        let _ = win.set_position(PhysicalPosition::new(x.round(), y.round()));
    }
    let _ = win.show();
}

/// Shows or hides the HUD: visible while a file is being written, the setting is on, the
/// user did not hide it, and the main window is not frontmost.
fn refresh_hud(app: &AppHandle) {
    let Some(win) = app.get_webview_window(HUD) else {
        return;
    };
    let wanted = {
        let s = state(app);
        let g = s.lock();
        g.status.is_capturing() && g.show_hud && !g.hud_dismissed && !g.main_focused
    };
    let shown = visible(&win);
    if wanted && !shown {
        place_hud(app, &win);
        let _ = win.show();
    } else if !wanted && shown {
        let _ = win.hide();
        save_hud_spots(app);
    }
}

/// Puts the HUD at its remembered spot on the active display (bottom centre by default),
/// kept inside the display's visible area.
fn place_hud(app: &AppHandle, win: &WebviewWindow) {
    let (Some(m), Ok(size)) = (active_monitor(app), win.outer_size()) else {
        return;
    };
    let scale = m.scale_factor();
    let origin = m.position();
    let spot = state(app).lock().hud_spots.get(&monitor_key(&m)).copied();
    let (bx, by) = match spot {
        Some((x, y)) => (
            f64::from(origin.x) + x * scale,
            f64::from(origin.y) + y * scale,
        ),
        None => (
            f64::from(origin.x) + f64::from(m.size().width) / 2.0,
            f64::from(origin.y) + f64::from(m.size().height) - HUD_BOTTOM * scale,
        ),
    };
    let (w, h) = (f64::from(size.width), f64::from(size.height));
    let area = m.work_area();
    let edge = EDGE * scale;
    let left = f64::from(area.position.x) + edge;
    let top = f64::from(area.position.y) + edge;
    let right = f64::from(area.position.x) + f64::from(area.size.width) - edge - w;
    let bottom = f64::from(area.position.y) + f64::from(area.size.height) - edge - h;
    let x = (bx - w / 2.0).clamp(left, right.max(left));
    let y = (by - h).clamp(top, bottom.max(top));
    let _ = win.set_position(PhysicalPosition::new(x.round(), y.round()));
}

/// Stores the HUD's bottom-centre for the display it is on (after the user drags it).
fn remember_hud_spot(app: &AppHandle, win: &WebviewWindow) {
    if !visible(win) {
        return;
    }
    let (Ok(Some(m)), Ok(pos), Ok(size)) = (
        win.current_monitor(),
        win.outer_position(),
        win.outer_size(),
    ) else {
        return;
    };
    let scale = m.scale_factor();
    let x = (f64::from(pos.x) + f64::from(size.width) / 2.0 - f64::from(m.position().x)) / scale;
    let y = (f64::from(pos.y) + f64::from(size.height) - f64::from(m.position().y)) / scale;
    state(app).lock().hud_spots.insert(monitor_key(&m), (x, y));
}

fn save_hud_spots(app: &AppHandle) {
    let (path, spots) = {
        let s = state(app);
        let g = s.lock();
        (g.hud_spots_path.clone(), g.hud_spots.clone())
    };
    let Some(path) = path else { return };
    let write = || -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, serde_json::to_vec(&spots)?)
    };
    if let Err(e) = write() {
        eprintln!("cortex: could not save the HUD position: {e}");
    }
}

/// Top-right of `m`'s visible area, just under the menu bar.
/// `width` is the window's new physical width (the window may not report it yet).
fn place_toasts(win: &WebviewWindow, m: &Monitor, width: f64) {
    let scale = m.scale_factor();
    let area = m.work_area();
    let x = f64::from(area.position.x) + f64::from(area.size.width) - width - TOAST_MARGIN * scale;
    let y = f64::from(area.position.y) + TOAST_MARGIN * scale;
    let _ = win.set_position(PhysicalPosition::new(x.round(), y.round()));
}

fn failed(e: tauri::Error) -> EngineError {
    EngineError::failed(format!("Could not update the window: {e}"))
}

/// Size the calling floating window to its content (logical points). The toasts window
/// also shows itself top-right on the active display and hides at height 0; the HUD keeps
/// its bottom-centre and the countdown its centre.
#[tauri::command]
#[specta::specta]
pub fn fit_floating(window: WebviewWindow, width: f64, height: f64) -> Result<(), EngineError> {
    if !width.is_finite() || !height.is_finite() {
        return Err(EngineError::failed("The window size must be a number"));
    }
    let (width, height) = (width.clamp(0.0, MAX_SIDE), height.clamp(0.0, MAX_SIDE));
    let label = window.label().to_string();
    if label == TOASTS && height < 1.0 {
        return window.hide().map_err(failed);
    }
    if width < 1.0 || height < 1.0 {
        return Ok(());
    }
    // The point that must not move: the HUD's bottom-centre (once placed), the countdown's centre.
    let keep_y = match label.as_str() {
        HUD if visible(&window) => Some(1.0),
        COUNTDOWN => Some(0.5),
        _ => None,
    };
    let anchor =
        keep_y.and_then(|fy| Some((fy, window.outer_position().ok()?, window.outer_size().ok()?)));
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(failed)?;
    // The new size in pixels; the window reports it only after AppKit applies the resize.
    let scale = window.scale_factor().map_err(failed)?;
    let (new_w, new_h) = (width * scale, height * scale);
    match label.as_str() {
        TOASTS => {
            let shown = visible(&window);
            let app = window.app_handle();
            let monitor = if shown {
                window.current_monitor().ok().flatten()
            } else {
                active_monitor(app)
            };
            if let Some(m) = monitor {
                place_toasts(&window, &m, new_w);
            }
            if !shown {
                window.show().map_err(failed)?;
            }
        }
        _ => {
            if let Some((fy, pos, old)) = anchor {
                let x = f64::from(pos.x) + (f64::from(old.width) - new_w) / 2.0;
                let y = f64::from(pos.y) + (f64::from(old.height) - new_h) * fy;
                let _ = window.set_position(PhysicalPosition::new(x.round(), y.round()));
            }
        }
    }
    invalidate_window_shadow(&window);
    Ok(())
}

/// Show a floating surface in its place: the menu bar panel under the status item, the
/// region picker over the display under the cursor, or the HUD after the user hid it.
#[tauri::command]
#[specta::specta]
pub fn open_floating(app: AppHandle, surface: FloatingSurface) -> Result<(), EngineError> {
    match surface {
        FloatingSurface::Menubar => windows::open_menubar(&app),
        FloatingSurface::Hud => {
            state(&app).lock().hud_dismissed = false;
            refresh_hud(&app);
        }
        FloatingSurface::Region => {
            let win = app
                .get_webview_window(REGION)
                .ok_or_else(|| EngineError::failed("The region picker is missing"))?;
            // One overlay per display is the goal; for now the display where the pick starts.
            if let Some(m) = active_monitor(&app) {
                win.set_position(*m.position()).map_err(failed)?;
                win.set_size(*m.size()).map_err(failed)?;
            }
            win.show().map_err(failed)?;
            win.set_focus().map_err(failed)?;
        }
    }
    Ok(())
}

/// Hide the calling floating window. A hidden HUD stays hidden until the next recording.
#[tauri::command]
#[specta::specta]
pub fn hide_floating(window: WebviewWindow) -> Result<(), EngineError> {
    let app = window.app_handle().clone();
    match window.label() {
        MENUBAR => windows::hide_menubar(&app),
        HUD => {
            state(&app).lock().hud_dismissed = true;
            refresh_hud(&app);
        }
        _ => window.hide().map_err(failed)?,
    }
    Ok(())
}

/// Debug runs only (`CORTEX_DEBUG_CAPTURABLE=1`): let screenshots see the floating windows.
#[cfg(debug_assertions)]
pub fn make_capturable(app: &AppHandle) {
    for (label, _) in KINDS {
        if let Some(w) = app.get_webview_window(label) {
            rec_macos_ui::panel::set_window_capturable(&w);
        }
    }
}

/// Saves the HUD spots on quit.
pub fn shutdown(app: &AppHandle) {
    if app.try_state::<Arc<Floating>>().is_some() {
        save_hud_spots(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_floating_window_has_a_distinct_label() {
        let mut labels: Vec<_> = KINDS.iter().map(|(l, _)| *l).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), KINDS.len());
    }

    #[test]
    fn surfaces_use_kebab_case_on_the_wire() {
        let s = serde_json::to_string(&FloatingSurface::Menubar).unwrap();
        assert_eq!(s, "\"menubar\"");
    }
}
