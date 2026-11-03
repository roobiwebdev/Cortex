//! Native macOS UI pieces for the Cortex desktop recorder.
//!
//! - [`preview`]: the live preview view under the Tauri webview.
//! - [`panel`]: floating-window behaviour for the HUD, toasts, countdown,
//!   region picker and menu bar panel.
//! - [`geometry`]: pure coordinate maths shared with the web layout.
//! - [`signals`]: SIGTERM/SIGINT as a callback, so the app can quit gracefully.
//!
//! All `unsafe` in the desktop app's UI layer lives in this crate, each block
//! with a SAFETY comment. AppKit calls run on the main thread via
//! [`main_thread::on_main`] / [`main_thread::run_on_main`].
#![cfg(target_os = "macos")]

pub mod geometry;
pub mod main_thread;
pub mod panel;
pub mod preview;
pub mod signals;

pub use geometry::{Rect, aspect_fit, web_rect_to_appkit};
pub use panel::{
    FloatingKind, NonActivation, configure_floating, configure_floating_window,
    invalidate_window_shadow, set_ignores_mouse_events, set_window_capturable,
    set_window_ignores_mouse_events, set_window_on_all_spaces,
};
pub use preview::{
    AttachError, PreviewFn, PreviewView, frame_size, make_webview_transparent,
    make_window_webviews_transparent,
};
