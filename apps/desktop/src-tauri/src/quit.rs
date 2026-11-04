//! SOURCE OF TRUTH KEYWORDS: quit, ExitRequested, graceful exit, shutdown thread, SIGTERM, finalize on quit
//! WHAT:  The quit flow. An exit request (tray Quit, `AppHandle::exit`, SIGTERM/SIGINT, the
//!        debug recording run) is held back while `Shell::shutdown` finalizes the recording
//!        and flushes settings on a background thread; that thread then exits for real.
//! WHY:   Finalizing can take seconds. Doing it on the main thread froze the UI and made
//!        any main-thread wait elsewhere a deadlock (the quit hang). Here the event loop
//!        keeps running until the file is written. ⌘Q from the app menu skips
//!        `ExitRequested` (AppKit terminates directly), so `RunEvent::Exit` still runs the
//!        same shutdown on the main thread, which is safe because event fan-out never waits
//!        on the main thread (`ipc::VisibleWindows`).
//! WHERE: `lib.rs` run loop (`RunEvent::ExitRequested`) and setup (`watch_signals`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use tauri::{AppHandle, ExitRequestApi, Manager};

use crate::shell::Shell;

const RUNNING: u8 = 0;
const SHUTTING_DOWN: u8 = 1;
const DONE: u8 = 2;

/// Where the quit flow is. Managed state, one per app.
#[derive(Default)]
pub struct Quit(AtomicU8);

/// Handles `RunEvent::ExitRequested`: the first request starts the shutdown thread and is
/// prevented; later ones are prevented until that thread has finished and exits itself.
pub fn on_exit_requested(app: &AppHandle, code: Option<i32>, api: &ExitRequestApi) {
    let Some(quit) = app.try_state::<Quit>() else {
        return;
    };
    match quit
        .0
        .compare_exchange(RUNNING, SHUTTING_DOWN, Ordering::SeqCst, Ordering::SeqCst)
    {
        Ok(_) => {}
        Err(DONE) => return,
        Err(_) => {
            api.prevent_exit();
            return;
        }
    }
    let Some(shell) = app.try_state::<Arc<Shell>>().map(|s| s.inner().clone()) else {
        quit.0.store(DONE, Ordering::SeqCst);
        return;
    };
    api.prevent_exit();
    let h = app.clone();
    let code = code.unwrap_or(0);
    let spawned = std::thread::Builder::new()
        .name("quit".into())
        .spawn(move || {
            shell.shutdown();
            if let Some(q) = h.try_state::<Quit>() {
                q.0.store(DONE, Ordering::SeqCst);
            }
            h.exit(code);
        });
    if let Err(e) = spawned {
        // No thread: let this exit through; `RunEvent::Exit` finalizes on the main thread.
        log_line!("could not start the quit thread: {e}");
        quit.0.store(DONE, Ordering::SeqCst);
        app.exit(code);
    }
}

/// SIGTERM and SIGINT quit the same way as the tray's Quit item (the file is finalized),
/// instead of killing the process mid-write.
pub fn watch_signals(app: &AppHandle) {
    let h = app.clone();
    if let Err(e) = crate::platform::system().on_terminate(Box::new(move || h.exit(0))) {
        log_line!("could not watch for SIGTERM: {e}");
    }
}
