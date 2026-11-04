//! SOURCE OF TRUTH KEYWORDS: scale_bgra, bgra_to_nv12, nv12_len, CPU scaler, BT.709 limited range, software video path, bilinear
//! WHAT:  The CPU fallback for the last GPU step of a frame: scale a BGRA image to the
//!        output size (bilinear) and convert it to NV12 (BT.709 matrix, 16–235 range).
//! WHY:   PCs without a usable D3D11 video processor (no GPU, Microsoft Basic Render
//!        Driver, VMs, Remote Desktop, CI runners) refuse `ID3D11VideoDevice`, so the
//!        recording could not start at all. These functions produce the same output the
//!        video processor path does (same colour space, `compose::OUTPUT_COLOR_SPACE`), on
//!        the CPU, with integer maths fast enough for 1080p at 30 fps in release builds.
//!        Pure, so they are unit-tested on any OS. The input is a `CamImage` (B, G, R, X
//!        rows with a stride), the overlay renderer's view of a CPU image.
//! WHERE: `win::compose` (CPU path, after mapping the canvas through a staging texture).

use crate::overlay::CamImage;

/// Bytes in an NV12 image of `w`x`h`: the Y plane, then interleaved U/V at half size in
/// both directions (rounded up). Both planes use `w` (rounded up to even) bytes per row
/// for U/V, `w` for Y.
pub fn nv12_len(w: u32, h: u32) -> usize {
    let (w, h) = (w as usize, h as usize);
    w * h + w.div_ceil(2) * 2 * h.div_ceil(2)
}

/// One bilinear tap along an axis: the two source indices and the weight of the second
/// in 1/256ths. Pixel centres are aligned (`(d + 0.5) * s / d_len - 0.5`).
fn taps(src_len: u32, dst_len: u32) -> Vec<(usize, usize, u32)> {
    let last = src_len.max(1) - 1;
    let k = f64::from(src_len) / f64::from(dst_len.max(1));
    (0..dst_len)
        .map(|d| {
            let s = ((f64::from(d) + 0.5) * k - 0.5).clamp(0.0, f64::from(last));
            let s0 = s.floor() as u32;
            let s1 = (s0 + 1).min(last);
            let f = ((s - f64::from(s0)) * 256.0).round() as u32;
            (s0 as usize, s1 as usize, f.min(256))
        })
        .collect()
}

/**
 * SOURCE OF TRUTH KEYWORDS: scale_bgra, bilinear scale, CPU scaler, BGRA resize
 * WHAT:  Scales `src` to `dw`x`dh` BGRA (alpha 255, `dw * 4` bytes per row) into `out`.
 * WHY:   The CPU video path's replacement for the video processor's scaler. Bilinear with
 *        aligned pixel centres: a 2:1 downscale averages each 2x2 block exactly.
 * WHERE: `win::compose` CPU path; tests below.
 */
pub fn scale_bgra(src: &CamImage, dw: u32, dh: u32, out: &mut Vec<u8>) {
    let (dw_us, dh_us) = (dw as usize, dh as usize);
    out.clear();
    out.resize(dw_us * dh_us * 4, 0);
    if src.width == 0 || src.height == 0 || dw == 0 || dh == 0 {
        return;
    }
    let xs = taps(src.width, dw);
    let ys = taps(src.height, dh);
    let row = |y: usize| -> &[u8] {
        let start = y * src.stride;
        src.data
            .get(start..start + src.width as usize * 4)
            .unwrap_or(&[])
    };
    for (dy, &(y0, y1, fy)) in ys.iter().enumerate() {
        let (r0, r1) = (row(y0), row(y1));
        let dst = &mut out[dy * dw_us * 4..(dy + 1) * dw_us * 4];
        for (dx, &(x0, x1, fx)) in xs.iter().enumerate() {
            let o = dx * 4;
            for c in 0..3 {
                let px = |r: &[u8], x: usize| u32::from(r.get(x * 4 + c).copied().unwrap_or(0));
                let top = px(r0, x0) * (256 - fx) + px(r0, x1) * fx;
                let bottom = px(r1, x0) * (256 - fx) + px(r1, x1) * fx;
                let v = (top * (256 - fy) + bottom * fy + (1 << 15)) >> 16;
                dst[o + c] = v.min(255) as u8;
            }
            dst[o + 3] = 255;
        }
    }
}

// BT.709 full-range RGB to limited-range YCbCr, in 1/65536ths. Each chroma row sums to
// zero, so greys come out at exactly 128.
const Y_R: i32 = 11_966;
const Y_G: i32 = 40_254;
const Y_B: i32 = 4_064;
const U_R: i32 = -6_596;
const U_G: i32 = -22_189;
const U_B: i32 = 28_785;
const V_R: i32 = 28_785;
const V_G: i32 = -26_145;
const V_B: i32 = -2_640;

fn luma(b: i32, g: i32, r: i32) -> u8 {
    (16 + ((Y_R * r + Y_G * g + Y_B * b + (1 << 15)) >> 16)).clamp(0, 255) as u8
}

/// Cb and Cr from the sums of `n` pixels' B, G, R (`n` = 4 for a 2x2 block).
fn chroma(b: i32, g: i32, r: i32, shift: u32) -> (u8, u8) {
    let round = 1 << (shift - 1);
    let u = 128 + ((U_R * r + U_G * g + U_B * b + round) >> shift);
    let v = 128 + ((V_R * r + V_G * g + V_B * b + round) >> shift);
    (u.clamp(0, 255) as u8, v.clamp(0, 255) as u8)
}

/**
 * SOURCE OF TRUTH KEYWORDS: bgra_to_nv12, NV12, BT.709, limited range, chroma subsampling
 * WHAT:  Converts `src` to NV12 in `out` (`nv12_len` bytes, Y rows of `width` bytes, then
 *        U/V rows): BT.709 matrix, 16–235 luma, 16–240 chroma, chroma from each 2x2 block's
 *        average (edge pixels repeat on odd sizes).
 * WHY:   Matches the video processor path (`compose::OUTPUT_COLOR_SPACE`) and the input
 *        type the encoder declares (`MF_MT_YUV_MATRIX` BT.709, `MFNominalRange_16_235`).
 * WHERE: `win::compose` CPU path; tests below.
 */
pub fn bgra_to_nv12(src: &CamImage, out: &mut Vec<u8>) {
    let (w, h) = (src.width as usize, src.height as usize);
    out.clear();
    out.resize(nv12_len(src.width, src.height), 0);
    if w == 0 || h == 0 {
        return;
    }
    let px = |x: usize, y: usize| -> (i32, i32, i32) {
        let i = y.min(h - 1) * src.stride + x.min(w - 1) * 4;
        match src.data.get(i..i + 3) {
            Some(p) => (i32::from(p[0]), i32::from(p[1]), i32::from(p[2])),
            None => (0, 0, 0),
        }
    };
    let (y_plane, uv_plane) = out.split_at_mut(w * h);
    for y in 0..h {
        for x in 0..w {
            let (b, g, r) = px(x, y);
            y_plane[y * w + x] = luma(b, g, r);
        }
    }
    let cw = w.div_ceil(2);
    for cy in 0..h.div_ceil(2) {
        for cx in 0..cw {
            let (mut b, mut g, mut r) = (0, 0, 0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = px(cx * 2 + dx, cy * 2 + dy);
                b += p.0;
                g += p.1;
                r += p.2;
            }
            let (u, v) = chroma(b, g, r, 18);
            let o = cy * cw * 2 + cx * 2;
            uv_plane[o] = u;
            uv_plane[o + 1] = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let [b, g, r] = f(x, y);
                v.extend_from_slice(&[b, g, r, 255]);
            }
        }
        v
    }

    fn view(data: &[u8], w: u32, h: u32) -> CamImage<'_> {
        CamImage {
            width: w,
            height: h,
            stride: w as usize * 4,
            data,
        }
    }

    /// The BT.709 limited-range formula in floating point, for (b, g, r).
    fn reference(b: f64, g: f64, r: f64) -> (f64, f64, f64) {
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        (
            16.0 + 219.0 / 255.0 * y,
            128.0 + 224.0 / 255.0 * (b - y) / 1.8556,
            128.0 + 224.0 / 255.0 * (r - y) / 1.5748,
        )
    }

    fn close(a: u8, b: f64) -> bool {
        (f64::from(a) - b).abs() <= 1.0
    }

    #[test]
    fn nv12_sizes() {
        assert_eq!(nv12_len(1920, 1080), 1920 * 1080 * 3 / 2);
        assert_eq!(nv12_len(4, 2), 12);
        // Odd sizes round the chroma plane up.
        assert_eq!(nv12_len(3, 3), 9 + 2 * 2 * 2);
        assert_eq!(nv12_len(1, 1), 1 + 2);
        assert_eq!(nv12_len(0, 0), 0);
    }

    #[test]
    fn known_colours_convert_to_bt709_limited_range() {
        for [b, g, r] in [
            [0u8, 0, 0],
            [255, 255, 255],
            [128, 128, 128],
            [0, 0, 255],
            [0, 255, 0],
            [255, 0, 0],
            [40, 170, 220],
        ] {
            let data = image(2, 2, |_, _| [b, g, r]);
            let mut out = vec![];
            bgra_to_nv12(&view(&data, 2, 2), &mut out);
            assert_eq!(out.len(), 6);
            let (ry, ru, rv) = reference(f64::from(b), f64::from(g), f64::from(r));
            for &y in &out[..4] {
                assert!(close(y, ry), "Y {y} vs {ry} for {b},{g},{r}");
            }
            assert!(close(out[4], ru), "U {} vs {ru} for {b},{g},{r}", out[4]);
            assert!(close(out[5], rv), "V {} vs {rv} for {b},{g},{r}", out[5]);
        }
        // The range ends exactly.
        let mut out = vec![];
        bgra_to_nv12(&view(&image(2, 2, |_, _| [0, 0, 0]), 2, 2), &mut out);
        assert_eq!(out, [16, 16, 16, 16, 128, 128]);
        bgra_to_nv12(&view(&image(2, 2, |_, _| [255; 3]), 2, 2), &mut out);
        assert_eq!(out, [235, 235, 235, 235, 128, 128]);
    }

    #[test]
    fn chroma_averages_each_two_by_two_block() {
        // Left column black, right column white: luma keeps both, chroma stays grey.
        let data = image(2, 2, |x, _| if x == 0 { [0; 3] } else { [255; 3] });
        let mut out = vec![];
        bgra_to_nv12(&view(&data, 2, 2), &mut out);
        assert_eq!(out, [16, 235, 16, 235, 128, 128]);
        // A red block next to a blue block: one chroma pair each.
        let data = image(4, 2, |x, _| if x < 2 { [0, 0, 255] } else { [255, 0, 0] });
        bgra_to_nv12(&view(&data, 4, 2), &mut out);
        assert_eq!(out.len(), 12);
        let (_, ru, rv) = reference(0.0, 0.0, 255.0);
        let (_, bu, bv) = reference(255.0, 0.0, 0.0);
        assert!(close(out[8], ru) && close(out[9], rv));
        assert!(close(out[10], bu) && close(out[11], bv));
    }

    #[test]
    fn odd_sizes_and_strides_convert() {
        // 3x3 with a padded stride: the edge pixels repeat for the last chroma sample.
        let (w, h, stride) = (3u32, 3u32, 16usize);
        let mut data = vec![0u8; stride * h as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                data[y * stride + x * 4..y * stride + x * 4 + 4].copy_from_slice(&[255; 4]);
            }
        }
        let img = CamImage {
            width: w,
            height: h,
            stride,
            data: &data,
        };
        let mut out = vec![];
        bgra_to_nv12(&img, &mut out);
        assert_eq!(out.len(), nv12_len(3, 3));
        assert!(out[..9].iter().all(|&y| y == 235));
        assert!(out[9..].iter().all(|&c| c == 128));
    }

    #[test]
    fn scaling_by_half_averages_blocks() {
        // 4x2 → 2x1: each output pixel is the mean of a 2x2 block.
        let data = image(4, 2, |x, y| {
            let v = (x * 40 + y * 20) as u8;
            [v, v, v]
        });
        let mut out = vec![];
        scale_bgra(&view(&data, 4, 2), 2, 1, &mut out);
        assert_eq!(out.len(), 8);
        // Block 0: 0, 40, 20, 60 → 30. Block 1: 80, 120, 100, 140 → 110.
        assert_eq!(&out[..4], &[30, 30, 30, 255]);
        assert_eq!(&out[4..], &[110, 110, 110, 255]);
    }

    #[test]
    fn same_size_scaling_is_a_copy() {
        let data = image(5, 3, |x, y| [(x * 50) as u8, (y * 80) as u8, 7]);
        let mut out = vec![];
        scale_bgra(&view(&data, 5, 3), 5, 3, &mut out);
        assert_eq!(out, data);
    }

    #[test]
    fn upscaling_interpolates_and_keeps_the_ends() {
        // 2x1 black-to-white → 4x1.
        let data = image(2, 1, |x, _| if x == 0 { [0; 3] } else { [255; 3] });
        let mut out = vec![];
        scale_bgra(&view(&data, 2, 1), 4, 1, &mut out);
        let v: Vec<u8> = out.chunks(4).map(|p| p[0]).collect();
        assert_eq!(v[0], 0);
        assert_eq!(v[3], 255);
        assert!(v[1] > 0 && v[1] < v[2] && v[2] < 255, "{v:?}");
    }

    #[test]
    fn odd_canvas_scales_to_the_even_output_size() {
        // A 1919x1079 capture records at the even size `rec_core::output` gives.
        let (sw, sh) = (1919u32, 1079u32);
        let s = rec_core::settings::RecorderSettings::default();
        let cfg = crate::plan::encoder_config(&s, (sw, sh), false).unwrap().0;
        assert_eq!((cfg.width % 2, cfg.height % 2), (0, 0));
        let data = image(sw, sh, |x, y| [(x % 256) as u8, (y % 256) as u8, 90]);
        let (mut scaled, mut nv12) = (vec![], vec![]);
        scale_bgra(&view(&data, sw, sh), cfg.width, cfg.height, &mut scaled);
        assert_eq!(scaled.len(), (cfg.width * cfg.height * 4) as usize);
        bgra_to_nv12(&view(&scaled, cfg.width, cfg.height), &mut nv12);
        assert_eq!(nv12.len(), (cfg.width * cfg.height * 3 / 2) as usize);
    }

    #[test]
    fn empty_or_short_input_does_not_panic() {
        let mut out = vec![];
        scale_bgra(&view(&[], 0, 0), 4, 2, &mut out);
        assert_eq!(out.len(), 32);
        scale_bgra(&view(&[1, 2, 3], 2, 2), 2, 2, &mut out);
        assert_eq!(out.len(), 16);
        bgra_to_nv12(&view(&[1, 2], 2, 2), &mut out);
        assert_eq!(out.len(), 6);
    }
}
