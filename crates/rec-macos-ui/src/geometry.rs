//! Pure geometry shared by the preview view and the web layer. No AppKit
//! here, so it is unit-tested on any thread.

/**
 * SOURCE OF TRUTH KEYWORDS: Rect, WebRect, preview frame, window points, layout rect
 * WHAT:  An axis-aligned rectangle in points (f64, like CGRect).
 * WHY:   Keeps the coordinate maths independent of objc2 types so it can be tested.
 * WHERE: PreviewView::set_frame, web_rect_to_appkit, aspect_fit.
 */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// Negative or NaN sizes (a collapsed flex item, a bad IPC value) become 0,
    /// so AppKit never receives an invalid frame.
    pub fn sanitized(self) -> Self {
        let clean = |v: f64| if v.is_finite() { v } else { 0.0 };
        Self {
            x: clean(self.x),
            y: clean(self.y),
            w: clean(self.w).max(0.0),
            h: clean(self.h).max(0.0),
        }
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: web_rect_to_appkit, flipped, top-left origin, bottom-left origin, getBoundingClientRect
 * WHAT:  Converts a rect measured by the web layout (CSS px, top-left origin of
 *        the webview) into a frame in the container NSView's coordinates.
 * WHY:   AppKit views are bottom-left origin unless `isFlipped`; the web is
 *        always top-left. CSS px equal AppKit points (both are scale-independent).
 * WHERE: PreviewView::set_frame.
 */
pub fn web_rect_to_appkit(web: Rect, container_height: f64, container_flipped: bool) -> Rect {
    let r = web.sanitized();
    if container_flipped {
        r
    } else {
        Rect::new(r.x, container_height - r.y - r.h, r.w, r.h)
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: aspect_fit, letterbox, pillarbox, resizeAspect, video gravity, contain
 * WHAT:  Where a `src_w`×`src_h` image lands inside `dst` when scaled to fit
 *        (CSS `object-fit: contain`, AVLayerVideoGravityResizeAspect).
 * WHY:   The layer letterboxes on its own; the web layer needs the same rect to
 *        place camera-overlay handles exactly over the visible video.
 * WHERE: Callers that position overlays over the preview; demo checks.
 */
pub fn aspect_fit(src_w: f64, src_h: f64, dst: Rect) -> Rect {
    let dst = dst.sanitized();
    if !(src_w > 0.0 && src_h > 0.0) || dst.w <= 0.0 || dst.h <= 0.0 {
        return Rect::new(dst.x + dst.w / 2.0, dst.y + dst.h / 2.0, 0.0, 0.0);
    }
    let scale = (dst.w / src_w).min(dst.h / src_h);
    let (w, h) = (src_w * scale, src_h * scale);
    Rect::new(dst.x + (dst.w - w) / 2.0, dst.y + (dst.h - h) / 2.0, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unflipped_container_inverts_y() {
        // 800pt tall window, preview 100pt from the top, 300pt tall.
        let f = web_rect_to_appkit(Rect::new(20.0, 100.0, 400.0, 300.0), 800.0, false);
        assert_eq!(f, Rect::new(20.0, 400.0, 400.0, 300.0));
    }

    #[test]
    fn flipped_container_keeps_y() {
        let f = web_rect_to_appkit(Rect::new(20.0, 100.0, 400.0, 300.0), 800.0, true);
        assert_eq!(f, Rect::new(20.0, 100.0, 400.0, 300.0));
    }

    #[test]
    fn full_window_rect_maps_to_origin() {
        let f = web_rect_to_appkit(Rect::new(0.0, 0.0, 1200.0, 800.0), 800.0, false);
        assert_eq!(f, Rect::new(0.0, 0.0, 1200.0, 800.0));
    }

    #[test]
    fn invalid_sizes_are_clamped() {
        let f = web_rect_to_appkit(Rect::new(f64::NAN, 10.0, -5.0, f64::INFINITY), 100.0, true);
        assert_eq!(f, Rect::new(0.0, 10.0, 0.0, 0.0));
    }

    #[test]
    fn aspect_fit_pillarboxes_wide_box() {
        // 16:9 source into a 1000x400 box: height-limited.
        let r = aspect_fit(1920.0, 1080.0, Rect::new(0.0, 0.0, 1000.0, 400.0));
        let w = 400.0 * 16.0 / 9.0;
        assert!((r.w - w).abs() < 1e-9 && (r.h - 400.0).abs() < 1e-9);
        assert!((r.x - (1000.0 - w) / 2.0).abs() < 1e-9 && r.y == 0.0);
    }

    #[test]
    fn aspect_fit_letterboxes_tall_box() {
        let r = aspect_fit(3840.0, 2160.0, Rect::new(10.0, 10.0, 160.0, 160.0));
        assert_eq!(r, Rect::new(10.0, 45.0, 160.0, 90.0));
    }

    #[test]
    fn aspect_fit_degenerate_source_is_empty_center() {
        let r = aspect_fit(0.0, 1080.0, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(r, Rect::new(50.0, 25.0, 0.0, 0.0));
    }
}
