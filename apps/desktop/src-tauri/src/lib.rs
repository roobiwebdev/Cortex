//! Cortex desktop shell: windows, menu bar icon, and the typed IPC layer over
//! the macOS recorder engine (`rec-engine-macos`, driven through `shell::Shell`).

#[cfg(debug_assertions)]
mod debug_live;
mod floating;
mod ipc;
mod preview;
mod quit;
pub mod shell;
mod trim;
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
/// - `CORTEX_DEBUG_WITHHOLD=camera,microphone,screen`: treat those grants as never asked,
///   to exercise the missing-permission paths on a Mac where they are granted.
/// - `CORTEX_DEBUG_HIDE_MAIN=1`: hide the main window at launch (preview off).
/// - `CORTEX_DEBUG_PREVIEW_STATS=1`: print the preview frame rate every 5 s (0 fps when it
///   stalls) and every preview on/off decision with its reason.
/// - `CORTEX_DEBUG_FRONT=1`: keep the main window above other apps and on every Space, so
///   the preview stays visible (and running) while a measurement runs, even when someone
///   at the Mac switches Spaces.
///
/// - `CORTEX_DEBUG_CAPTURABLE=1`: let screenshots see the floating windows (debug builds),
///   and wait 5 s after a debug recording so its toast can be captured.
/// - `CORTEX_DEBUG_OPEN=menubar|region|settings|onboarding|trim`: open that window 2 s after
///   launch (debug builds), to check windows that are built on demand.
/// - `CORTEX_DEBUG_TARGET`: what to capture, written to the settings at launch. Only with
///   `CORTEX_DEBUG_DATA_DIR`, so the user's settings are never touched. One of
///   `display:N`, `window:ID|substring`, `app:BUNDLE_ID`, `region:N,X,Y,W,H` (points).
/// - `CORTEX_DEBUG_LIVE_URL` (+ `_KEY`, `_SECS`): Go live to a local test server through the
///   IPC commands, then quit (debug builds; see `debug_live.rs`).
///
/// Used by `scripts/desktop/perf-compare.sh`.
#[derive(Default)]
struct DebugRun {
    data_dir: Option<std::path::PathBuf>,
    test_pattern: bool,
    record_secs: Option<f64>,
    hide_main: bool,
    withheld: Vec<rec_core::state::Permission>,
    preview_stats: bool,
    front: bool,
    capturable: bool,
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    open: Option<String>,
    target: Option<String>,
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
            withheld: var("CORTEX_DEBUG_WITHHOLD")
                .map(|v| {
                    use rec_core::state::Permission;
                    v.split(',')
                        .filter_map(|p| match p.trim() {
                            "screen" => Some(Permission::Screen),
                            "microphone" | "mic" => Some(Permission::Microphone),
                            "camera" => Some(Permission::Camera),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            preview_stats: var("CORTEX_DEBUG_PREVIEW_STATS").is_some_and(|v| v == "1"),
            front: var("CORTEX_DEBUG_FRONT").is_some_and(|v| v == "1"),
            capturable: var("CORTEX_DEBUG_CAPTURABLE").is_some_and(|v| v == "1"),
            open: var("CORTEX_DEBUG_OPEN"),
            target: var("CORTEX_DEBUG_TARGET"),
        }
    }
}

/// `CORTEX_DEBUG_TARGET` as a capture target. A window is an ID or the largest window
/// whose "app title" contains the text (any case).
fn debug_target(
    spec: &str,
    windows: &[rec_core::state::CaptureWindow],
) -> Option<rec_core::settings::CaptureTarget> {
    use rec_core::settings::CaptureTarget;
    let (kind, rest) = spec.split_once(':')?;
    match kind {
        "display" => Some(CaptureTarget::Display {
            display_id: rest.parse().ok()?,
        }),
        "app" => Some(CaptureTarget::App {
            bundle_id: rest.into(),
        }),
        "window" => {
            let lower = rest.to_lowercase();
            let id = rest.parse::<u32>().ok().or_else(|| {
                windows
                    .iter()
                    .filter(|w| {
                        format!("{} {}", w.app, w.title)
                            .to_lowercase()
                            .contains(&lower)
                    })
                    .max_by(|a, b| (a.frame.w * a.frame.h).total_cmp(&(b.frame.w * b.frame.h)))
                    .map(|w| w.id)
            })?;
            Some(CaptureTarget::Window { window_id: id })
        }
        "region" => {
            let v: Vec<f64> = rest
                .split(',')
                .map(|x| x.trim().parse().ok())
                .collect::<Option<_>>()?;
            let [d, x, y, w, h] = v[..] else { return None };
            Some(CaptureTarget::Region {
                display_id: d as u32,
                x,
                y,
                w,
                h,
            })
        }
        _ => None,
    }
}

/// `CORTEX_DEBUG_OPEN`: opens a window the way the UI would.
#[cfg(debug_assertions)]
fn debug_open(app: &tauri::AppHandle, what: &str) {
    use crate::floating::FloatingSurface;
    use crate::windows::AppWindow;
    let r = match what {
        "menubar" => floating::open_floating(app.clone(), FloatingSurface::Menubar),
        "region" => floating::open_floating(app.clone(), FloatingSurface::Region),
        "settings" => windows::open_window(app.clone(), AppWindow::Settings),
        "onboarding" => windows::open_window(app.clone(), AppWindow::Onboarding),
        "trim" => windows::open_window(app.clone(), AppWindow::Trim),
        _ => Ok(()),
    };
    if let Err(e) = r {
        eprintln!("cortex: CORTEX_DEBUG_OPEN={what} failed: {e}");
    }
}

pub fn run() {
    // A child started by `Shell::permission_report`: answer and exit before any UI starts.
    if std::env::args().nth(1).as_deref() == Some(shell::SCREEN_PREFLIGHT_ARG) {
        let granted = rec_engine::tcc::screen_status() == rec_engine::tcc::Status::Granted;
        std::process::exit(if granted { 0 } else { 1 });
    }
    let ipc = ipc::builder();

    let app = tauri::Builder::default()
        .invoke_handler(ipc.invoke_handler())
        // The Trim window plays recordings from here (see `trim::media_protocol`).
        .register_asynchronous_uri_scheme_protocol("cortex-media", trim::media_protocol)
        // Window state read from engine threads comes from this cache (see `ipc::VisibleWindows`).
        .on_window_event(|w, _| ipc::window_changed(w.app_handle()))
        .setup(move |app| {
            ipc.mount_events(app);
            app.manage(quit::Quit::default());
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
                withheld: debug.withheld.clone(),
            };
            // Our windows must never show up in a screen recording: fail instead.
            rec_engine::screen::require_self_exclusion(true);
            // Every outbound event also drives the floating windows (countdown, HUD).
            let floating = floating::Floating::new();
            let to_webviews = ipc::emitter(app.handle().clone());
            let to_windows = floating::observer(app.handle().clone(), floating.clone());
            let to_library = trim::observer(app.handle().clone());
            let fan_out = move |o: Outbound| {
                to_library(&o);
                to_windows(&o);
                to_webviews(o);
            };
            let emit: shell::Emit = if debug.record_secs.is_some() {
                let handle = app.handle().clone();
                // Screenshot runs keep the app up a little so the "saved" toast shows.
                let linger = debug.capturable && cfg!(debug_assertions);
                Arc::new(move |o: Outbound| {
                    let finished = matches!(&o, Outbound::RecordingFinished(_));
                    if let Outbound::RecordingFinished(r) = &o {
                        println!("cortex: recorded {}", r.path);
                    }
                    fan_out(o);
                    if finished && linger {
                        let h = handle.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            h.exit(0);
                        });
                    } else if finished {
                        handle.exit(0);
                    }
                })
            } else {
                Arc::new(fan_out)
            };
            // ~/Library/Caches/dev.roobi.cortex (timeline thumbnails and waveforms).
            let cache_dir = match &debug.data_dir {
                Some(d) => d.join("Caches"),
                None => app.path().app_cache_dir()?,
            };
            app.manage(Arc::new(trim::Library::new(cache_dir, emit.clone())));
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
            trim::rescan_later(app.handle());
            windows::setup(app)?;
            let live = match preview::LivePreview::attach(app, shell.clone(), debug.preview_stats) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("cortex: live preview unavailable: {e}");
                    None
                }
            };
            app.manage::<ipc::PreviewSlot>(live);
            if debug.preview_stats {
                // What the UI's device menus and permission affordances will show.
                let s = shell.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let st = s.state();
                    let names = |d: &[rec_core::state::InputDevice]| {
                        d.iter()
                            .map(|x| x.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    eprintln!(
                        "cortex: devices: cameras [{}], mics [{}]; permissions {:?}",
                        names(&st.cameras),
                        names(&st.mics),
                        st.permissions
                    );
                });
            }
            if let Some(main) = app.get_webview_window("main") {
                if debug.hide_main {
                    let _ = main.hide();
                } else if debug.front {
                    let _ = main.set_always_on_top(true);
                    // A Space switch moves an ordinary window off screen, which rightly stops
                    // the preview and spoiled idle perf runs. Follow the user to every Space,
                    // full-screen ones included.
                    rec_macos_ui::set_window_on_all_spaces(&main);
                    let _ = main.show();
                }
            }
            #[cfg(debug_assertions)]
            debug_live::maybe_start(app.handle(), shell.clone());
            if debug.record_secs.is_some() {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Err(e) = shell.start() {
                        eprintln!("cortex: debug record failed: {e}");
                        std::process::exit(2);
                    }
                });
            }
            // Debug builds only: release builds keep every floating window out of captures.
            let capturable = debug.capturable && cfg!(debug_assertions);
            floating::init(app, floating, &settings, capturable);
            // After the floating windows are set up: a settings change already drives them.
            // Only into a scratch settings folder: never the user's own settings.
            if let (Some(spec), Some(_)) = (&debug.target, &debug.data_dir) {
                let shell = app.state::<Arc<Shell>>();
                let devices = shell.devices();
                match debug_target(spec, &devices.windows) {
                    Some(target) => {
                        let s = rec_core::RecorderSettings {
                            target,
                            ..shell.settings()
                        };
                        match shell.update_settings(s) {
                            Ok(s) => eprintln!("cortex: debug target {:?}", s.target),
                            Err(e) => eprintln!("cortex: debug target refused: {e}"),
                        }
                    }
                    None => eprintln!("cortex: CORTEX_DEBUG_TARGET={spec} not understood"),
                }
            }
            // On the main thread, so this runs now: levels reach the main window at once.
            ipc::refresh_visible(app.handle());
            quit::watch_signals(app.handle());
            #[cfg(debug_assertions)]
            if let Some(what) = debug.open.clone() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let h = handle.clone();
                    let _ = handle.run_on_main_thread(move || debug_open(&h, &what));
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Cortex app");

    app.run(|app, event| match event {
        // Quit finalizes a running recording and writes pending settings first, off the
        // main thread (see `quit.rs`).
        RunEvent::ExitRequested { code, api, .. } => quit::on_exit_requested(app, code, &api),
        // ⌘Q from the app menu arrives here without `ExitRequested`. Already done if the
        // quit thread ran; otherwise it finalizes now (nothing it does waits on this thread).
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

#[cfg(test)]
mod tests {
    use rec_core::settings::CaptureTarget;
    use rec_core::state::{CaptureWindow, Rect};

    #[test]
    fn debug_targets_parse() {
        let w = |id, app: &str, w| CaptureWindow {
            id,
            app: app.into(),
            title: "t".into(),
            bundle_id: String::new(),
            display_id: 1,
            frame: Rect {
                x: 0.0,
                y: 0.0,
                w,
                h: 100.0,
            },
        };
        let ws = [
            w(5, "Finder", 100.0),
            w(6, "Finder", 300.0),
            w(7, "Safari", 900.0),
        ];
        let t = |s| super::debug_target(s, &ws);
        assert_eq!(
            t("window:finder"),
            Some(CaptureTarget::Window { window_id: 6 })
        );
        assert_eq!(
            t("window:42"),
            Some(CaptureTarget::Window { window_id: 42 })
        );
        assert_eq!(
            t("display:2"),
            Some(CaptureTarget::Display { display_id: 2 })
        );
        assert_eq!(
            t("region:1,10,20,640,360"),
            Some(CaptureTarget::Region {
                display_id: 1,
                x: 10.0,
                y: 20.0,
                w: 640.0,
                h: 360.0
            })
        );
        assert_eq!(
            t("app:com.apple.finder"),
            Some(CaptureTarget::App {
                bundle_id: "com.apple.finder".into()
            })
        );
        assert_eq!(t("region:1,2"), None);
        assert_eq!(t("nope"), None);
    }
}
