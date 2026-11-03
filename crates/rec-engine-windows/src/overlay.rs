//! SOURCE OF TRUTH KEYWORDS: OverlaySpec, overlay_rect, coverage, cover_crop, render_overlay, camera bubble, pip shape mask, border
//! WHAT:  The camera overlay for the Windows engine: where it sits in the output frame
//!        (corner with margin or free centre), its size and shape, and a CPU renderer that
//!        turns one camera frame into a premultiplied BGRA image of the overlay (cover crop,
//!        mirror, anti-aliased shape mask, border). The GPU then only blends that image.
//! WHY:   The geometry and mask maths are the macOS compositor's (`compose::overlay_rect`,
//!        `compose::coverage`, `border_px`), so the bubble lands on the same pixels on both
//!        systems and matches the React drag handle. Doing the mask on the CPU keeps the
//!        Direct3D side to one trivial textured quad. Pure, so it is unit-tested anywhere.
//! WHERE: `win::compose` (overlay texture), `win::session`.

use rec_core::settings::{PipCorner, PipPlace, PipShape, RecorderSettings, normalize_hex};

/// What the overlay looks like, from the settings.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySpec {
    /// Width as a fraction of the output width.
    pub size: f64,
    pub place: PipPlace,
    pub shape: PipShape,
    pub roundness: f64,
    /// Border width in pixels at 1080p.
    pub border: f64,
    pub border_color: [u8; 3],
    pub mirror: bool,
}

impl OverlaySpec {
    pub fn from_settings(s: &RecorderSettings) -> OverlaySpec {
        let hex = normalize_hex(&s.pip_border_color).unwrap_or_else(|| "ffffff".into());
        let c = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
        OverlaySpec {
            size: s.pip_size,
            place: s.pip_place.clone(),
            shape: s.pip_shape,
            roundness: s.pip_roundness,
            border: s.pip_border,
            border_color: [c(0), c(2), c(4)],
            mirror: s.mirror,
        }
    }
}

/// Circle, square and squircle crop the camera to a centred square.
pub fn is_square(shape: PipShape) -> bool {
    matches!(
        shape,
        PipShape::Circle | PipShape::Square | PipShape::Squircle
    )
}

/// Gap between a corner-snapped overlay and the frame edges.
pub fn margin(w: f64) -> f64 {
    (w * 0.02).round()
}

/// Overlay `[x, y, width, height]` in output pixels for a `w`x`h` frame, always inside it.
pub fn overlay_rect(spec: &OverlaySpec, w: f64, h: f64, cam_aspect: f64) -> [f64; 4] {
    let aspect = if is_square(spec.shape) {
        1.0
    } else {
        cam_aspect.max(0.1)
    };
    let ow = (w * spec.size).round().clamp(16.0_f64.min(w), w);
    let oh = (ow / aspect).round().clamp(16.0_f64.min(h), h);
    let (x, y) = match &spec.place {
        PipPlace::Corner { corner } => {
            let m = margin(w);
            let left = matches!(corner, PipCorner::TopLeft | PipCorner::BottomLeft);
            let top = matches!(corner, PipCorner::TopLeft | PipCorner::TopRight);
            (
                if left { m } else { w - m - ow },
                if top { m } else { h - m - oh },
            )
        }
        PipPlace::Free { x, y } => ((x * w - ow / 2.0).round(), (y * h - oh / 2.0).round()),
    };
    [x.clamp(0.0, w - ow), y.clamp(0.0, h - oh), ow, oh]
}

/// Border width in output pixels (the setting is pixels at 1080p).
pub fn border_px(spec: &OverlaySpec, out_h: f64) -> f64 {
    (spec.border.max(0.0) * out_h / 1080.0).round()
}

/// Superellipse exponent for a squircle (2 = circle at roundness 1, 10 near square at 0).
pub fn squircle_exponent(roundness: f64) -> f64 {
    2.0 + 8.0 * (1.0 - roundness.clamp(0.0, 1.0)).powi(2)
}

/// Anti-aliased coverage (0..1) of the pixel centred at (`px`, `py`) by `shape` filling a
/// `w`x`h` box.
pub fn coverage(shape: PipShape, w: f64, h: f64, roundness: f64, px: f64, py: f64) -> f32 {
    if w <= 0.0 || h <= 0.0 {
        return 0.0;
    }
    let (hw, hh) = (w / 2.0, h / 2.0);
    let (x, y) = ((px - hw).abs(), (py - hh).abs());
    let d = match shape {
        PipShape::Squircle => {
            let n = squircle_exponent(roundness);
            (((x / hw).powf(n) + (y / hh).powf(n)).powf(1.0 / n) - 1.0) * hw.min(hh)
        }
        PipShape::Circle => (((x / hw).powi(2) + (y / hh).powi(2)).sqrt() - 1.0) * hw.min(hh),
        _ => {
            let r = match shape {
                PipShape::Pill => hw.min(hh),
                PipShape::Rect => 0.0,
                _ => roundness.clamp(0.0, 1.0) * hw.min(hh),
            };
            let (qx, qy) = (x - (hw - r), y - (hh - r));
            (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
        }
    };
    (0.5 - d).clamp(0.0, 1.0) as f32
}

/// The part of a `cam`-sized frame that fills a `dst`-shaped box edge to edge (centred
/// crop), as `[x0, y0, x1, y1]` in camera pixels.
pub fn cover_crop(cam: (u32, u32), dst: (f64, f64)) -> [f64; 4] {
    let (cw, ch) = (cam.0.max(1) as f64, cam.1.max(1) as f64);
    let want = if dst.1 > 0.0 { dst.0 / dst.1 } else { 1.0 };
    if cw / ch > want {
        let w = ch * want;
        let x0 = (cw - w) / 2.0;
        [x0, 0.0, x0 + w, ch]
    } else {
        let h = cw / want;
        let y0 = (ch - h) / 2.0;
        [0.0, y0, cw, y0 + h]
    }
}

/// A camera frame on the CPU: 32-bit B, G, R, X rows (Media Foundation's RGB32).
pub struct CamImage<'a> {
    pub width: u32,
    pub height: u32,
    /// Bytes per row.
    pub stride: usize,
    pub data: &'a [u8],
}

impl CamImage<'_> {
    /// Bilinear sample at camera pixel coordinates; [b, g, r].
    fn sample(&self, x: f64, y: f64) -> [f32; 3] {
        let (w, h) = (self.width.max(1) as i64, self.height.max(1) as i64);
        let fx = (x - 0.5).clamp(0.0, (w - 1) as f64);
        let fy = (y - 0.5).clamp(0.0, (h - 1) as f64);
        let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (tx, ty) = ((fx - x0 as f64) as f32, (fy - y0 as f64) as f32);
        let px = |xi: i64, yi: i64| -> [f32; 3] {
            let i = yi as usize * self.stride + xi as usize * 4;
            match self.data.get(i..i + 3) {
                Some(b) => [b[0] as f32, b[1] as f32, b[2] as f32],
                None => [0.0; 3],
            }
        };
        let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
        let mut out = [0.0; 3];
        for k in 0..3 {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bottom = c[k] + (d[k] - c[k]) * tx;
            out[k] = top + (bottom - top) * ty;
        }
        out
    }
}

/// The shape's coverage per pixel for a `w`x`h` overlay: the outer edge and the edge
/// inside a `border`-pixel band (equal when there is no border).
pub fn masks(spec: &OverlaySpec, w: u32, h: u32, border: f64) -> (Vec<f32>, Vec<f32>) {
    let (fw, fh) = (w as f64, h as f64);
    let (iw, ih) = (fw - 2.0 * border, fh - 2.0 * border);
    let n = (w * h) as usize;
    let (mut outer, mut inner) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for y in 0..h {
        for x in 0..w {
            let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
            let o = coverage(spec.shape, fw, fh, spec.roundness, cx, cy);
            let i = if border <= 0.0 {
                o
            } else if iw > 0.0 && ih > 0.0 && o > 0.0 {
                coverage(spec.shape, iw, ih, spec.roundness, cx - border, cy - border)
            } else {
                0.0
            };
            outer.push(o);
            inner.push(i);
        }
    }
    (outer, inner)
}

/// Renders overlays frame after frame, keeping the masks while the size and shape stay
/// the same (they cost more than the camera copy).
#[derive(Default)]
pub struct OverlayRenderer {
    key: Option<(u32, u32, PipShape, u64, u64)>,
    outer: Vec<f32>,
    inner: Vec<f32>,
}

impl OverlayRenderer {
    /// The overlay as premultiplied BGRA rows (`w * 4` bytes each), ready to blend with
    /// "one, one minus source alpha". `border` is in these pixels. The camera fills the
    /// shape inside the border (cover crop, mirrored when `spec.mirror`).
    pub fn render(
        &mut self,
        spec: &OverlaySpec,
        cam: &CamImage,
        w: u32,
        h: u32,
        border: f64,
    ) -> Vec<u8> {
        let key = (w, h, spec.shape, spec.roundness.to_bits(), border.to_bits());
        if self.key != Some(key) {
            (self.outer, self.inner) = masks(spec, w, h, border);
            self.key = Some(key);
        }
        let (fw, fh) = (w as f64, h as f64);
        let crop = cover_crop((cam.width, cam.height), (fw, fh));
        let (sx, sy) = (
            (crop[2] - crop[0]) / fw.max(1.0),
            (crop[3] - crop[1]) / fh.max(1.0),
        );
        let bc = spec.border_color;
        let border_bgr = [bc[2] as f32, bc[1] as f32, bc[0] as f32];
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let k = (y * w + x) as usize;
                let outer = self.outer.get(k).copied().unwrap_or(0.0);
                if outer <= 0.0 {
                    continue;
                }
                let inner = self.inner.get(k).copied().unwrap_or(0.0);
                let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
                let u = if spec.mirror { fw - cx } else { cx };
                let c = cam.sample(crop[0] + u * sx, crop[1] + cy * sy);
                let band = (outer - inner).max(0.0);
                let i = k * 4;
                for j in 0..3 {
                    out[i + j] = (c[j] * inner + border_bgr[j] * band)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
                out[i + 3] = (outer * 255.0).round() as u8;
            }
        }
        out
    }
}

/// One overlay frame (see `OverlayRenderer::render`).
pub fn render_overlay(spec: &OverlaySpec, cam: &CamImage, w: u32, h: u32, border: f64) -> Vec<u8> {
    OverlayRenderer::default().render(spec, cam, w, h, border)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(shape: PipShape, place: PipPlace) -> OverlaySpec {
        OverlaySpec {
            size: 0.22,
            place,
            shape,
            roundness: 0.35,
            border: 0.0,
            border_color: [255, 255, 255],
            mirror: false,
        }
    }

    #[test]
    fn corner_overlays_match_the_mac() {
        let s = spec(
            PipShape::Rounded,
            PipPlace::Corner {
                corner: PipCorner::BottomRight,
            },
        );
        // 22% of 1920 = 422 wide, 16:9 camera = 237 tall, 38 px margin.
        assert_eq!(
            overlay_rect(&s, 1920.0, 1080.0, 16.0 / 9.0),
            [1460.0, 805.0, 422.0, 237.0]
        );
        let c = spec(
            PipShape::Circle,
            PipPlace::Corner {
                corner: PipCorner::TopLeft,
            },
        );
        assert_eq!(
            overlay_rect(&c, 1920.0, 1080.0, 16.0 / 9.0),
            [38.0, 38.0, 422.0, 422.0]
        );
    }

    #[test]
    fn free_overlays_stay_inside() {
        let s = spec(PipShape::Square, PipPlace::Free { x: 0.99, y: 0.5 });
        let r = overlay_rect(&s, 1000.0, 500.0, 1.0);
        assert_eq!(r, [780.0, 140.0, 220.0, 220.0]);
    }

    #[test]
    fn shapes_cover_the_middle_not_the_corners() {
        for shape in PipShape::ALL {
            assert_eq!(
                coverage(*shape, 100.0, 100.0, 0.5, 50.0, 50.0),
                1.0,
                "{shape:?}"
            );
        }
        assert_eq!(coverage(PipShape::Circle, 100.0, 100.0, 0.5, 0.5, 0.5), 0.0);
        assert_eq!(coverage(PipShape::Rect, 100.0, 100.0, 0.5, 0.5, 0.5), 1.0);
        let edge = coverage(PipShape::Circle, 100.0, 100.0, 0.5, 50.0, 0.0);
        assert!(edge > 0.0 && edge < 1.0, "anti-aliased edge {edge}");
    }

    #[test]
    fn cover_crop_centres_the_camera() {
        assert_eq!(
            cover_crop((1920, 1080), (100.0, 100.0)),
            [420.0, 0.0, 1500.0, 1080.0]
        );
        assert_eq!(
            cover_crop((1080, 1080), (160.0, 90.0)),
            [0.0, 236.25, 1080.0, 843.75]
        );
    }

    #[test]
    fn rendering_masks_mirrors_and_borders() {
        // A 4x2 camera: left half red, right half blue (BGRX).
        let mut data = vec![];
        for _ in 0..2 {
            for x in 0..4 {
                data.extend_from_slice(if x < 2 {
                    &[0, 0, 255, 0]
                } else {
                    &[255, 0, 0, 0]
                });
            }
        }
        let cam = CamImage {
            width: 4,
            height: 2,
            stride: 16,
            data: &data,
        };
        let mut s = spec(PipShape::Rect, PipPlace::Free { x: 0.5, y: 0.5 });
        let img = render_overlay(&s, &cam, 40, 20, 0.0);
        let px = |img: &[u8], x: usize, y: usize| {
            let i = (y * 40 + x) * 4;
            [img[i], img[i + 1], img[i + 2], img[i + 3]]
        };
        assert_eq!(px(&img, 2, 10), [0, 0, 255, 255], "red on the left");
        s.mirror = true;
        let img = render_overlay(&s, &cam, 40, 20, 0.0);
        assert_eq!(
            px(&img, 2, 10),
            [255, 0, 0, 255],
            "mirrored: blue on the left"
        );
        // A 3 px white border.
        s.border_color = [255, 255, 255];
        let img = render_overlay(&s, &cam, 40, 20, 3.0);
        assert_eq!(px(&img, 1, 10), [255, 255, 255, 255]);
        assert_eq!(px(&img, 20, 10)[3], 255);
        // A circle leaves the corners transparent (premultiplied zero).
        s.shape = PipShape::Circle;
        let img = render_overlay(&s, &cam, 20, 20, 0.0);
        assert_eq!(&img[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn settings_map_to_the_spec() {
        let s = RecorderSettings {
            pip_border_color: "#FF8000".into(),
            ..RecorderSettings::default()
        };
        let o = OverlaySpec::from_settings(&s);
        assert_eq!(o.border_color, [255, 128, 0]);
        assert_eq!(o.shape, s.pip_shape);
        assert_eq!(border_px(&OverlaySpec { border: 12.0, ..o }, 360.0), 4.0);
    }
}
