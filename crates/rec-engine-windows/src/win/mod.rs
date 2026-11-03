//! SOURCE OF TRUTH KEYWORDS: WinBackend, Windows backend, probe, Backend implementation, Windows.Graphics.Capture, Media Foundation, WASAPI
//! WHAT:  The Windows side of the engine: `WinBackend` implements `engine::Backend` with
//!        the modules below (devices, capture, compositor, encoder, audio, camera,
//!        Credential Manager), and `shell`/`media` serve the Tauri shell's platform traits.
//! WHY:   Everything that calls Windows lives here, behind `cfg(windows)`, so the rest of
//!        the crate stays pure and testable on any OS.
//! WHERE: `WindowsEngine` (= `Engine<WinBackend>`), the shell's `platform/windows.rs`.

mod audio;
mod camera;
mod capture;
mod compose;
mod d3d;
pub mod devices;
mod encoder;
pub mod media;
pub mod secrets;
mod session;
pub mod shell;
mod util;

use std::path::Path;

use rec_core::api::EngineError;
use rec_core::library::LocalTime;
use rec_core::platform::{PlatformCapabilities, Secrets};
use rec_core::state::{Devices, Permissions};

use crate::caps::{Probe, windows_capabilities};
use crate::engine::{Backend, SessionRequest, Started};

pub use devices::privacy_uri;

/**
 * SOURCE OF TRUTH KEYWORDS: WinBackend, Backend, probe, capabilities, devices, start session
 * WHAT:  The Windows backend of the engine.
 * WHY:   See the module header.
 * WHERE: `rec_engine_windows::WindowsEngine`.
 */
pub struct WinBackend {
    probe: Probe,
    secrets: secrets::CredentialStore,
    fragmented: bool,
}

impl WinBackend {
    /// Probe the PC. `fragmented`: write fragmented MP4 (crash-safe, unverified).
    pub fn new(fragmented: bool) -> WinBackend {
        util::com_init();
        let _ = util::mf_startup();
        WinBackend {
            probe: Probe {
                build: util::windows_build(),
                capture: capture::Capture::supported(),
                hevc: encoder::hevc_encoder_available(),
            },
            secrets: secrets::CredentialStore,
            fragmented,
        }
    }

    pub fn probe(&self) -> Probe {
        self.probe
    }
}

impl Backend for WinBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        windows_capabilities(&self.probe)
    }

    fn devices(&self) -> Devices {
        util::com_init();
        devices::list_devices()
    }

    fn permissions(&self) -> Permissions {
        devices::permissions()
    }

    fn start(&self, req: &SessionRequest) -> Result<Started, EngineError> {
        util::com_init();
        if !self.probe.capture && !req.test_pattern {
            return Err(EngineError::Unsupported {
                message: "Screen capture needs Windows 10 version 1903 or later".into(),
            });
        }
        let gpu = d3d::Gpu::new()
            .map_err(|e| EngineError::failed(util::msg("No Direct3D 11 GPU device", &e)))?;
        session::start(req, &gpu, &self.probe, self.fragmented)
    }

    fn free_disk_bytes(&self, dir: &Path) -> u64 {
        util::free_disk_bytes(dir)
    }

    fn local_time(&self) -> LocalTime {
        util::local_time()
    }

    fn secrets(&self) -> &dyn Secrets {
        &self.secrets
    }
}
