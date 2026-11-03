//! SOURCE OF TRUTH KEYWORDS: Capture, Windows.Graphics.Capture, GraphicsCaptureItem, CreateForMonitor, CreateForWindow, Direct3D11CaptureFramePool, TryGetNextFrame, cursor, border
//! WHAT:  A Windows.Graphics.Capture session on one monitor or window, polled for its newest
//!        frame. Frames are D3D11 textures on the shared device, stamped on the QPC clock.
//! WHY:   WGC is the supported capture API on Windows 10 1903+ and the only one that sees
//!        `WDA_EXCLUDEFROMCAPTURE` (our HUD stays out). A free-threaded frame pool needs no
//!        dispatcher; the pacer polls it on its own thread at the recording's frame rate
//!        and keeps only the newest frame, so no callback threading is involved.
//! WHERE: `win::session` (the video thread).

use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::Result;

use super::d3d::{Gpu, surface_texture};
use super::util::hns_to_secs;

/// What to capture.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    Monitor(HMONITOR),
    Window(HWND),
}

/// Frames the pool holds: one being read, one being filled.
const POOL_BUFFERS: i32 = 2;

pub struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    gpu: Gpu,
    size: SizeInt32,
}

/// A captured frame; the texture is valid while this lives (it is the pool's buffer).
pub struct Frame {
    _frame: Direct3D11CaptureFrame,
    pub texture: ID3D11Texture2D,
    /// Capture time on the QPC clock, in seconds.
    pub time: f64,
    /// The content's size (a window's client area may be smaller than the texture).
    pub size: (u32, u32),
}

impl Capture {
    /// Is Windows.Graphics.Capture available (Windows 10 1903+)?
    pub fn supported() -> bool {
        GraphicsCaptureSession::IsSupported().unwrap_or(false)
    }

    /// Start capturing `source`. `cursor` draws the pointer; `borderless` asks Windows 11
    /// not to draw the yellow capture border (ignored where unsupported).
    pub fn start(gpu: &Gpu, source: Source, cursor: bool, borderless: bool) -> Result<Capture> {
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        // SAFETY: the handles come from EnumDisplayMonitors / EnumWindows; a stale one
        // makes the call fail, it does not crash.
        let item: GraphicsCaptureItem = unsafe {
            match source {
                Source::Monitor(m) => interop.CreateForMonitor(m)?,
                Source::Window(w) => interop.CreateForWindow(w)?,
            }
        };
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &gpu.winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            POOL_BUFFERS,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        // Windows 10 2004+ (cursor) and Windows 11 (border); older builds lack the
        // property and the call fails, which only means the default stays.
        let _ = session.SetIsCursorCaptureEnabled(cursor);
        if borderless {
            let _ = session.SetIsBorderRequired(false);
        }
        session.StartCapture()?;
        Ok(Capture {
            pool,
            session,
            gpu: gpu.clone(),
            size,
        })
    }

    /// The source's size when the capture started, in pixels.
    pub fn size(&self) -> (u32, u32) {
        (
            self.size.Width.max(0) as u32,
            self.size.Height.max(0) as u32,
        )
    }

    /// The newest frame since the last call, or None. Older queued frames are dropped.
    /// When the source changes size (a resized window), the pool follows it.
    pub fn latest(&mut self) -> Option<Frame> {
        let mut newest = None;
        while let Ok(f) = self.pool.TryGetNextFrame() {
            newest = Some(f);
        }
        let f = newest?;
        let content = f.ContentSize().ok()?;
        if content.Width != self.size.Width || content.Height != self.size.Height {
            if content.Width > 0 && content.Height > 0 {
                self.size = content;
                let _ = self.pool.Recreate(
                    &self.gpu.winrt,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    POOL_BUFFERS,
                    content,
                );
            }
            // This frame was drawn at the old size; the next one has the new size.
            return None;
        }
        let texture = surface_texture(&f.Surface().ok()?).ok()?;
        let time = hns_to_secs(f.SystemRelativeTime().ok()?.Duration);
        Some(Frame {
            _frame: f,
            texture,
            time,
            size: (content.Width as u32, content.Height as u32),
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
