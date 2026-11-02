//! Capture, encode and stream engine for macOS (ScreenCaptureKit +
//! VideoToolbox + AVAssetWriter). The CLI and the GUI live in `rec-cli`.

pub mod args;
pub mod camera;
pub mod compose;
pub mod engine;
pub mod hotkey;
pub mod live;
pub mod mixer;
pub mod outlatency;
pub mod play;
pub mod probe;
pub mod record;
pub mod scale;
pub mod screen;
pub mod settings;
pub mod synth;
pub mod tcc;
pub mod util;
pub mod writer;
