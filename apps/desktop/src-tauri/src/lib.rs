//! Cortex desktop shell: windows, menu bar icon, and the typed IPC layer over
//! the macOS recorder engine (`rec-engine-macos`, driven through `shell::Shell`).

mod floating;
mod ipc;
mod preview;
pub mod shell;
mod windows;

use std::sync::Arc;

use rec_engine::engine::EngineOptions;
use tauri::{Manager, RunEvent};

use crate::shell::{Outbound, Shell};

/// Perf and self-test hooks, read once at startup (never set in normal use):
/// - `CORTEX_DEBUG_DATA_DIR`: settings folder instead of Application Support.
/// - `CORTEX_DEBUG_TEST_PATTERN=1`: record Rec's test pattern (no Screen Recording grant).
/// - `CORTEX_DEBUG_RECORD_SECS=N`: start recording 3 s after launch, stop after N recorded
///   seconds, print `cortex: recorded <path>` and quit.
/// - `CORTEX_DEBUG_HIDE_MAIN=1`: hide the main window at launch (preview off).
/// - `CORTEX_DEBUG_PREVIEW_STATS=1`: print the preview frame rate every 5 s.
/// - `CORTEX_DEBUG_FRONT=1`: keep the main window above other apps, so the preview
///   stays visible (and running) while a measurement runs.
///
/// - `CORTEX_DEBUG_CAPTURABLE=1`: let screenshots see the floating windows (debug builds).
///
/// Used by `scripts/desktop/perf-compare.sh`.
#[derive(Default)]
struct DebugRun {
    data_dir: Option<std::path::PathBuf>,
    test_pattern: bool,
    record_secs: Option<f64>,
    hide_main: bool,
    preview_stats: bool,
    front: bool,
    capturable: bool,
}

impl DebugRun {
    fn from_env() -> DebugRun {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        DebugRun {
            data_dir: var("CORTEX_DEBUG_DATA_DIR").map(Into::into),
            test_pattern: var("CORTEX_DEBUG_TEST_PATTERN").is_some_and(|v| v == "1"),
            record_secs: var("CORTEX_DEBUG_RECORD_SECS")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0),
            hide_main: var("CORTEX_DEBUG_HIDE_MAIN").is_some_and(|v| v == "1"),
            preview_stats: var("CORTEX_DEBUG_PREVIEW_STATS").is_some_and(|v| v == "1"),
            front: var("CORTEX_DEBUG_FRONT").is_some_and(|v| v == "1"),
            capturable: var("CORTEX_DEBUG_CAPTURABLE").is_some_and(|v| v == "1"),
        }
    }
}

pub fn run() {
    let ipc = ipc::builder();

    let app = tauri::Builder::default()
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            let debug = DebugRun::from_env();
            // ~/Library/Application Support/dev.roobi.cortex/settings.json
            let data_dir = match debug.data_dir.clone() {
                Some(d) => d,
                None => app.path().app_data_dir()?,
            };
            let settings_path = data_dir.join("settings.json");
            let opts = EngineOptions {
                home: app.path().home_dir()?,
                test_pattern: debug.test_pattern,
                max_duration_secs: debug.record_secs,
            };
            // Our windows must never show up in a screen recording: fail instead.
            rec_engine::screen::require_self_exclusion(true);
            // Every outbound event also drives the floating windows (countdown, HUD).
            let floating = floating::Floating::new();
            let to_webviews = ipc::emitter(app.handle().clone());
            let to_windows = floating::observer(app.handle().clone(), floating.clone());
            let fan_out = move |o: Outbound| {
                to_windows(&o);
                to_webviews(o);
            };
            let emit: shell::Emit = if debug.record_secs.is_some() {
                let handle = app.handle().clone();
                Arc::new(move |o: Outbound| {
                    if let Outbound::RecordingFinished(r) = &o {
                        println!("cortex: recorded {}", r.path);
                        handle.exit(0);
                    }
                    fan_out(o)
                })
            } else {
                Arc::new(fan_out)
            };
            let shell = Shell::open(opts, settings_path, emit.clone());
            if let Some(message) = &shell.load_error {
                eprintln!("cortex: {message}");
                emit(Outbound::Error {
                    message: message.clone(),
                    recoverable: true,
                });
            }
            // On the main thread, as the device notifications require.
            shell.engine().watch_devices();
            let settings = shell.settings();
            let shell = Arc::new(shell);
            app.manage(shell.clone());
            windows::setup(app)?;
            let live = match preview::LivePreview::attach(app, shell.clone(), debug.preview_stats) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("cortex: live preview unavailable: {e}");
                    None
                }
            };
            app.manage::<ipc::PreviewSlot>(live);
            if let Some(main) = app.get_webview_window("main") {
                if debug.hide_main {
                    let _ = main.hide();
                } else if debug.front {
                    let _ = main.set_always_on_top(true);
                    let _ = main.show();
                }
            }
            if debug.record_secs.is_some() {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Err(e) = shell.start() {
                        eprintln!("cortex: debug record failed: {e}");
                        std::process::exit(2);
                    }
                });
            }
            floating::init(app, floating, &settings);
            #[cfg(debug_assertions)]
            if debug.capturable {
                floating::make_capturable(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Cortex app");

    app.run(|app, event| match event {
        // Quit finalizes a running recording and writes pending settings first.
        RunEvent::Exit => {
            if let Some(shell) = app.try_state::<Arc<Shell>>() {
                shell.shutdown();
            }
            floating::shutdown(app);
        }
        // Clicking the Dock icon brings the main window back after it was closed.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => windows::show_main(app),
        _ => {}
    });
}
