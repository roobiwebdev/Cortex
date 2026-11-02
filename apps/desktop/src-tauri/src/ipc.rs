//! SOURCE OF TRUTH KEYWORDS: IPC, tauri commands, tauri-specta, bindings.ts, engine://state, engine://levels
//! WHAT:  The single IPC checkpoint (engineering plan §6): every command the webview can call
//!        and every event it can hear. Commands are thin wrappers over `shell::Shell`; all
//!        types come from `rec-core`, so the TypeScript bindings generated from this module
//!        (`apps/desktop/src/ipc/bindings.ts`, `pnpm --filter desktop ipc:bindings`) are the
//!        UI's source of truth.
//! WHY:   One allow-listed surface: windows may only call commands their capability grants
//!        (see `build.rs` and `capabilities/default.json`). Stream keys go in, never out.
//! WHERE: `lib.rs` registers `builder()`; `Shell` emits through `emitter()`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rec_core::api::EngineError;
use rec_core::settings::{RecorderSettings, StreamDest};
use rec_core::state::{
    Devices, EngineState, EngineStatus, Levels, Permission, PermissionState, Recording,
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

/// How long the list of visible windows is reused for level events.
const VISIBLE_REFRESH: Duration = Duration::from_millis(250);

/// Labels of the windows on screen, refreshed at most every `VISIBLE_REFRESH`.
fn visible_labels<R: Runtime>(
    app: &AppHandle<R>,
    cache: &Mutex<(Option<Instant>, Vec<String>)>,
) -> Vec<String> {
    let Ok(mut c) = cache.lock() else {
        return vec![];
    };
    if c.0.is_none_or(|t| t.elapsed() >= VISIBLE_REFRESH) {
        c.1 = app
            .webview_windows()
            .into_iter()
            .filter(|(_, w)| w.is_visible().unwrap_or(true))
            .map(|(label, _)| label)
            .collect();
        c.0 = Some(Instant::now());
    }
    c.1.clone()
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

/// Sends the shell's events to the webviews as their typed Tauri events. Every window
/// is its own WebKit process, and re-rendering hidden ones at the meter rate cost more
/// CPU than the recording, so (with `lib/engine.ts` listening per window) levels go only
/// to visible windows, and progress-only state updates reach hidden windows once a second.
/// Status changes always go to every window.
pub fn emitter<R: Runtime>(app: AppHandle<R>) -> Emit {
    let visible = Mutex::new((None, vec![]));
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
                    let labels = visible_labels(&app, &visible);
                    EngineStateEvent(s).emit_filter(&app, |t| label_in(t, &labels))
                }
            }
            Outbound::Levels(l) => {
                let labels = visible_labels(&app, &visible);
                LevelsEvent(l).emit_filter(&app, |t| label_in(t, &labels))
            }
            Outbound::RecordingFinished(r) => RecordingFinishedEvent(r).emit(&app),
            Outbound::ReplaySaved(path) => ReplaySavedEvent { path }.emit(&app),
            Outbound::Error {
                message,
                recoverable,
            } => EngineErrorEvent {
                message,
                recoverable,
            }
            .emit(&app),
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
}
