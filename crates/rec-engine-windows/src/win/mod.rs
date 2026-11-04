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
    probe: std::sync::Arc<ProbeSlot>,
    secrets: secrets::CredentialStore,
    fragmented: bool,
}

/// The probe result, filled in by a background thread. Until it lands, `capabilities`
/// reports a conservative guess and a recording start waits for it (off the UI thread).
struct ProbeSlot {
    value: std::sync::Mutex<Option<Probe>>,
    ready: std::sync::Condvar,
    /// Read synchronously (a registry value): cheap and never blocks.
    build: u32,
}

/// Screen capture (Windows.Graphics.Capture) arrived in Windows 10 1903, build 18362.
const BUILD_GRAPHICS_CAPTURE: u32 = 18_362;
/// How long a recording start waits for a slow probe before going ahead with the guess.
const PROBE_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

impl WinBackend {
    /// Starts probing the PC in the background and returns at once. `fragmented`: write
    /// fragmented MP4 (crash-safe, unverified).
    ///
    /// Never waits for the probe: the shell builds this on the UI thread, and the probe
    /// enumerates hardware encoder MFTs, which loads GPU drivers. On a laptop with two
    /// GPUs (Intel + NVIDIA) that can take seconds or hang in the driver, and waiting for
    /// it here froze the whole window ("Not Responding") before it ever drew.
    pub fn new(fragmented: bool) -> WinBackend {
        let slot = std::sync::Arc::new(ProbeSlot {
            value: std::sync::Mutex::new(None),
            ready: std::sync::Condvar::new(),
            build: util::windows_build(),
        });
        let fill = slot.clone();
        let spawned = std::thread::Builder::new()
            .name("win-probe".into())
            .spawn(move || {
                let t = std::time::Instant::now();
                util::com_init();
                let _ = util::mf_startup();
                let probe = Probe {
                    build: fill.build,
                    capture: capture::Capture::supported(),
                    hevc: encoder::hevc_encoder_available(),
                };
                eprintln!(
                    "cortex: windows probe took {} ms: {probe:?}",
                    t.elapsed().as_millis()
                );
                if let Ok(mut v) = fill.value.lock() {
                    *v = Some(probe);
                }
                fill.ready.notify_all();
            });
        if let Err(e) = spawned {
            eprintln!("cortex: could not start the windows probe: {e}");
        }
        WinBackend {
            probe: slot,
            secrets: secrets::CredentialStore,
            fragmented,
        }
    }

    /// The probe if it has finished, else the guess from the Windows build alone.
    pub fn probe(&self) -> Probe {
        self.probe.current()
    }
}

impl ProbeSlot {
    fn guess(&self) -> Probe {
        Probe {
            build: self.build,
            capture: self.build >= BUILD_GRAPHICS_CAPTURE,
            hevc: false,
        }
    }

    fn current(&self) -> Probe {
        self.value
            .lock()
            .ok()
            .and_then(|v| *v)
            .unwrap_or_else(|| self.guess())
    }

    /// Wait up to `limit` for the probe; the guess if it is still running.
    fn wait(&self, limit: std::time::Duration) -> Probe {
        let Ok(guard) = self.value.lock() else {
            return self.guess();
        };
        match self.ready.wait_timeout_while(guard, limit, |v| v.is_none()) {
            Ok((v, _)) => v.unwrap_or_else(|| self.guess()),
            Err(_) => self.guess(),
        }
    }
}

impl Backend for WinBackend {
    fn capabilities(&self) -> PlatformCapabilities {
        windows_capabilities(&self.probe.current())
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
        // Off the UI thread here: waiting for a slow probe only delays this recording.
        let probe = self.probe.wait(PROBE_WAIT);
        if !probe.capture && !req.test_pattern {
            return Err(EngineError::Unsupported {
                message: "Screen capture needs Windows 10 version 1903 or later".into(),
            });
        }
        let gpu = d3d::Gpu::new()
            .map_err(|e| EngineError::failed(util::msg("No Direct3D 11 GPU device", &e)))?;
        session::start(req, &gpu, &probe, self.fragmented)
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
