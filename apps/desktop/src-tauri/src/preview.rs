//! SOURCE OF TRUTH KEYWORDS: LivePreview, live preview, PreviewView, set_preview_rect, set_preview_enabled, transparent hole, occlusion
//! WHAT:  The main window's live preview: the platform's native view under the (transparent)
//!        webview (`rec_macos_ui::PreviewView` on macOS), fed straight from the engine's
//!        preview sink (`platform::DesktopEngine::attach_preview`).
//! WHY:   Video frames never enter the webview. React leaves a transparent hole and reports
//!        its rect; this module places the native view there and runs the engine's preview
//!        session only while the hole exists and the window can be seen.
//! WHERE: `lib.rs` attaches it at setup; `ipc.rs` exposes `set_preview_rect` and
//!        `set_preview_enabled` to the main window only (`capabilities/preview.json`).

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use tauri::{App, Manager, WebviewWindow};

use crate::platform::PreviewSurface;
use crate::shell::Shell;

const MAIN: &str = "main";

/// What decides whether the preview runs.
#[derive(Default)]
struct Wanted {
    /// React shows the hole (main window on the preview screen, capture allowed).
    hole: bool,
    /// The window is on screen: not hidden, minimised, covered or on another Space.
    visible: bool,
    /// What the engine was last told.
    on: bool,
}

pub struct LivePreview {
    view: Box<dyn PreviewSurface>,
    window: WebviewWindow,
    shell: Arc<Shell>,
    wanted: Mutex<Wanted>,
    /// Engine preview on/off requests, run in order on one worker thread
    /// (starting a session can block on ScreenCaptureKit).
    engine_tx: Sender<bool>,
    /// Print every on/off decision and its reason (`CORTEX_DEBUG_PREVIEW_STATS=1`).
    trace: bool,
}

impl LivePreview {
    /// Attaches the native view to the main window and makes its webview transparent.
    /// Call from `setup` (main thread), after the shell is managed.
    /// `stats`: print the preview frame rate every 5 s (perf runs).
    pub fn attach(app: &App, shell: Arc<Shell>, stats: bool) -> Result<Arc<LivePreview>, String> {
        let window = app
            .get_webview_window(MAIN)
            .ok_or_else(|| "no main window".to_string())?;
        let view = shell.engine().attach_preview(&window, stats)?;

        let (engine_tx, rx) = mpsc::channel::<bool>();
        let worker_shell = shell.clone();
        std::thread::Builder::new()
            .name("preview-toggle".into())
            .spawn(move || {
                while let Ok(mut on) = rx.recv() {
                    // Only the latest request matters.
                    while let Ok(next) = rx.try_recv() {
                        on = next;
                    }
                    let r = worker_shell.engine().set_preview_enabled(on);
                    if stats {
                        eprintln!("cortex: preview engine on={on} -> {r:?}");
                    }
                    if let Err(e) = r {
                        eprintln!("cortex: preview: {e}");
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        let preview = Arc::new(LivePreview {
            view,
            window,
            shell,
            wanted: Mutex::new(Wanted::default()),
            engine_tx,
            trace: stats,
        });
        let p = preview.clone();
        preview.view.on_window_visibility(Box::new(move |visible| {
            if p.trace {
                // Main thread here, so the window getters do not block.
                eprintln!(
                    "cortex: preview window: shown={:?} minimized={:?} focused={:?}",
                    p.window.is_visible().ok(),
                    p.window.is_minimized().ok(),
                    p.window.is_focused().ok()
                );
            }
            p.update("occlusion", |w| w.visible = visible);
        }));
        Ok(preview)
    }

    /// Places the preview over the hole: CSS px from the webview's top-left, corner radius in points.
    pub fn set_rect(&self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        let finite = [x, y, w, h, radius].iter().all(|v| v.is_finite());
        if !finite || w < 0.0 || h < 0.0 {
            return;
        }
        self.view.set_frame(x, y, w, h);
        self.view.set_corner_radius(radius);
        // Frames arrive scaled to the view's pixel size (the engine caps it at 1920 wide).
        let scale = self.window.scale_factor().unwrap_or(2.0);
        let px = |v: f64| (v * scale).round().clamp(0.0, 16384.0) as u32;
        self.shell.engine().set_preview_pixel_size(px(w), px(h));
    }

    /// React mounted (true) or removed (false) the hole.
    pub fn set_enabled(&self, enabled: bool) {
        self.update("hole", |w| w.hole = enabled);
    }

    fn update(&self, why: &str, f: impl FnOnce(&mut Wanted)) {
        let Ok(mut w) = self.wanted.lock() else {
            return;
        };
        f(&mut w);
        let on = should_preview(w.hole, w.visible);
        if self.trace {
            eprintln!(
                "cortex: preview {why}: hole={} visible={} -> on={on} (was {})",
                w.hole, w.visible, w.on
            );
        }
        if on == w.on {
            return;
        }
        w.on = on;
        if on {
            self.view.show();
        } else {
            self.view.hide();
            self.view.clear();
        }
        // The engine keeps capturing for replay or streaming even when this is false.
        let _ = self.engine_tx.send(on);
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: should_preview, preview decision, occlusion, hole, visible, preview on off
 * WHAT:  Whether the engine's preview session should run: the page shows the hole and
 *        the window is visible (its occlusion state: not hidden, minimised, fully covered
 *        or on another Space).
 * WHY:   Nobody sees the preview otherwise, and it costs about 14% CPU. Recording, replay
 *        and streaming are not inputs: the engine keeps capturing for them on its own and
 *        feeds this view from the recording session (`sync_preview` in rec-engine-macos).
 * WHERE: `LivePreview::update`.
 */
fn should_preview(hole: bool, visible: bool) -> bool {
    hole && visible
}

#[cfg(test)]
mod tests {
    use super::should_preview;

    #[test]
    fn preview_runs_only_with_hole_and_visible_window() {
        // (hole, visible) -> on. "visible" is false when hidden, minimised, covered or on another Space.
        for (hole, visible, on) in [
            (true, true, true),
            (true, false, false),
            (false, true, false),
            (false, false, false),
        ] {
            assert_eq!(
                should_preview(hole, visible),
                on,
                "hole={hole} visible={visible}"
            );
        }
    }
}
