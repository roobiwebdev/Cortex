//! The Cortex recorder engine for Windows: Windows.Graphics.Capture for displays, windows
//! and regions, Direct3D 11 for cropping, the camera overlay and the BGRA→NV12 scale,
//! Media Foundation's sink writer for H.264/HEVC + AAC in MP4, and WASAPI for system audio
//! (loopback) and microphones. It implements `rec_core::api::RecorderEngine`, so the Tauri
//! shell drives it exactly as it drives `MacEngine`.
//!
//! The crate builds on every target. Its pure parts (timeline and pacing, audio mixing and
//! resampling, sizing and encoder settings, the overlay renderer, the engine state machine)
//! are unit-tested anywhere; the Windows API code is in `win/` behind `cfg(windows)`.
//! See `docs/engineering/platforms.md` for what is built, verified and still open.

pub mod audio;
pub mod caps;
pub mod devices;
pub mod engine;
pub mod overlay;
pub mod pattern;
pub mod plan;
pub mod timeline;

#[cfg(windows)]
pub mod win;

/// The Windows engine: the shared state machine over the Windows backend.
#[cfg(windows)]
pub type WindowsEngine = engine::Engine<win::WinBackend>;
