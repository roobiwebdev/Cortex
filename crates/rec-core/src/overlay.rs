//! SOURCE OF TRUTH KEYWORDS: overlay_rect, overlay_size, overlay_margin, shape_coverage, shape_mask, squircle_exponent, camera overlay geometry, pip
//! WHAT:  Where the camera overlay goes in an output frame and which pixels its shape covers,
//!        in `rec-core` settings types (`PipShape`, `PipPlace`).
//! WHY:   Every engine draws the same overlay: the same 2% corner margin, the same square
//!        shapes (circle, square, squircle), the same anti-aliased edge. The macOS engine has
//!        this math in `rec-engine-macos::compose` over Rec's own types, which other
//!        platforms cannot build; this is the platform-neutral copy, checked against the same
//!        numbers. `compose` can switch to it when the Rec diff is next touched.
//! WHERE: `rec-engine-linux` (compositor position, alpha mask); Windows next.

use crate::settings::{PipCorner, PipPlace, PipShape};

/// The shape is drawn in a square box (otherwise the camera's aspect).
pub fn is_square(shape: PipShape) -> bool {
    matches!(
        shape,
        PipShape::Circle | PipShape::Square | PipShape::Squircle
    )
}

/// Gap between a corner-snapped overlay and the frame edges.
pub fn overlay_margin(w: f64) -> f64 {
    (w * 0.02).round()
}

/// Overlay size in output pixels for a `w`x`h` output: `size` of the width, the shape's
/// aspect (square, or the camera's).
pub fn overlay_size(size: f64, shape: PipShape, w: f64, h: f64, cam_aspect: f64) -> (f64, f64) {
    let aspect = if is_square(shape) {
        1.0
    } else {
        cam_aspect.max(0.1)
    };
    let ow = (w * size).round().clamp(16.0, w.max(16.0));
    let oh = (ow / aspect).round().clamp(16.0, h.max(16.0));
    (ow, oh)
}

/// Overlay rectangle `[x, y, width, height]` in output pixels from the top-left, always
/// inside the frame.
pub fn overlay_rect(
    size: f64,
    place: &PipPlace,
    shape: PipShape,
    w: f64,
    h: f64,
    cam_aspect: f64,
) -> [f64; 4] {
    let (ow, oh) = overlay_size(size, shape, w, h, cam_aspect);
    let (x, y) = match place {
        PipPlace::Corner { corner } => {
            let m = overlay_margin(w);
            let left = matches!(corner, PipCorner::TopLeft | PipCorner::BottomLeft);
            let top = matches!(corner, PipCorner::TopLeft | PipCorner::TopRight);
            (
                if left { m } else { w - m - ow },
                if top { m } else { h - m - oh },
            )
        }
        PipPlace::Free { x, y } => ((x * w - ow / 2.0).round(), (y * h - oh / 2.0).round()),
    };
    [
        x.clamp(0.0, (w - ow).max(0.0)),
        y.clamp(0.0, (h - oh).max(0.0)),
        ow,
        oh,
    ]
}

/// Superellipse exponent for a squircle: 2 (a circle) at roundness 1, 4 at 0.5, 10 (nearly
/// square) at 0.
pub fn squircle_exponent(roundness: f64) -> f64 {
    2.0 + 8.0 * (1.0 - roundness.clamp(0.0, 1.0)).powi(2)
}

/// Anti-aliased coverage (0..1) of the pixel centred at (`px`, `py`) by `shape` filling a
/// `w`x`h` box.
pub fn shape_coverage(shape: PipShape, w: f64, h: f64, roundness: f64, px: f64, py: f64) -> f32 {
    let (hw, hh) = (w / 2.0, h / 2.0);
    let (x, y) = ((px - hw).abs(), (py - hh).abs());
    // Approximate signed distance to the edge in pixels (negative inside).
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

/// Alpha mask (row-major, `w`x`h`, 0..255) of the shape inset by `inset` pixels.
pub fn shape_mask(shape: PipShape, w: usize, h: usize, roundness: f64, inset: f64) -> Vec<u8> {
    let (iw, ih) = (w as f64 - 2.0 * inset, h as f64 - 2.0 * inset);
    let mut out = vec![0u8; w * h];
    if iw <= 0.0 || ih <= 0.0 {
        return out;
    }
    for y in 0..h {
        for x in 0..w {
            let c = shape_coverage(
                shape,
                iw,
                ih,
                roundness,
                x as f64 + 0.5 - inset,
                y as f64 + 0.5 - inset,
            );
            out[y * w + x] = (c * 255.0).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BR: PipPlace = PipPlace::Corner {
        corner: PipCorner::BottomRight,
    };

    #[test]
    fn corner_overlays_sit_inside_the_margin() {
        // 1920x1080, 22% wide, 16:9 camera, rounded: 422x237 at 2% (38 px) from the corner.
        let r = overlay_rect(0.22, &BR, PipShape::Rounded, 1920.0, 1080.0, 16.0 / 9.0);
        assert_eq!(r, [1460.0, 805.0, 422.0, 237.0]);
        let tl = PipPlace::Corner {
            corner: PipCorner::TopLeft,
        };
        let r = overlay_rect(0.22, &tl, PipShape::Circle, 1920.0, 1080.0, 16.0 / 9.0);
        assert_eq!(r, [38.0, 38.0, 422.0, 422.0]);
    }

    #[test]
    fn free_placement_is_clamped_into_the_frame() {
        let p = PipPlace::Free { x: 0.5, y: 0.5 };
        let r = overlay_rect(0.2, &p, PipShape::Square, 1000.0, 500.0, 1.0);
        assert_eq!(r, [400.0, 150.0, 200.0, 200.0]);
        let p = PipPlace::Free { x: 1.0, y: 1.0 };
        let r = overlay_rect(0.2, &p, PipShape::Square, 1000.0, 500.0, 1.0);
        assert_eq!(r, [800.0, 300.0, 200.0, 200.0]);
    }

    #[test]
    fn masks_cover_the_inside_and_clear_the_corners() {
        let m = shape_mask(PipShape::Circle, 100, 100, 0.0, 0.0);
        assert_eq!(m[50 * 100 + 50], 255, "centre");
        assert_eq!(m[0], 0, "corner");
        let edge = m[50 * 100];
        assert!(edge > 0 && edge < 255, "edge is anti-aliased: {edge}");
        let m = shape_mask(PipShape::Rect, 10, 10, 0.0, 0.0);
        assert!(m.iter().all(|&a| a == 255));
        let m = shape_mask(PipShape::Rounded, 100, 50, 0.5, 0.0);
        assert_eq!(m[0], 0);
        assert_eq!(m[25 * 100 + 50], 255);
        // An inset clears a ring all round.
        let m = shape_mask(PipShape::Rect, 10, 10, 0.0, 2.0);
        assert_eq!(m[10 + 1], 0);
        assert_eq!(m[5 * 10 + 5], 255);
    }

    #[test]
    fn squircle_runs_from_circle_to_square() {
        assert_eq!(squircle_exponent(1.0), 2.0);
        assert_eq!(squircle_exponent(0.5), 4.0);
        assert_eq!(squircle_exponent(0.0), 10.0);
    }
}
