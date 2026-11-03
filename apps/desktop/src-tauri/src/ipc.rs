//! SOURCE OF TRUTH KEYWORDS: IPC, tauri commands, tauri-specta, bindings.ts, engine://state, engine://levels
//! WHAT:  The single IPC checkpoint (engineering plan §6): every command the webview can call
//!        and every event it can hear. Commands are thin wrappers over `shell::Shell`; all
//!        types come from `rec-core`, so the TypeScript bindings generated from this module
//!        (`apps/desktop/src/ipc/bindings.ts`, `pnpm --filter desktop ipc:bindings`) are the
//!        UI's source of truth.
//! WHY:   One allow-listed surface: windows may only call commands their capability grants
//!        (see `build.rs` and `capabilities/default.json`). Stream keys go in, never out.
//! WHERE: `lib.rs` registers `builder()`; `Shell` emits through `emitter()`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use rec_core::api::EngineError;
use rec_core::settings::{RecorderSettings, StreamDest};
use rec_core::state::{
    Devices, EngineState, EngineStatus, Levels, Permission, PermissionReport, PermissionState,
    Recording,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, EventTarget, Manager, Runtime, State};
use tauri_specta::{Event, collect_commands, collect_events};

use crate::preview::LivePreview;
use crate::shell::{self, Emit, Outbound, Shell};

/// The engine snapshot: on every status change, and at most 10 times a second while
/// recording (clock, bytes). Levels come separately.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EngineStateEvent(pub EngineState);
impl Event for EngineStateEvent {
    const NAME: &'static str = "engine://state";
}

/// Audio meters, at most 30 times a second while a session runs.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LevelsEvent(pub Levels);
impl Event for LevelsEvent {
    const NAME: &'static str = "engine://levels";
}

/// A recording was written.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RecordingFinishedEvent(pub Recording);
impl Event for RecordingFinishedEvent {
    const NAME: &'static str = "engine://recording-finished";
}

/// The replay buffer was saved to `path`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ReplaySavedEvent {
    pub path: String,
}
impl Event for ReplaySavedEvent {
    const NAME: &'static str = "engine://replay-saved";
}

/// Something went wrong. `recoverable` false: the recording or preview stopped.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EngineErrorEvent {
    pub message: String,
    pub recoverable: bool,
}
impl Event for EngineErrorEvent {
    const NAME: &'static str = "engine://error";
}

/// The stored settings changed (from any window, or a stream key was saved).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SettingsChangedEvent(pub RecorderSettings);
impl Event for SettingsChangedEvent {
    const NAME: &'static str = "settings://changed";
}

/// How long the list of visible windows is reused before a refresh is queued.
const VISIBLE_REFRESH: Duration = Duration::from_millis(250);

/**
 * SOURCE OF TRUTH KEYWORDS: VisibleWindows, visible windows, visibility registry, refresh_visible, main thread, quit hang, event fan-out
 * WHAT:  The labels of the windows on screen, as last read on the main thread.
 * WHY:   Events are sent from engine threads. Asking Tauri whether a window is visible
 *        from there waits for the main thread, and at quit the main thread waits for the
 *        engine thread: Cortex hung. Readers here never wait on another thread. A stale
 *        list queues one refresh on the main thread (fire and forget) and is used as is
 *        meanwhile, so windows shown or hidden from anywhere (Rust or the webview) are
 *        picked up within about `VISIBLE_REFRESH` without hooks at every show/hide site.
 * WHERE: `emitter` reads it; `refresh_visible` (setup) and `window_changed` (every
 *        window event, from `lib.rs`) keep it current.
 */
#[derive(Default)]
pub struct VisibleWindows {
    labels: RwLock<Vec<String>>,
    refreshed: Mutex<Option<Instant>>,
    /// A refresh has been queued and has not run yet.
    queued: AtomicBool,
}

impl VisibleWindows {
    /// The labels as last recorded. When they are older than `VISIBLE_REFRESH` and no
    /// refresh is pending, `queue` is called to schedule one; it must not wait for it.
    fn labels(&self, queue: impl FnOnce() -> bool) -> Vec<String> {
        let stale = self
            .refreshed
            .lock()
            .map_or(true, |t| t.is_none_or(|t| t.elapsed() >= VISIBLE_REFRESH));
        if stale && !self.queued.swap(true, Ordering::SeqCst) && !queue() {
            self.queued.store(false, Ordering::SeqCst);
        }
        self.labels.read().map(|l| l.clone()).unwrap_or_default()
    }

    fn set(&self, labels: Vec<String>) {
        if let Ok(mut l) = self.labels.write() {
            *l = labels;
        }
        if let Ok(mut t) = self.refreshed.lock() {
            *t = Some(Instant::now());
        }
        self.queued.store(false, Ordering::SeqCst);
    }

    /// The next read queues a refresh.
    fn mark_stale(&self) {
        if let Ok(mut t) = self.refreshed.lock() {
            *t = None;
        }
    }
}

/// Reads which windows are visible and records it. On the main thread this runs at once;
/// from any other thread it is queued there and this returns without waiting.
pub fn refresh_visible<R: Runtime>(app: &AppHandle<R>) -> bool {
    let a = app.clone();
    app.run_on_main_thread(move || {
        if let Some(v) = a.try_state::<Arc<VisibleWindows>>() {
            v.set(
                a.webview_windows()
                    .into_iter()
                    .filter(|(_, w)| w.is_visible().unwrap_or(true))
                    .map(|(label, _)| label)
                    .collect(),
            );
        }
    })
    .is_ok()
}

/// A window was focused, resized, minimised, closed or destroyed: refresh on the next event.
/// (Runs inside Tauri's window event handler, so it only marks the list stale.)
pub fn window_changed<R: Runtime>(app: &AppHandle<R>) {
    if let Some(v) = app.try_state::<Arc<VisibleWindows>>() {
        v.mark_stale();
    }
}

/// Hidden windows still get the engine state this often, so they are current when shown.
const HIDDEN_STATE_EVERY: Duration = Duration::from_secs(1);

fn label_in(t: &EventTarget, labels: &[String]) -> bool {
    match t {
        EventTarget::WebviewWindow { label }
        | EventTarget::Webview { label }
        | EventTarget::Window { label }
        | EventTarget::AnyLabel { label } => labels.contains(label),
        _ => false,
    }
}

fn not_toasts(t: &EventTarget) -> bool {
    !label_in(t, &[crate::floating::TOASTS.to_string()])
}

/// Sends the shell's events to the webviews as their typed Tauri events. Every window
/// is its own WebKit process, and re-rendering hidden ones at the meter rate cost more
/// CPU than the recording, so (with `lib/engine.ts` listening per window) levels go only
/// to visible windows, and progress-only state updates reach hidden windows once a second.
/// Status changes always go to every window. Never waits on the main thread (see
/// `VisibleWindows`): it runs on engine threads, which the main thread waits for at quit.
pub fn emitter<R: Runtime>(app: AppHandle<R>) -> Emit {
    let visible = Arc::new(VisibleWindows::default());
    app.manage(visible.clone());
    let last_all = Mutex::new((None::<Instant>, None::<EngineStatus>));
    Arc::new(move |o: Outbound| {
        let r = match o {
            Outbound::State(s) => {
                let all = last_all.lock().map_or(true, |mut l| {
                    let all = l.1 != Some(s.status)
                        || l.0.is_none_or(|t| t.elapsed() >= HIDDEN_STATE_EVERY);
                    if all {
                        *l = (Some(Instant::now()), Some(s.status));
                    }
                    all
                });
                if all {
                    EngineStateEvent(s).emit(&app)
                } else {
                    let labels = visible.labels(|| refresh_visible(&app));
                    EngineStateEvent(s).emit_filter(&app, |t| label_in(t, &labels))
                }
            }
            Outbound::Levels(l) => {
                let labels = visible.labels(|| refresh_visible(&app));
                LevelsEvent(l).emit_filter(&app, |t| label_in(t, &labels))
            }
            // The toasts window gets these through `floating::observer`, which builds it on
            // demand and holds them until it listens.
            Outbound::RecordingFinished(r) => {
                RecordingFinishedEvent(r).emit_filter(&app, not_toasts)
            }
            Outbound::ReplaySaved(path) => ReplaySavedEvent { path }.emit_filter(&app, not_toasts),
            Outbound::Error {
                message,
                recoverable,
            } => EngineErrorEvent {
                message,
                recoverable,
            }
            .emit_filter(&app, not_toasts),
            Outbound::Settings(s) => SettingsChangedEvent(s).emit(&app),
        };
        if let Err(err) = r {
            eprintln!("cortex: failed to emit an event: {err}");
        }
    })
}

type Shared<'a> = State<'a, Arc<Shell>>;

/// Run a call that can block (ScreenCaptureKit, prompts, Keychain) off the IPC thread.
async fn blocking<T: Send + 'static>(
    shell: &Shared<'_>,
    f: impl FnOnce(&Shell) -> Result<T, EngineError> + Send + 'static,
) -> Result<T, EngineError> {
    let s = shell.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&s))
        .await
        .map_err(|e| EngineError::failed(format!("The recorder stopped responding: {e}")))?
}

#[tauri::command]
#[specta::specta]
fn engine_state(shell: Shared<'_>) -> EngineState {
    shell.state()
}

/// Start recording with the stored settings, after their countdown.
#[tauri::command]
#[specta::specta]
async fn start(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::start).await
}

#[tauri::command]
#[specta::specta]
async fn cancel_countdown(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::cancel_countdown).await
}

/// Stop and keep the file; `engine://recording-finished` follows.
#[tauri::command]
#[specta::specta]
async fn stop(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::stop).await
}

#[tauri::command]
#[specta::specta]
async fn pause(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::pause).await
}

#[tauri::command]
#[specta::specta]
async fn resume(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::resume).await
}

/// Stop without keeping the file (or cancel the countdown).
#[tauri::command]
#[specta::specta]
async fn discard(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::discard).await
}

#[tauri::command]
#[specta::specta]
async fn save_replay(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::save_replay).await
}

#[tauri::command]
#[specta::specta]
async fn go_live(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::go_live).await
}

#[tauri::command]
#[specta::specta]
async fn stop_live(shell: Shared<'_>) -> Result<(), EngineError> {
    blocking(&shell, Shell::stop_live).await
}

/// Replace the settings (the UI merges its patch first). Validated, persisted, applied;
/// returns what was stored and broadcasts `settings://changed`.
#[tauri::command]
#[specta::specta]
async fn update_settings(
    shell: Shared<'_>,
    settings: RecorderSettings,
) -> Result<RecorderSettings, EngineError> {
    blocking(&shell, move |s| s.update_settings(settings)).await
}

#[tauri::command]
#[specta::specta]
fn get_settings(shell: Shared<'_>) -> RecorderSettings {
    shell.settings()
}

/// Every capture source the engine can see right now.
#[tauri::command]
#[specta::specta]
async fn devices(shell: Shared<'_>) -> Result<Devices, EngineError> {
    blocking(&shell, |s| Ok(s.devices())).await
}

/// Show the system prompt; resolves when answered (or after 60 s).
#[tauri::command]
#[specta::specta]
async fn request_permission(
    shell: Shared<'_>,
    permission: Permission,
) -> Result<PermissionState, EngineError> {
    blocking(&shell, move |s| Ok(s.request_permission(permission))).await
}

/// The privacy grants now, and whether a Screen Recording grant waits for a relaunch.
/// Cheap enough to poll about once a second while a permission step is shown.
#[tauri::command]
#[specta::specta]
async fn check_permissions(shell: Shared<'_>) -> Result<PermissionReport, EngineError> {
    blocking(&shell, |s| Ok(s.permission_report())).await
}

/// Quit and reopen Cortex, so a new Screen Recording grant applies. Refused while recording.
#[tauri::command]
#[specta::specta]
fn relaunch(app: AppHandle, shell: Shared<'_>) -> Result<(), EngineError> {
    shell.can_relaunch()?;
    app.request_restart();
    Ok(())
}

/// Open the System Settings privacy pane for a permission.
#[tauri::command]
#[specta::specta]
fn open_system_settings(permission: Permission) -> Result<(), EngineError> {
    shell::open_system_settings(permission)
}

/// Show a recording in Finder (only files inside the recordings folder).
#[tauri::command]
#[specta::specta]
fn reveal_recording(shell: Shared<'_>, path: String) -> Result<(), EngineError> {
    shell.reveal_recording(&path)
}

#[tauri::command]
#[specta::specta]
fn open_recordings_folder(shell: Shared<'_>) -> Result<(), EngineError> {
    shell.open_recordings_folder()
}

/// Save a destination's stream key in the Keychain. The key never comes back to the
/// webview; the returned settings only say `keyStored`.
#[tauri::command]
#[specta::specta]
async fn set_stream_key(
    shell: Shared<'_>,
    dest: StreamDest,
    key: String,
) -> Result<RecorderSettings, EngineError> {
    blocking(&shell, move |s| s.set_stream_key(dest, key)).await
}

#[tauri::command]
#[specta::specta]
async fn remove_stream_key(
    shell: Shared<'_>,
    dest: StreamDest,
) -> Result<RecorderSettings, EngineError> {
    blocking(&shell, move |s| Ok(s.remove_stream_key(dest))).await
}

/// One-time import of Rec's settings.json as the current settings.
#[tauri::command]
#[specta::specta]
async fn import_rec_settings(shell: Shared<'_>) -> Result<RecorderSettings, EngineError> {
    blocking(&shell, Shell::import_rec_settings).await
}

/// The main window's live preview, if it could be attached (see `preview.rs`).
pub type PreviewSlot = Option<Arc<LivePreview>>;

/// Main window only: where the preview hole is, in CSS px from the webview's top-left,
/// and its corner radius. The native preview view follows it.
#[tauri::command]
#[specta::specta]
fn set_preview_rect(preview: State<'_, PreviewSlot>, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    if let Some(p) = preview.inner() {
        p.set_rect(x, y, w, h, radius);
    }
}

/// Main window only: the page shows (true) or removed (false) the preview hole.
/// The engine previews while this is on and the window is visible.
#[tauri::command]
#[specta::specta]
fn set_preview_enabled(preview: State<'_, PreviewSlot>, enabled: bool) {
    if let Some(p) = preview.inner() {
        p.set_enabled(enabled);
    }
}

/// The command + event registry shared by the app and the bindings exporter.
/// Keep `build.rs` COMMANDS and `capabilities/default.json` in step with this list.
pub fn builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            engine_state,
            start,
            cancel_countdown,
            stop,
            pause,
            resume,
            discard,
            save_replay,
            go_live,
            stop_live,
            update_settings,
            get_settings,
            devices,
            request_permission,
            check_permissions,
            relaunch,
            open_system_settings,
            reveal_recording,
            open_recordings_folder,
            set_stream_key,
            remove_stream_key,
            import_rec_settings,
            set_preview_rect,
            set_preview_enabled,
            crate::floating::fit_floating,
            crate::floating::open_floating,
            crate::floating::hide_floating,
            crate::floating::window_ready,
            crate::floating::push_toast,
            crate::windows::open_window,
        ])
        .events(collect_events![
            EngineStateEvent,
            LevelsEvent,
            RecordingFinishedEvent,
            ReplaySavedEvent,
            EngineErrorEvent,
            SettingsChangedEvent
        ])
        // serde_json writes NaN/Infinity as `null`, so floats default to `number | null`.
        // rec-core clamps every float to a finite value, so the bindings type them `number`.
        .semantic_types(
            specta_typescript::semantic::Configuration::empty().enable_lossless_floats(),
        )
}

#[cfg(test)]
mod tests {
    use specta_typescript::Typescript;

    /// Regenerates `src/ipc/bindings.ts`. Run via `pnpm --filter desktop ipc:bindings`.
    #[test]
    fn export_bindings() {
        super::builder()
            .export(
                Typescript::default().header("// @generated by tauri-specta from apps/desktop/src-tauri/src/ipc.rs. Do not edit.\n"),
                concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings.ts"),
            )
            .expect("failed to export TypeScript bindings");
    }

    use std::sync::Arc;
    use std::sync::mpsc::{self, Sender};
    use std::time::Duration;

    use super::VisibleWindows;

    type Task = Box<dyn FnOnce() + Send>;

    /// The quit hang in miniature. A "main" thread joins a "ticker" thread (as
    /// `MacEngine::shutdown` did at `RunEvent::Exit`) and only afterwards runs the tasks
    /// queued for it, like AppKit's main thread. The ticker calls `query` 20 times, the
    /// way the emitter reads window visibility for each levels event. Returns whether the
    /// main thread got through the join within 3 s.
    fn quits(query: impl Fn(&Sender<Task>) + Send + 'static) -> bool {
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (tx, rx) = mpsc::channel::<Task>();
            let ticker = std::thread::spawn(move || {
                for _ in 0..20 {
                    query(&tx);
                    std::thread::sleep(Duration::from_millis(1));
                }
            });
            let _ = ticker.join();
            let _ = done_tx.send(());
            for task in rx.try_iter() {
                task();
            }
        });
        done_rx.recv_timeout(Duration::from_secs(3)).is_ok()
    }

    /// What the old `visible_labels` did through `WebviewWindow::is_visible`: queue the
    /// question on the main thread and wait for the answer. It deadlocks.
    #[test]
    fn waiting_on_the_main_thread_from_the_ticker_hangs_quit() {
        let ok = quits(|main| {
            let (reply_tx, reply_rx) = mpsc::channel::<bool>();
            let _ = main.send(Box::new(move || {
                let _ = reply_tx.send(true);
            }));
            let _ = reply_rx.recv();
        });
        assert!(!ok, "expected the old pattern to deadlock");
    }

    /// The registry only queues a refresh and returns, so quit gets through, and the
    /// refresh still lands once the main thread runs its queue.
    #[test]
    fn the_visibility_registry_never_waits_on_the_main_thread() {
        let reg = Arc::new(VisibleWindows::default());
        let r = reg.clone();
        let ok = quits(move |main| {
            let r2 = r.clone();
            let _ = r.labels(|| {
                main.send(Box::new(move || r2.set(vec!["main".into()])))
                    .is_ok()
            });
        });
        assert!(ok, "quit hung on the visibility registry");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while reg.labels(|| true).is_empty() {
            assert!(std::time::Instant::now() < deadline, "refresh never landed");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(reg.labels(|| true), vec!["main".to_string()]);
    }

    #[test]
    fn a_stale_registry_queues_one_refresh_at_a_time() {
        let reg = VisibleWindows::default();
        let mut queued = 0;
        for _ in 0..5 {
            let _ = reg.labels(|| {
                queued += 1;
                true
            });
        }
        assert_eq!(queued, 1);
        reg.set(vec!["main".into()]);
        reg.mark_stale();
        let _ = reg.labels(|| {
            queued += 1;
            true
        });
        assert_eq!(queued, 2);
    }
}
