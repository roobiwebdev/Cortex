//! Native macOS UI pieces for the Cortex desktop recorder.
//!
//! - [`preview`]: the live preview view under the Tauri webview.
//! - [`panel`]: floating-window behaviour for the HUD, toasts, countdown,
//!   region picker and menu bar panel.
//! - [`geometry`]: pure coordinate maths shared with the web layout.
//!
//! All `unsafe` in the desktop app's UI layer lives in this crate, each block
//! with a SAFETY comment. AppKit calls run on the main thread via
//! [`main_thread::on_main`] / [`main_thread::run_on_main`].
#![cfg(target_os = "macos")]

pub mod geometry;
pub mod main_thread;
pub mod panel;
pub mod preview;

pub use geometry::{Rect, aspect_fit, web_rect_to_appkit};
pub use panel::{FloatingKind, NonActivation, configure_floating, set_ignores_mouse_events};
pub use preview::{
    AttachError, PreviewFn, PreviewView, make_webview_transparent, make_window_webviews_transparent,
};
