//! SOURCE OF TRUTH KEYWORDS: Gpu, D3D11CreateDevice, IDirect3DDevice, texture, BGRA, NV12, multithread protected, WinRT interop
//! WHAT:  The Direct3D 11 device the capture, compositor and encoder share, its WinRT
//!        wrapper (Windows.Graphics.Capture wants an `IDirect3DDevice`), and texture helpers.
//! WHY:   Frames stay on the GPU from capture to encoder. One device for all of them means
//!        no copies between devices; multithread protection lets the capture pool, the
//!        pacer and the Media Foundation encoder use it from their own threads. PCs with
//!        no video-capable driver (VMs, Remote Desktop, the Basic Render Driver) get a
//!        device without video support, or WARP, and the CPU video path.
//! WHERE: `win::capture`, `win::compose`, `win::encoder`, `win::session`.

use windows::Graphics::DirectX::Direct3D11::{IDirect3DDevice, IDirect3DSurface};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_FLAG, D3D11_CPU_ACCESS_FLAG, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_CREATE_DEVICE_FLAG, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_RESOURCE_MISC_FLAG,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::core::{Interface, Result};

/// The shared GPU device.
#[derive(Clone)]
pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    /// The same device for Windows.Graphics.Capture.
    pub winrt: IDirect3DDevice,
    /// Created with video support (the D3D11 video processor may exist).
    pub video: bool,
    /// What the device is, for the log ("hardware", "hardware, no video", "warp").
    pub kind: &'static str,
}

// SAFETY: the device is created with multithread protection (`SetMultithreadProtected`),
// which serialises immediate-context calls across threads; ID3D11Device itself is
// free-threaded. The WinRT device wrapper is agile.
unsafe impl Send for Gpu {}
// SAFETY: as above.
unsafe impl Sync for Gpu {}

impl Gpu {
    /// A hardware device with BGRA (capture) and video (scaler, encoder) support; else a
    /// hardware device without video support (old or basic display drivers); else WARP,
    /// Microsoft's software rasterizer, when there is no hardware device at all. Without
    /// video support the compositor scales on the CPU (`compose::Compositor`).
    pub fn new() -> Result<Gpu> {
        let bgra = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        let attempts = [
            (
                D3D_DRIVER_TYPE_HARDWARE,
                bgra | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
                true,
                "hardware",
            ),
            (D3D_DRIVER_TYPE_HARDWARE, bgra, false, "hardware, no video"),
            (D3D_DRIVER_TYPE_WARP, bgra, false, "warp"),
        ];
        let mut first = None;
        for (driver, flags, video, kind) in attempts {
            match Gpu::create(driver, flags, video, kind) {
                Ok(g) => return Ok(g),
                Err(e) => {
                    eprintln!("cortex: d3d11 {kind} device: {}", e.message());
                    first.get_or_insert(e);
                }
            }
        }
        Err(first.unwrap_or_else(windows::core::Error::empty))
    }

    fn create(
        driver: D3D_DRIVER_TYPE,
        flags: D3D11_CREATE_DEVICE_FLAG,
        video: bool,
        kind: &'static str,
    ) -> Result<Gpu> {
        let mut device = None;
        let mut context = None;
        // SAFETY: out-pointers are valid; no adapter means the default one.
        unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        let device: ID3D11Device = device.ok_or_else(windows::core::Error::empty)?;
        let context: ID3D11DeviceContext = context.ok_or_else(windows::core::Error::empty)?;
        let mt: ID3D11Multithread = context.cast()?;
        // SAFETY: plain call on a live interface; the return value is the previous state.
        let _ = unsafe { mt.SetMultithreadProtected(true) };
        let dxgi: IDXGIDevice = device.cast()?;
        // SAFETY: `dxgi` is a live DXGI device.
        let winrt: IDirect3DDevice =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? }.cast()?;
        Ok(Gpu {
            device,
            context,
            winrt,
            video,
            kind,
        })
    }

    /// A 2D texture.
    pub fn texture(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        bind: D3D11_BIND_FLAG,
        usage: D3D11_USAGE,
        cpu: D3D11_CPU_ACCESS_FLAG,
    ) -> Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: usage,
            BindFlags: bind.0 as u32,
            CPUAccessFlags: cpu.0 as u32,
            MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
        };
        let mut tex = None;
        // SAFETY: `desc` is valid for the call; no initial data.
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut tex))? };
        tex.ok_or_else(windows::core::Error::empty)
    }
}

/// The D3D11 texture behind a captured WinRT surface.
pub fn surface_texture(surface: &IDirect3DSurface) -> Result<ID3D11Texture2D> {
    let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
    // SAFETY: the surface belongs to a frame that is alive for this call.
    unsafe { access.GetInterface() }
}
