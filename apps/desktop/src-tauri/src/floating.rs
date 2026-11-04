//! SOURCE OF TRUTH KEYWORDS: floating windows, HUD, toasts, countdown, region picker, global hotkeys, fit_floating
//! WHAT:  Owns the recorder's floating windows: gives them native panel behaviour at start,
//!        shows and hides them from engine state (countdown, HUD), places them on the right
//!        display, sizes them to their web content, and registers the global hotkeys
//!        (⇧⌘R record/stop, ⇧⌘S save replay, Esc during the countdown).
//! WHY:   The engine is the source of truth for "are we recording", so the windows follow its
//!        events in Rust instead of each webview guessing. Panels and hotkeys go through
//!        `platform` (on macOS: `rec_macos_ui` panels and Carbon hotkeys, which need no
//!        Accessibility permission).
//! WHERE: `lib.rs` builds it in `setup` and feeds it every outbound event; `ipc.rs` exposes
//!        the three commands; the floating surfaces call them through `app/tauri-window.tsx`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use rec_core::api::EngineError;
use rec_core::settings::RecorderSettings;
use rec_core::state::{EngineStatus, Recording};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    App, AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, WebviewWindow,
    WindowEvent,
};
use tauri_specta::Event;

use crate::ipc::{EngineErrorEvent, RecordingFinishedEvent, ReplaySavedEvent};
use crate::platform::{self, FloatingKind, Hotkey, HotkeyKey};
use crate::shell::{Outbound, Shell};
use crate::windows::{self, MAIN, MENUBAR};

pub const HUD: &str = "hud";
pub const TOASTS: &str = "toasts";
pub const COUNTDOWN: &str = "countdown";
pub const REGION: &str = "region";

/// Every floating window and the native behaviour it gets.
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

/// Toasts other windows send to the toasts window (`lib/engine.ts` listens for it).
const TOAST_EVENT: &str = "ui://toast";
/// Toasts held while the toasts window loads; older ones are dropped beyond this.
const MAX_PENDING_TOASTS: usize = 8;
/// Longest toast title or detail accepted from a webview, in characters.
const MAX_TOAST_TEXT: usize = 500;

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
    /// Seconds of countdown before a recording (settings); above 0 keeps the countdown warm.
    countdown_secs: u32,
    /// The toasts window has rendered and listens; until then toasts wait in `pending_toasts`.
    toasts_ready: bool,
    pending_toasts: Vec<ToastEvent>,
    /// Debug runs: new floating windows are made visible to screenshots.
    capturable: bool,
}

/// Something the toasts window turns into a toast.
#[derive(Debug, Clone)]
enum ToastEvent {
    Finished(Recording),
    ReplaySaved(String),
    Error { message: String, recoverable: bool },
    Ui(UiToast),
}

/// The kind of a toast sent from a window (matches `ToastKind` in `lib/types.ts`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum UiToastKind {
    Saved,
    Replay,
    Warning,
}

/// A toast another window asks the toasts window to show (`Toast` in `lib/types.ts`).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UiToast {
    pub id: String,
    pub kind: UiToastKind,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
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
 * WHAT:  Sets up the floating-window state, wires the main window's focus events and
 *        registers the global hotkeys. The windows themselves are built on demand.
 * WHY:   Hotkeys and the HUD rules must work before any floating window exists.
 * WHERE: `lib.rs` setup, after the shell is managed. Runs on the main thread.
 *        `data_dir` is the settings folder (`CORTEX_DEBUG_DATA_DIR` in test runs, so they
 *        never write the user's HUD positions).
 */
pub fn init(
    app: &mut App,
    floating: Arc<Floating>,
    settings: &RecorderSettings,
    data_dir: &std::path::Path,
    capturable: bool,
) {
    let spots_path = Some(data_dir.join("hud-positions.json"));
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
        g.capturable = capturable;
    }
    app.manage(floating);

    let handle = app.handle().clone();
    // The HUD steps aside while the main window is frontmost (it has the same controls).
    // That is also when a recording is likely to start, so the countdown is built then.
    if let Some(main) = app.get_webview_window(MAIN) {
        let h = handle.clone();
        main.on_window_event(move |e| {
            if let WindowEvent::Focused(focused) = e {
                state(&h).lock().main_focused = *focused;
                refresh_hud(&h);
                if *focused {
                    warm_countdown(&h);
                }
            }
        });
    }

    let h = handle.clone();
    register_hotkey(HOTKEY_RECORD, HotkeyKey::R, move || toggle_record(&h));
    apply_settings(
        &handle,
        settings.show_hud,
        settings.replay_enabled,
        settings.countdown,
    );
}

/**
 * SOURCE OF TRUTH KEYWORDS: floating::on_created, configure floating window, lazy floating window, panel level
 * WHAT:  Gives a newly built floating window its native panel behaviour (level, all Spaces,
 *        non-activating, excluded from sharing) and its event handlers.
 * WHY:   Floating windows are built on demand (`windows::ensure`); the behaviour must be in
 *        place before the window is first shown.
 * WHERE: `windows::ensure`, right after the window is built. Runs on the main thread.
 */
pub(crate) fn on_created(app: &AppHandle, win: &WebviewWindow) {
    let label = win.label();
    let Some(kind) = KINDS.iter().find(|(l, _)| *l == label).map(|(_, k)| *k) else {
        return;
    };
    if !platform::chrome().configure_floating(win, kind) {
        eprintln!("cortex: could not configure the {label} window");
    }
    match label {
        // The countdown never takes clicks meant for the app being recorded.
        COUNTDOWN => platform::chrome().set_ignores_mouse_events(win, true),
        HUD => {
            let (h, w) = (app.clone(), win.clone());
            win.on_window_event(move |e| {
                if let WindowEvent::Moved(_) = e {
                    remember_hud_spot(&h, &w);
                }
            });
        }
        TOASTS => {
            let h = app.clone();
            win.on_window_event(move |e| {
                if let WindowEvent::Destroyed = e {
                    state(&h).lock().toasts_ready = false;
                }
            });
        }
        _ => {}
    }
    #[cfg(debug_assertions)]
    if state(app).lock().capturable {
        platform::chrome().set_capturable(win);
    }
}

/// Whether a hidden floating window must survive the idle reaper: the HUD and the menu
/// bar panel while recording, the countdown while it runs or is warm, and the toasts
/// window until it has delivered what it holds.
pub(crate) fn keep_warm(app: &AppHandle, label: &str) -> bool {
    let Some(s) = app.try_state::<Arc<Floating>>() else {
        return false;
    };
    let g = s.lock();
    match label {
        HUD | MENUBAR => g.status.is_capturing(),
        COUNTDOWN => {
            g.status == EngineStatus::Countdown || (g.countdown_secs > 0 && main_shown(app))
        }
        TOASTS => !g.toasts_ready || !g.pending_toasts.is_empty(),
        _ => false,
    }
}

/// Called just before the reaper destroys a window.
pub(crate) fn before_destroy(app: &AppHandle, label: &str) {
    if label == TOASTS
        && let Some(s) = app.try_state::<Arc<Floating>>()
    {
        s.lock().toasts_ready = false;
    }
}

fn main_shown(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
}

/// Builds the countdown window ahead of time when a countdown is configured and the main
/// window is in use, so the first number shows without waiting for a WebKit process.
fn warm_countdown(app: &AppHandle) {
    if state(app).lock().countdown_secs == 0 || !main_shown(app) {
        return;
    }
    let a = app.clone();
    // Not from inside a window event handler: queue it on the main thread.
    let _ = app.run_on_main_thread(move || {
        windows::ensure(&a, COUNTDOWN);
    });
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
            let (a, hud, replay, cd) = (app.clone(), s.show_hud, s.replay_enabled, s.countdown);
            let _ = app.run_on_main_thread(move || apply_settings(&a, hud, replay, cd));
        }
        Outbound::RecordingFinished(r) => {
            deliver_toast(&app, &floating, ToastEvent::Finished(r.clone()))
        }
        Outbound::ReplaySaved(p) => {
            deliver_toast(&app, &floating, ToastEvent::ReplaySaved(p.clone()))
        }
        Outbound::Error {
            message,
            recoverable,
        } => deliver_toast(
            &app,
            &floating,
            ToastEvent::Error {
                message: message.clone(),
                recoverable: *recoverable,
            },
        ),
        Outbound::Levels(_) => {}
    }
}

fn emit_toast(app: &AppHandle, t: ToastEvent) -> tauri::Result<()> {
    match t {
        ToastEvent::Finished(r) => RecordingFinishedEvent(r).emit_to(app, TOASTS),
        ToastEvent::ReplaySaved(path) => ReplaySavedEvent { path }.emit_to(app, TOASTS),
        ToastEvent::Error {
            message,
            recoverable,
        } => EngineErrorEvent {
            message,
            recoverable,
        }
        .emit_to(app, TOASTS),
        ToastEvent::Ui(t) => app.emit_to(TOASTS, TOAST_EVENT, t),
    }
}

/// Sends a toast to the toasts window, or holds it and builds the window when the window
/// does not exist or has not rendered yet (`window_ready` then delivers it).
fn deliver_toast(app: &AppHandle, floating: &Floating, t: ToastEvent) {
    {
        let mut g = floating.lock();
        if g.toasts_ready && app.get_webview_window(TOASTS).is_some() {
            if let Err(e) = emit_toast(app, t) {
                eprintln!("cortex: could not send a toast: {e}");
            }
            return;
        }
        g.toasts_ready = false;
        if g.pending_toasts.len() >= MAX_PENDING_TOASTS {
            g.pending_toasts.remove(0);
        }
        g.pending_toasts.push(t);
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        windows::ensure(&a, TOASTS);
    });
}

fn on_status(app: &AppHandle, from: EngineStatus, to: EngineStatus) {
    if from == EngineStatus::Idle {
        state(app).lock().hud_dismissed = false;
    }
    if to == EngineStatus::Countdown {
        show_countdown(app);
        let a = app.clone();
        register_hotkey(HOTKEY_ESCAPE, HotkeyKey::Escape, move || {
            cancel_countdown(&a)
        });
    } else if from == EngineStatus::Countdown {
        platform::hotkeys().unregister(HOTKEY_ESCAPE);
        hide(app, COUNTDOWN);
    }
    refresh_hud(app);
}

/// HUD on/off setting and the replay hotkey (only while the replay buffer is enabled).
fn apply_settings(app: &AppHandle, show_hud: bool, replay_enabled: bool, countdown: u32) {
    let replay_changed = {
        let s = state(app);
        let mut g = s.lock();
        g.show_hud = show_hud;
        g.countdown_secs = countdown;
        std::mem::replace(&mut g.replay_enabled, replay_enabled) != replay_enabled
    };
    if replay_changed {
        if replay_enabled {
            let a = app.clone();
            register_hotkey(HOTKEY_REPLAY, HotkeyKey::S, move || save_replay(&a));
        } else {
            platform::hotkeys().unregister(HOTKEY_REPLAY);
        }
    }
    refresh_hud(app);
    if state(app).lock().main_focused {
        warm_countdown(app);
    }
}

/// ⇧⌘ (Ctrl+Shift off macOS) plus `key`; Esc alone.
fn register_hotkey(id: u32, key: HotkeyKey, f: impl Fn() + 'static) {
    let hotkey = Hotkey {
        key,
        primary_shift: id != HOTKEY_ESCAPE,
    };
    if let Err(e) = platform::hotkeys().register(id, hotkey, Box::new(f)) {
        eprintln!("cortex: hotkey {id} is taken by another app ({e})");
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
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = f(&shell) {
            let t = ToastEvent::Error {
                message: e.to_string(),
                recoverable: true,
            };
            deliver_toast(&app, &state(&app), t);
        }
    });
}

/// ⇧⌘R: start, cancel the countdown, or stop and save.
fn toggle_record(app: &AppHandle) {
    // The tour's shortcut step wants the press for itself (runs on the main thread).
    let status = state(app).lock().status;
    if crate::tour::catch_record_hotkey(app, status) {
        return;
    }
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
    let Some(win) = windows::ensure(app, COUNTDOWN) else {
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
    let wanted = {
        let s = state(app);
        let g = s.lock();
        g.status.is_capturing() && g.show_hud && !g.hud_dismissed && !g.main_focused
    };
    // Built only when it is about to show.
    let win = if wanted {
        windows::ensure(app, HUD)
    } else {
        app.get_webview_window(HUD)
    };
    let Some(win) = win else {
        return;
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
    if crate::trace_on() {
        eprintln!(
            "cortex: trace fit_floating {label} {width}x{height} (scale {:?}, now {:?})",
            window.scale_factor(),
            window.outer_size()
        );
    }
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
    platform::chrome().invalidate_shadow(&window);
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
            let win = windows::ensure(&app, REGION)
                .ok_or_else(|| EngineError::failed("Could not open the region picker"))?;
            // One overlay per display is the goal; for now the display where the pick starts.
            // In points from that display's own scale: the window may still be on a display
            // with another scale, which a physical size would be converted with.
            if let Some(m) = active_monitor(&app) {
                let k = m.scale_factor();
                win.set_position(m.position().to_logical::<f64>(k))
                    .map_err(failed)?;
                win.set_size(m.size().to_logical::<f64>(k))
                    .map_err(failed)?;
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

/// The calling window has rendered and listens for events. The toasts window then gets
/// the toasts held for it.
#[tauri::command]
#[specta::specta]
pub fn window_ready(window: WebviewWindow) {
    let app = window.app_handle();
    windows::log_ready(app, window.label());
    if window.label() != TOASTS {
        return;
    }
    let s = state(app);
    let mut g = s.lock();
    g.toasts_ready = true;
    for t in std::mem::take(&mut g.pending_toasts) {
        if let Err(e) = emit_toast(app, t) {
            eprintln!("cortex: could not send a toast: {e}");
        }
    }
}

/// Show a toast in the toasts window (from any window; failed commands use it).
#[tauri::command]
#[specta::specta]
pub fn push_toast(app: AppHandle, toast: UiToast) -> Result<(), EngineError> {
    let short = |s: &str| s.chars().take(MAX_TOAST_TEXT).collect::<String>();
    let toast = UiToast {
        id: short(&toast.id),
        title: short(&toast.title),
        detail: short(&toast.detail),
        recording_id: toast.recording_id.as_deref().map(short),
        kind: toast.kind,
    };
    deliver_toast(&app, &state(&app), ToastEvent::Ui(toast));
    Ok(())
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
