//! SOURCE OF TRUTH KEYWORDS: Compositor, canvas, CopySubresourceRegion, crop, overlay quad, premultiplied blend, ID3D11VideoProcessor, BGRA to NV12, scale
//! WHAT:  The GPU half of each recorded frame: copy the captured texture (cropped to the
//!        region) into a BGRA canvas, blend the camera overlay image onto it, then scale it
//!        to the output size and convert it to NV12 with the D3D11 video processor, into a
//!        small ring of textures the encoder reads. Where the video processor is missing
//!        (no GPU, Basic Render Driver, VMs, Remote Desktop), the canvas is read back
//!        through a staging texture and scaled and converted on the CPU (`crate::convert`)
//!        instead. The path is chosen once, when the compositor is made.
//! WHY:   On the GPU path frames never leave the GPU. The crop is a plain copy, the overlay is one textured
//!        quad (its mask was drawn on the CPU, see `overlay.rs`), and the video processor is
//!        the driver's own scaler and colour converter, which hardware encoders take as is.
//!        The ring keeps a texture untouched while the encoder may still read it.
//! WHERE: `win::session` (video thread).

use std::mem::ManuallyDrop;
use std::sync::Arc;

use windows::Win32::Foundation::{E_NOTIMPL, RECT};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_NV12, DXGI_RATIONAL,
};
use windows::core::{Interface, PCSTR, Result};

use super::d3d::Gpu;
use crate::convert::{bgra_to_nv12, scale_bgra};
use crate::overlay::CamImage;
use crate::plan::{CropRect, clip_crop};

/// Output textures in flight between the scaler and the encoder. A texture is reused
/// only when fewer than `RING - 1` frames wait for the encoder (`Encoder::video_backlog`).
pub const RING: usize = 8;

/// The overlay quad: the vertex shader makes a full-viewport strip from the vertex id
/// (the viewport is the overlay's rectangle), the pixel shader samples the
/// premultiplied image.
const OVERLAY_HLSL: &str = r"
struct VOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
VOut vs_main(uint id : SV_VertexID) {
    float2 uv = float2(id & 1, id >> 1);
    VOut o;
    o.pos = float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    return o;
}
Texture2D tex : register(t0);
SamplerState smp : register(s0);
float4 ps_main(VOut i) : SV_Target { return tex.Sample(smp, i.uv); }
";

fn compile(entry: &[u8], target: &[u8]) -> Result<Vec<u8>> {
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // SAFETY: the source, entry point and target are valid for the call; `entry` and
    // `target` are NUL-terminated byte strings; the blobs are out-pointers.
    let r = unsafe {
        D3DCompile(
            OVERLAY_HLSL.as_ptr().cast(),
            OVERLAY_HLSL.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr()),
            PCSTR(target.as_ptr()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(e) = r {
        let detail = errors
            .map(|b| {
                // SAFETY: the blob owns `GetBufferSize` bytes at `GetBufferPointer`.
                let s = unsafe {
                    std::slice::from_raw_parts(b.GetBufferPointer().cast::<u8>(), b.GetBufferSize())
                };
                String::from_utf8_lossy(s).into_owned()
            })
            .unwrap_or_default();
        eprintln!("rec-engine-windows: shader compile failed: {detail}");
        return Err(e);
    }
    let code = code.ok_or_else(windows::core::Error::empty)?;
    // SAFETY: as above.
    let bytes = unsafe {
        std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
    };
    Ok(bytes.to_vec())
}

/// The overlay pass: shaders, blend state, sampler, and the current overlay image.
struct OverlayPass {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    blend: ID3D11BlendState,
    sampler: ID3D11SamplerState,
    image: Option<(ID3D11Texture2D, ID3D11ShaderResourceView, (u32, u32))>,
    /// Where the image goes on the canvas, in canvas pixels.
    rect: [f32; 4],
}

impl OverlayPass {
    fn new(gpu: &Gpu) -> Result<OverlayPass> {
        let vs_code = compile(b"vs_main\0", b"vs_4_0\0")?;
        let ps_code = compile(b"ps_main\0", b"ps_4_0\0")?;
        let (mut vs, mut ps) = (None, None);
        // SAFETY: the bytecode slices are valid compiled shaders; out-pointers are valid.
        unsafe {
            gpu.device
                .CreateVertexShader(&vs_code, None, Some(&mut vs))?;
            gpu.device
                .CreatePixelShader(&ps_code, None, Some(&mut ps))?;
        }
        let rt = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut bd = D3D11_BLEND_DESC::default();
        bd.RenderTarget[0] = rt;
        let sd = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let (mut blend, mut sampler) = (None, None);
        // SAFETY: the descriptions are valid; out-pointers are valid.
        unsafe {
            gpu.device.CreateBlendState(&bd, Some(&mut blend))?;
            gpu.device.CreateSamplerState(&sd, Some(&mut sampler))?;
        }
        let none = windows::core::Error::empty;
        Ok(OverlayPass {
            vs: vs.ok_or_else(none)?,
            ps: ps.ok_or_else(none)?,
            blend: blend.ok_or_else(none)?,
            sampler: sampler.ok_or_else(none)?,
            image: None,
            rect: [0.0; 4],
        })
    }
}

/// The scaler and its output ring.
struct Scaler {
    vctx: ID3D11VideoContext,
    proc: ID3D11VideoProcessor,
    input: ID3D11VideoProcessorInputView,
    ring: Vec<(ID3D11Texture2D, ID3D11VideoProcessorOutputView)>,
    next: usize,
}

/// `D3D11_VIDEO_PROCESSOR_COLOR_SPACE` bits: BT.709 matrix, 16–235 nominal range (what
/// H.264/HEVC players expect for SDR video).
const OUTPUT_COLOR_SPACE: u32 = (1 << 2) | (1 << 4);
/// Full-range RGB in.
const INPUT_COLOR_SPACE: u32 = 0;

impl Scaler {
    fn new(
        gpu: &Gpu,
        canvas: &ID3D11Texture2D,
        cw: u32,
        ch: u32,
        ow: u32,
        oh: u32,
        fps: u32,
    ) -> Result<Scaler> {
        let vdev: ID3D11VideoDevice = gpu.device.cast()?;
        let vctx: ID3D11VideoContext = gpu.context.cast()?;
        let rate = DXGI_RATIONAL {
            Numerator: fps,
            Denominator: 1,
        };
        let desc = D3D11_VIDEO_PROCESSOR_CONTENT_DESC {
            InputFrameFormat: D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE,
            InputFrameRate: rate,
            InputWidth: cw,
            InputHeight: ch,
            OutputFrameRate: rate,
            OutputWidth: ow,
            OutputHeight: oh,
            Usage: D3D11_VIDEO_USAGE_OPTIMAL_QUALITY,
        };
        // SAFETY: `desc` is valid; the device supports video (created with VIDEO_SUPPORT).
        let enumerator = unsafe { vdev.CreateVideoProcessorEnumerator(&desc)? };
        // SAFETY: plain queries on a live enumerator.
        let (bgra_in, nv12_out) = unsafe {
            (
                enumerator.CheckVideoProcessorFormat(DXGI_FORMAT_B8G8R8A8_UNORM)?,
                enumerator.CheckVideoProcessorFormat(DXGI_FORMAT_NV12)?,
            )
        };
        if bgra_in & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT.0 as u32 == 0
            || nv12_out & D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT.0 as u32 == 0
        {
            return Err(windows::core::Error::new(
                E_NOTIMPL,
                "the video processor can't convert BGRA to NV12",
            ));
        }
        // SAFETY: a live enumerator; rate conversion 0 always exists.
        let proc = unsafe { vdev.CreateVideoProcessor(&enumerator, 0)? };
        let iv_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
            FourCC: 0,
            ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPIV {
                    MipSlice: 0,
                    ArraySlice: 0,
                },
            },
        };
        let mut input = None;
        // SAFETY: the canvas is a live BGRA texture; out-pointer valid.
        unsafe {
            vdev.CreateVideoProcessorInputView(canvas, &enumerator, &iv_desc, Some(&mut input))?
        };
        let input = input.ok_or_else(windows::core::Error::empty)?;
        let ov_desc = D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC {
            ViewDimension: D3D11_VPOV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_VPOV { MipSlice: 0 },
            },
        };
        let mut ring = Vec::with_capacity(RING);
        for _ in 0..RING {
            let tex = gpu.texture(
                ow,
                oh,
                DXGI_FORMAT_NV12,
                D3D11_BIND_RENDER_TARGET,
                D3D11_USAGE_DEFAULT,
                D3D11_CPU_ACCESS_FLAG(0),
            )?;
            let mut view = None;
            // SAFETY: a live NV12 render-target texture; out-pointer valid.
            unsafe {
                vdev.CreateVideoProcessorOutputView(&tex, &enumerator, &ov_desc, Some(&mut view))?
            };
            ring.push((tex, view.ok_or_else(windows::core::Error::empty)?));
        }
        let src = RECT {
            left: 0,
            top: 0,
            right: cw as i32,
            bottom: ch as i32,
        };
        let dst = RECT {
            left: 0,
            top: 0,
            right: ow as i32,
            bottom: oh as i32,
        };
        let in_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
            _bitfield: INPUT_COLOR_SPACE,
        };
        let out_cs = D3D11_VIDEO_PROCESSOR_COLOR_SPACE {
            _bitfield: OUTPUT_COLOR_SPACE,
        };
        // SAFETY: plain state setters on a live processor with valid pointers.
        unsafe {
            vctx.VideoProcessorSetStreamFrameFormat(&proc, 0, D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
            vctx.VideoProcessorSetStreamSourceRect(&proc, 0, true, Some(&src));
            vctx.VideoProcessorSetStreamDestRect(&proc, 0, true, Some(&dst));
            vctx.VideoProcessorSetOutputTargetRect(&proc, true, Some(&dst));
            vctx.VideoProcessorSetStreamColorSpace(&proc, 0, &in_cs);
            vctx.VideoProcessorSetOutputColorSpace(&proc, &out_cs);
            vctx.VideoProcessorSetStreamAutoProcessingMode(&proc, 0, false);
        }
        Ok(Scaler {
            vctx,
            proc,
            input,
            ring,
            next: 0,
        })
    }

    /// Scale and convert the canvas into the next ring texture; returns it.
    fn run(&mut self) -> Result<ID3D11Texture2D> {
        let (tex, view) = &self.ring[self.next % self.ring.len()];
        self.next = self.next.wrapping_add(1);
        let mut stream = D3D11_VIDEO_PROCESSOR_STREAM {
            Enable: true.into(),
            pInputSurface: ManuallyDrop::new(Some(self.input.clone())),
            ..Default::default()
        };
        // SAFETY: the processor, views and stream are live for the call.
        let r = unsafe {
            self.vctx
                .VideoProcessorBlt(&self.proc, view, 0, std::slice::from_ref(&stream))
        };
        // SAFETY: drops the clone put into the stream above, exactly once.
        unsafe { ManuallyDrop::drop(&mut stream.pInputSurface) };
        r?;
        Ok(tex.clone())
    }
}

/// The CPU path: the canvas is copied to a staging texture, mapped, then scaled and
/// converted with `crate::convert`.
struct CpuScaler {
    staging: ID3D11Texture2D,
    size: (u32, u32),
    out: (u32, u32),
    scaled: Vec<u8>,
}

impl CpuScaler {
    fn new(gpu: &Gpu, cw: u32, ch: u32, ow: u32, oh: u32) -> Result<CpuScaler> {
        let staging = gpu.texture(
            cw,
            ch,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            D3D11_BIND_FLAG(0),
            D3D11_USAGE_STAGING,
            D3D11_CPU_ACCESS_READ,
        )?;
        Ok(CpuScaler {
            staging,
            size: (cw, ch),
            out: (ow, oh),
            scaled: vec![],
        })
    }

    fn run(&mut self, gpu: &Gpu, canvas: &ID3D11Texture2D) -> Result<Vec<u8>> {
        let ctx = &gpu.context;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: both textures are `size` BGRA on this device; the staging texture is
        // CPU-readable. Map waits for the copy.
        unsafe {
            ctx.CopyResource(&self.staging, canvas);
            ctx.Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let (cw, ch) = self.size;
        let stride = mapped.RowPitch as usize;
        let len = stride * (ch as usize - 1) + cw as usize * 4;
        let mut nv12 = vec![];
        if !mapped.pData.is_null() && stride >= cw as usize * 4 {
            // SAFETY: a mapped `cw`x`ch` texture holds `ch` rows of `RowPitch` bytes (the
            // last row at least `cw * 4`) until Unmap below.
            let data = unsafe { std::slice::from_raw_parts(mapped.pData.cast::<u8>(), len) };
            let img = CamImage {
                width: cw,
                height: ch,
                stride,
                data,
            };
            if self.out == self.size {
                bgra_to_nv12(&img, &mut nv12);
            } else {
                scale_bgra(&img, self.out.0, self.out.1, &mut self.scaled);
                let scaled = CamImage {
                    width: self.out.0,
                    height: self.out.1,
                    stride: self.out.0 as usize * 4,
                    data: &self.scaled,
                };
                bgra_to_nv12(&scaled, &mut nv12);
            }
        }
        // SAFETY: mapped above; the slice is no longer used.
        unsafe { ctx.Unmap(&self.staging, 0) };
        if nv12.is_empty() {
            return Err(windows::core::Error::new(
                E_NOTIMPL,
                "the canvas could not be read back",
            ));
        }
        Ok(nv12)
    }
}

/// Where the scaling and colour conversion run.
enum Convert {
    Gpu(Scaler),
    Cpu(CpuScaler),
}

/**
 * SOURCE OF TRUTH KEYWORDS: Frame, NV12 frame, GPU texture, system memory, write_video
 * WHAT:  One encoder-ready NV12 frame: a D3D11 texture (GPU path) or bytes (CPU path,
 *        `width` bytes per row, `crate::convert::nv12_len` long).
 * WHY:   The encoder takes either: a DXGI surface buffer or a memory buffer.
 * WHERE: `Compositor::finish`, `Encoder::write_video`, the video thread.
 */
#[derive(Clone)]
pub enum Frame {
    Gpu(ID3D11Texture2D),
    Cpu(Arc<Vec<u8>>),
}

/**
 * SOURCE OF TRUTH KEYWORDS: Compositor, crop copy, overlay blend, video processor, NV12 ring
 * WHAT:  Turns captured BGRA frames into encoder-ready NV12 textures at the output size.
 * WHY:   One place for every GPU step of a frame (crop, camera, scale, colour).
 * WHERE: `win::session` video thread.
 */
pub struct Compositor {
    gpu: Gpu,
    canvas: ID3D11Texture2D,
    canvas_rtv: ID3D11RenderTargetView,
    canvas_size: (u32, u32),
    crop: CropRect,
    convert: Convert,
    overlay: Option<OverlayPass>,
}

impl Compositor {
    /// `crop`: the part of each captured frame to keep (its size is the canvas size).
    /// `out`: the encoded size. `overlay`: set up the camera pass. The video processor
    /// is used when the device has one that converts BGRA to NV12 at these sizes;
    /// otherwise the CPU path (`is_gpu` says which).
    pub fn new(
        gpu: &Gpu,
        crop: CropRect,
        out: (u32, u32),
        fps: u32,
        overlay: bool,
    ) -> Result<Compositor> {
        let (cw, ch) = (crop.w.max(2), crop.h.max(2));
        let canvas = gpu.texture(
            cw,
            ch,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            D3D11_BIND_FLAG(D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0),
            D3D11_USAGE_DEFAULT,
            D3D11_CPU_ACCESS_FLAG(0),
        )?;
        let mut rtv = None;
        // SAFETY: a live texture with RENDER_TARGET binding; out-pointer valid.
        unsafe {
            gpu.device
                .CreateRenderTargetView(&canvas, None, Some(&mut rtv))?
        };
        let canvas_rtv = rtv.ok_or_else(windows::core::Error::empty)?;
        let gpu_scaler = if gpu.video {
            Scaler::new(gpu, &canvas, cw, ch, out.0, out.1, fps)
        } else {
            Err(windows::core::Error::new(
                E_NOTIMPL,
                "the Direct3D device has no video support",
            ))
        };
        let convert = match gpu_scaler {
            Ok(s) => Convert::Gpu(s),
            Err(e) => {
                eprintln!(
                    "cortex: no GPU video processor ({}); scaling on the CPU",
                    e.message()
                );
                Convert::Cpu(CpuScaler::new(gpu, cw, ch, out.0, out.1)?)
            }
        };
        let overlay = if overlay {
            match OverlayPass::new(gpu) {
                Ok(o) => Some(o),
                Err(e) => {
                    eprintln!("rec-engine-windows: no camera overlay: {}", e.message());
                    None
                }
            }
        } else {
            None
        };
        let c = Compositor {
            gpu: gpu.clone(),
            canvas,
            canvas_rtv,
            canvas_size: (cw, ch),
            crop,
            convert,
            overlay,
        };
        c.clear();
        Ok(c)
    }

    pub fn canvas_size(&self) -> (u32, u32) {
        self.canvas_size
    }

    /// The D3D11 video processor scales and converts (frames stay on the GPU).
    pub fn is_gpu(&self) -> bool {
        matches!(self.convert, Convert::Gpu(_))
    }

    fn clear(&self) {
        // SAFETY: a live render-target view.
        unsafe {
            self.gpu
                .context
                .ClearRenderTargetView(&self.canvas_rtv, &[0.0, 0.0, 0.0, 1.0]);
        }
    }

    /// Copy the cropped part of a captured texture (`frame` pixels of content) onto the
    /// canvas. A source smaller than the crop (a window that shrank) leaves black around it.
    pub fn draw_capture(&self, tex: &ID3D11Texture2D, frame: (u32, u32)) {
        let Some(c) = clip_crop(self.crop, frame) else {
            self.clear();
            return;
        };
        if (c.w, c.h) != self.canvas_size {
            self.clear();
        }
        let b = D3D11_BOX {
            left: c.x,
            top: c.y,
            front: 0,
            right: c.x + c.w,
            bottom: c.y + c.h,
            back: 1,
        };
        // SAFETY: both textures are BGRA on this device; the box lies inside the source
        // (`clip_crop`) and the copy fits the canvas (the crop never exceeds it).
        unsafe {
            self.gpu
                .context
                .CopySubresourceRegion(&self.canvas, 0, 0, 0, 0, tex, 0, Some(&b));
        }
    }

    /// Fill the canvas from CPU BGRA rows (the test pattern), `w * 4` bytes per row.
    pub fn draw_pixels(&self, bgra: &[u8], w: u32, h: u32) {
        if (w, h) != self.canvas_size || bgra.len() < (w * h * 4) as usize {
            return;
        }
        // SAFETY: `bgra` holds `h` rows of `w * 4` bytes, the canvas is `w`x`h` BGRA.
        unsafe {
            self.gpu.context.UpdateSubresource(
                &self.canvas,
                0,
                None,
                bgra.as_ptr().cast(),
                w * 4,
                0,
            );
        }
    }

    /// Black canvas (camera-only scene before the camera's first frame).
    pub fn draw_black(&self) {
        self.clear();
    }

    /// Replace the overlay image (premultiplied BGRA, `w * 4` bytes per row) and where it
    /// goes on the canvas (`rect` in canvas pixels).
    pub fn set_overlay(&mut self, bgra: &[u8], w: u32, h: u32, rect: [f32; 4]) {
        let gpu = self.gpu.clone();
        let Some(o) = &mut self.overlay else {
            return;
        };
        if w == 0 || h == 0 || bgra.len() < (w * h * 4) as usize {
            o.image = None;
            return;
        }
        if o.image.as_ref().is_none_or(|i| i.2 != (w, h)) {
            let made = gpu
                .texture(
                    w,
                    h,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    D3D11_BIND_SHADER_RESOURCE,
                    D3D11_USAGE_DEFAULT,
                    D3D11_CPU_ACCESS_FLAG(0),
                )
                .and_then(|t| {
                    let mut srv = None;
                    // SAFETY: a live texture with SHADER_RESOURCE binding.
                    unsafe {
                        gpu.device
                            .CreateShaderResourceView(&t, None, Some(&mut srv))?
                    };
                    Ok((t, srv.ok_or_else(windows::core::Error::empty)?, (w, h)))
                });
            o.image = made.ok();
        }
        if let Some((t, _, _)) = &o.image {
            // SAFETY: `bgra` holds `h` rows of `w * 4` bytes; the texture is `w`x`h` BGRA.
            unsafe {
                gpu.context
                    .UpdateSubresource(t, 0, None, bgra.as_ptr().cast(), w * 4, 0);
            }
        }
        o.rect = rect;
    }

    fn draw_overlay(&self) {
        let Some(o) = &self.overlay else { return };
        let Some((_, srv, _)) = &o.image else { return };
        let ctx = &self.gpu.context;
        let vp = D3D11_VIEWPORT {
            TopLeftX: o.rect[0],
            TopLeftY: o.rect[1],
            Width: o.rect[2],
            Height: o.rect[3],
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        // SAFETY: every object is live on this device; slices outlive the calls.
        unsafe {
            ctx.OMSetRenderTargets(
                Some(&[Some(self.canvas_rtv.clone())]),
                None::<&ID3D11DepthStencilView>,
            );
            ctx.OMSetBlendState(&o.blend, None, 0xffff_ffff);
            ctx.RSSetViewports(Some(&[vp]));
            ctx.IASetInputLayout(None::<&ID3D11InputLayout>);
            ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            ctx.VSSetShader(&o.vs, None);
            ctx.PSSetShader(&o.ps, None);
            ctx.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            ctx.PSSetSamplers(0, Some(&[Some(o.sampler.clone())]));
            ctx.Draw(4, 0);
            // Unbind so the canvas can be read by the video processor.
            ctx.OMSetRenderTargets(None, None::<&ID3D11DepthStencilView>);
            ctx.PSSetShaderResources(0, Some(&[None]));
        }
    }

    /// Finish the frame: overlay, scale, convert. Returns the NV12 frame to encode.
    pub fn finish(&mut self) -> Result<Frame> {
        self.draw_overlay();
        match &mut self.convert {
            Convert::Gpu(s) => s.run().map(Frame::Gpu),
            Convert::Cpu(s) => s
                .run(&self.gpu, &self.canvas)
                .map(|b| Frame::Cpu(Arc::new(b))),
        }
    }
}
