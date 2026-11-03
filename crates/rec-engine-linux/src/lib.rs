//! SOURCE OF TRUTH KEYWORDS: rec-engine-linux, LinuxEngine, ScreenCast portal, PipeWire, GStreamer, Secret Service
//! WHAT:  The Linux recording engine: `LinuxEngine` (`rec_core::RecorderEngine`), the
//!        GStreamer media backend (`MediaLibrary`), Secret Service stream keys (`Secrets`),
//!        and the desktop helpers the Tauri shell needs (reveal in the file manager, the
//!        GlobalShortcuts portal).
//! WHY:   Wayland is the default session on current distros, so capture goes through the
//!        xdg-desktop-portal ScreenCast picker and its PipeWire stream; GStreamer encodes
//!        with whatever the PC has (VA-API, NVENC, x264, OpenH264).
//!        The planning modules are plain Rust and build and test on every OS; the modules
//!        that touch GStreamer, D-Bus or the portals are Linux-only.
//! WHERE: `apps/desktop/src-tauri/src/platform/linux.rs`; see `docs/engineering/platforms.md`.

pub mod caps;
pub mod encoders;
pub mod pipeline;
pub mod portal_state;
pub mod recording;
pub mod timeline;
pub mod trash;

#[cfg(target_os = "linux")]
pub mod desktop;
#[cfg(target_os = "linux")]
pub mod devices;
#[cfg(target_os = "linux")]
pub mod engine;
#[cfg(target_os = "linux")]
pub mod media;
#[cfg(target_os = "linux")]
pub mod portal;
#[cfg(target_os = "linux")]
pub mod secrets;
#[cfg(target_os = "linux")]
pub mod session;

#[cfg(target_os = "linux")]
pub use engine::LinuxEngine;
#[cfg(target_os = "linux")]
pub use media::GstMedia;
#[cfg(target_os = "linux")]
pub use secrets::SecretService;
