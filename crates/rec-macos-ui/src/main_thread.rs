//! Main-thread dispatch. AppKit objects may only be touched on the main
//! thread; Tauri commands and engine callbacks run elsewhere.

use dispatch2::DispatchQueue;
pub use dispatch2::{MainThreadBound, run_on_main};
use objc2::MainThreadMarker;

/**
 * SOURCE OF TRUTH KEYWORDS: on_main, main thread, dispatch_async, MainThreadMarker, AppKit thread
 * WHAT:  Runs `f` on the main thread: inline when already there, otherwise
 *        queued with dispatch_async on the main queue (fire and forget).
 * WHY:   Setters (frame, radius, visibility) must never block the caller: a
 *        synchronous hop can deadlock when the main thread waits on the caller.
 *        Use `run_on_main` (re-exported) only when a result is required.
 * WHERE: PreviewView setters, panel helpers called from Tauri commands.
 */
pub fn on_main<F>(f: F)
where
    F: FnOnce(MainThreadMarker) + Send + 'static,
{
    if let Some(mtm) = MainThreadMarker::new() {
        f(mtm);
    } else {
        DispatchQueue::main().exec_async(move || {
            // SAFETY: blocks submitted to the main dispatch queue run on the main thread.
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            f(mtm);
        });
    }
}
