//! SOURCE OF TRUTH KEYWORDS: LivePreview, live preview, PreviewView, set_preview_rect, set_preview_enabled, transparent hole, occlusion
//! WHAT:  The main window's live preview: a native `rec_macos_ui::PreviewView` under the
//!        (transparent) webview, fed straight from `MacEngine`'s preview sink.
//! WHY:   Video frames never enter the webview. React leaves a transparent hole and reports
//!        its rect; this module places the native view there and runs the engine's preview
//!        session only while the hole exists and the window can be seen.
//! WHERE: `lib.rs` attaches it at setup; `ipc.rs` exposes `set_preview_rect` and
//!        `set_preview_enabled` to the main window only (`capabilities/preview.json`).

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use rec_core::api::RecorderEngine;
use rec_macos_ui::{PreviewView, make_window_webviews_transparent};
use tauri::{App, Manager, WebviewWindow};

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
    view: PreviewView,
    window: WebviewWindow,
    shell: Arc<Shell>,
    wanted: Mutex<Wanted>,
    /// Engine preview on/off requests, run in order on one worker thread
    /// (starting a session can block on ScreenCaptureKit).
    engine_tx: Sender<bool>,
}

impl LivePreview {
    /// Attaches the native view to the main window and makes its webview transparent.
    /// Call from `setup` (main thread), after the shell is managed.
    /// `stats`: print the preview frame rate every 5 s (perf runs).
    pub fn attach(app: &App, shell: Arc<Shell>, stats: bool) -> Result<Arc<LivePreview>, String> {
        let window = app
            .get_webview_window(MAIN)
            .ok_or_else(|| "no main window".to_string())?;
        let view = PreviewView::attach_to(&window).map_err(|e| e.to_string())?;
        if make_window_webviews_transparent(&window) == 0 {
            eprintln!("cortex: could not make the main webview transparent");
        }
        let sink = view.sink();
        let sink = if stats { counted(sink) } else { sink };
        shell.engine().set_preview_sink(Some(sink));

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
                    if let Err(e) = worker_shell.engine().set_preview_enabled(on) {
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
        });
        let p = preview.clone();
        preview
            .view
            .on_window_visibility(move |visible| p.update(|w| w.visible = visible));
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
        self.update(|w| w.hole = enabled);
    }

    fn update(&self, f: impl FnOnce(&mut Wanted)) {
        let Ok(mut w) = self.wanted.lock() else {
            return;
        };
        f(&mut w);
        let on = w.hole && w.visible;
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

/// Wraps a sink to print how many frames a second reach the view, and their size.
fn counted(sink: rec_macos_ui::PreviewFn) -> rec_macos_ui::PreviewFn {
    let window = Mutex::new((std::time::Instant::now(), 0u32));
    Arc::new(move |pb| {
        let (w, h) = rec_macos_ui::frame_size(&pb);
        if let Ok(mut c) = window.lock() {
            c.1 += 1;
            let secs = c.0.elapsed().as_secs_f64();
            if secs >= 5.0 {
                println!("cortex: preview {:.1} fps {w}x{h}", f64::from(c.1) / secs);
                *c = (std::time::Instant::now(), 0);
            }
        }
        sink(pb)
    })
}
