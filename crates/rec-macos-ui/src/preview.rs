//! Live preview: a native NSView under the webview that shows the engine's
//! preview frames with zero copies.
//!
//! Layering in a Tauri window (wry 0.57):
//!
//! ```text
//! NSWindow
//!  └─ contentView = WryWebViewParent
//!      ├─ PreviewView   (added by `attach`, ordered BELOW every sibling)
//!      └─ WKWebView     (drawsBackground = NO, page leaves a transparent hole)
//! ```
//!
//! The webview stays on top, so it keeps all mouse and keyboard input and can
//! draw overlay handles over the video. The window itself does not need to be
//! transparent: the window background is painted behind the content view.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSColor, NSView, NSWindow,
    NSWindowDidChangeOcclusionStateNotification, NSWindowOcclusionState, NSWindowOrderingMode,
};
use objc2_av_foundation::{
    AVLayerVideoGravityResizeAspect, AVQueuedSampleBufferRendering,
    AVQueuedSampleBufferRenderingStatus, AVSampleBufferDisplayLayer, AVSampleBufferVideoRenderer,
};
use objc2_core_foundation::{
    CFMutableDictionary, CFRetained, CGPoint, CGRect, CGSize, kCFBooleanTrue,
};
use objc2_core_media::{
    CMFormatDescription, CMSampleBuffer, CMVideoFormatDescriptionCreateForImageBuffer,
    CMVideoFormatDescriptionMatchesImageBuffer, kCMSampleAttachmentKey_DisplayImmediately,
    kCMTimingInfoInvalid,
};
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNumber, NSOperationQueue, NSString,
};
use objc2_quartz_core::CATransaction;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::geometry::{Rect, web_rect_to_appkit};
use crate::main_thread::{MainThreadBound, on_main, run_on_main};

/**
 * SOURCE OF TRUTH KEYWORDS: PreviewFn, preview sink, frame callback, CVPixelBuffer, set_preview_sink
 * WHAT:  The frame callback type. Identical to `rec_engine::writer::PreviewFn`
 *        (this crate does not depend on the engine, so it is restated here).
 * WHY:   Lets `PreviewView::sink()` be passed straight to `MacEngine::set_preview_sink`.
 * WHERE: PreviewView::sink, the Tauri shell, examples/preview_demo.rs.
 */
pub type PreviewFn = Arc<dyn Fn(CFRetained<CVPixelBuffer>) + Send + Sync>;

/// A preview frame's size in pixels (for stats and logs).
pub fn frame_size(pixels: &CVPixelBuffer) -> (usize, usize) {
    (
        objc2_core_video::CVPixelBufferGetWidth(pixels),
        objc2_core_video::CVPixelBufferGetHeight(pixels),
    )
}

/// Why `attach` failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachError {
    NullWindow,
    NoContentView,
}

impl std::fmt::Display for AttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullWindow => f.write_str("NSWindow pointer is null"),
            Self::NoContentView => f.write_str("NSWindow has no content view"),
        }
    }
}

impl std::error::Error for AttachError {}

/// The layer's renderer. Apple documents `sampleBufferRenderer` as the API
/// for enqueueing from a background thread, which is why it may cross threads.
struct Renderer(Retained<AVSampleBufferVideoRenderer>);
// SAFETY: AVSampleBufferVideoRenderer is designed to be fed from any thread
// ("allows the client to safely enqueue sample buffers from a background
// thread", AVSampleBufferDisplayLayer.h). We only call enqueue/status/flush,
// all of which that contract covers; the object is retained, never mutated in Rust.
unsafe impl Send for Renderer {}
// SAFETY: see the Send impl; the renderer synchronises internally.
unsafe impl Sync for Renderer {}

/// Shared state between the view and the frame sink.
struct SinkState {
    renderer: Renderer,
    /// Cached format description; rebuilt only when the frame size or format changes.
    format: Mutex<Option<CFRetained<CMFormatDescription>>>,
    /// False while hidden or detached: frames are dropped without any work.
    active: AtomicBool,
}

impl SinkState {
    /// Hands one frame to the layer. Never blocks: on any contention or
    /// back-pressure the frame is dropped (the next one is ~33 ms away).
    fn push(&self, pixels: &CVPixelBuffer) {
        if !self.active.load(Ordering::Relaxed) {
            return;
        }
        let r = &self.renderer.0;
        // SAFETY: plain property reads/flush on a thread-safe renderer (see `Renderer`).
        unsafe {
            if r.status() == AVQueuedSampleBufferRenderingStatus::Failed
                || r.requiresFlushToResumeDecoding()
            {
                r.flush();
                return;
            }
            if !r.isReadyForMoreMediaData() {
                return;
            }
        }
        let Ok(mut format) = self.format.try_lock() else {
            return;
        };
        let Some(sample) = make_sample(pixels, &mut format) else {
            return;
        };
        drop(format);
        // SAFETY: `sample` is a valid, ready CMSampleBuffer wrapping an IOSurface-backed pixel buffer.
        unsafe { r.enqueueSampleBuffer(&sample) };
    }
}

/// Wraps a pixel buffer in a CMSampleBuffer flagged "display immediately",
/// so the layer shows it now and drops anything older (no timebase needed).
fn make_sample(
    pixels: &CVPixelBuffer,
    format: &mut Option<CFRetained<CMFormatDescription>>,
) -> Option<CFRetained<CMSampleBuffer>> {
    // SAFETY: all pointers passed below are valid for the duration of each call;
    // the out-pointers are written by CoreMedia with +1 references (Create rule),
    // which `CFRetained::from_raw` takes ownership of.
    unsafe {
        let reuse = format
            .as_ref()
            .is_some_and(|f| CMVideoFormatDescriptionMatchesImageBuffer(f, pixels));
        if !reuse {
            let mut out: *const CMFormatDescription = std::ptr::null();
            let status =
                CMVideoFormatDescriptionCreateForImageBuffer(None, pixels, NonNull::from(&mut out));
            *format = if status == 0 {
                NonNull::new(out.cast_mut()).map(|p| CFRetained::from_raw(p))
            } else {
                None
            };
        }
        let desc = format.as_ref()?;
        let timing = kCMTimingInfoInvalid;
        let mut out: *mut CMSampleBuffer = std::ptr::null_mut();
        let status = CMSampleBuffer::create_ready_with_image_buffer(
            None,
            pixels,
            desc,
            NonNull::from(&timing),
            NonNull::from(&mut out),
        );
        if status != 0 {
            return None;
        }
        let sample = CFRetained::from_raw(NonNull::new(out)?);
        if let Some(arr) = sample.sample_attachments_array(true)
            && arr.count() > 0
        {
            let dict = arr
                .value_at_index(0)
                .cast::<CFMutableDictionary>()
                .cast_mut();
            let key: &objc2_core_foundation::CFString = kCMSampleAttachmentKey_DisplayImmediately;
            if let (Some(dict), Some(yes)) = (dict.as_ref(), kCFBooleanTrue) {
                CFMutableDictionary::set_value(
                    Some(dict),
                    (key as *const objc2_core_foundation::CFString).cast(),
                    (yes as *const objc2_core_foundation::CFBoolean).cast(),
                );
            }
        }
        Some(sample)
    }
}

/// AppKit objects owned by a PreviewView; only touched on the main thread.
struct Views {
    view: Retained<NSView>,
    layer: Retained<AVSampleBufferDisplayLayer>,
}

/**
 * SOURCE OF TRUTH KEYWORDS: PreviewView, live preview, AVSampleBufferDisplayLayer, native preview, under webview, zero-copy
 * WHAT:  A layer-hosting NSView placed under the window's webview that displays
 *        engine preview frames via AVSampleBufferDisplayLayer.
 * WHY:   Video frames never cross into the webview (no IPC, no copies); the
 *        layer displays IOSurface-backed buffers directly and letterboxes them.
 * WHERE: Tauri shell main window (record/preview screen); examples/preview_demo.rs.
 *
 * Every method is callable from any thread: AppKit work is dispatched to the
 * main thread asynchronously, and `sink()` frames are enqueued on the caller's thread.
 */
#[derive(Clone)]
pub struct PreviewView {
    views: Arc<MainThreadBound<Views>>,
    state: Arc<SinkState>,
}

impl PreviewView {
    /// Inserts the preview view into `ns_window`'s content view, below every
    /// existing subview (the WKWebView), initially hidden and zero-sized.
    ///
    /// Runs synchronously on the main thread (hops there if needed), so the
    /// main run loop must be running when called from another thread.
    ///
    /// # Safety
    ///
    /// `ns_window` must be null or a valid `NSWindow*` (e.g. Tauri's
    /// `WebviewWindow::ns_window()`) that stays alive while this call runs.
    /// The view is retained by the window afterwards, so the window may close
    /// later; the PreviewView then just goes inert.
    pub unsafe fn attach(ns_window: *mut c_void) -> Result<PreviewView, AttachError> {
        if ns_window.is_null() {
            return Err(AttachError::NullWindow);
        }
        let addr = ns_window as usize;
        run_on_main(move |mtm| {
            // SAFETY: the caller guarantees `addr` is a live NSWindow; we are on the main thread.
            let window: &NSWindow = unsafe { &*(addr as *const NSWindow) };
            build(window, mtm)
        })
    }

    /// Safe form of [`PreviewView::attach`] for any window that exposes an
    /// AppKit handle (Tauri's `WebviewWindow`, winit). Call on the main thread
    /// or with the main run loop running.
    pub fn attach_to(window: &impl HasWindowHandle) -> Result<PreviewView, AttachError> {
        let ns_view = appkit_view(window).ok_or(AttachError::NullWindow)?;
        run_on_main(move |mtm| {
            // SAFETY: the handle contract guarantees a live NSView while `window`
            // is borrowed (this call is synchronous); main thread.
            let view: &NSView = unsafe { &*(ns_view as *const NSView) };
            let window = view.window().ok_or(AttachError::NullWindow)?;
            build(&window, mtm)
        })
    }

    /// Positions the preview from a rect measured by the web layout
    /// (`getBoundingClientRect()` in CSS px, top-left origin of the webview).
    pub fn set_frame(&self, x: f64, y: f64, w: f64, h: f64) {
        let views = self.views.clone();
        on_main(move |mtm| {
            let v = views.get(mtm);
            // SAFETY: reading the superview of a view we own, on the main thread.
            let Some(container) = (unsafe { v.view.superview() }) else {
                return;
            };
            let bounds = container.bounds();
            let f = web_rect_to_appkit(
                Rect::new(x, y, w, h),
                bounds.size.height,
                container.isFlipped(),
            );
            without_animation(|| {
                v.view
                    .setFrame(CGRect::new(CGPoint::new(f.x, f.y), CGSize::new(f.w, f.h)));
            });
        });
    }

    /// Rounds the preview corners (points). 0 for square corners.
    pub fn set_corner_radius(&self, radius: f64) {
        let views = self.views.clone();
        on_main(move |mtm| {
            let v = views.get(mtm);
            without_animation(|| {
                v.layer.setCornerRadius(radius.max(0.0));
                v.layer.setMasksToBounds(radius > 0.0);
            });
        });
    }

    /// Shows the view and starts accepting frames.
    pub fn show(&self) {
        self.state.active.store(true, Ordering::Relaxed);
        let views = self.views.clone();
        on_main(move |mtm| views.get(mtm).view.setHidden(false));
    }

    /// Hides the view; frames are dropped while hidden (no decode/composite cost).
    pub fn hide(&self) {
        self.state.active.store(false, Ordering::Relaxed);
        let views = self.views.clone();
        on_main(move |mtm| views.get(mtm).view.setHidden(true));
    }

    /// Removes the displayed image and anything queued (e.g. on stop or source change).
    pub fn clear(&self) {
        // SAFETY: flush on the thread-safe renderer, no completion handler.
        unsafe {
            self.state
                .renderer
                .0
                .flushWithRemovalOfDisplayedImage_completionHandler(true, None);
        }
    }

    /// Removes the view from the window. Further frames are ignored.
    pub fn detach(&self) {
        self.state.active.store(false, Ordering::Relaxed);
        let views = self.views.clone();
        on_main(move |mtm| views.get(mtm).view.removeFromSuperview());
    }

    /// Calls `f(visible)` now and whenever the window's occlusion state
    /// changes: shown or hidden, minimised, fully covered, or on another
    /// Space. Runs `f` on the main thread. Lets the host stop the engine's
    /// preview session while nobody can see it.
    pub fn on_window_visibility(&self, f: impl Fn(bool) + 'static + Send) {
        let views = self.views.clone();
        on_main(move |mtm| {
            let v = views.get(mtm);
            let Some(window) = v.view.window() else {
                return;
            };
            let visible =
                |w: &NSWindow| w.occlusionState().contains(NSWindowOcclusionState::Visible);
            f(visible(&window));
            let w = window.clone();
            let block = RcBlock::new(move |_n: NonNull<NSNotification>| f(visible(&w)));
            // SAFETY: main thread; the block and window are retained by the
            // notification centre for the life of the app (the main window
            // is never released: closing it only hides it).
            unsafe {
                let _ = NSNotificationCenter::defaultCenter()
                    .addObserverForName_object_queue_usingBlock(
                        Some(NSWindowDidChangeOcclusionStateNotification),
                        Some(&window),
                        Some(&NSOperationQueue::mainQueue()),
                        &block,
                    );
            }
        });
    }

    /// The frame callback for `MacEngine::set_preview_sink`. Thread-safe and
    /// non-blocking: frames are dropped when hidden or when the layer is busy.
    pub fn sink(&self) -> PreviewFn {
        let state = self.state.clone();
        Arc::new(move |pixels: CFRetained<CVPixelBuffer>| state.push(&pixels))
    }
}

fn build(window: &NSWindow, mtm: MainThreadMarker) -> Result<PreviewView, AttachError> {
    let container = window.contentView().ok_or(AttachError::NoContentView)?;
    // SAFETY: standard AVFoundation/AppKit construction on the main thread.
    unsafe {
        let layer = AVSampleBufferDisplayLayer::new();
        if let Some(gravity) = AVLayerVideoGravityResizeAspect {
            layer.setVideoGravity(gravity);
        }
        // Transparent letterbox bars: the web page shows its own surface there.
        layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
        layer.setPreventsDisplaySleepDuringVideoPlayback(false);

        let view = NSView::initWithFrame(NSView::alloc(mtm), CGRect::default());
        // Layer-hosting view: we own the layer, AppKit never draws into it.
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view.setHidden(true);
        // Keep the distance to the window top fixed on resize (web coordinates are top-based).
        view.setAutoresizingMask(if container.isFlipped() {
            NSAutoresizingMaskOptions::ViewMaxYMargin
        } else {
            NSAutoresizingMaskOptions::ViewMinYMargin
        });
        // Sibling order only composites reliably when the container is layer-backed too.
        container.setWantsLayer(true);
        container.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, None);

        let renderer = layer.sampleBufferRenderer();
        Ok(PreviewView {
            views: Arc::new(MainThreadBound::new(Views { view, layer }, mtm)),
            state: Arc::new(SinkState {
                renderer: Renderer(renderer),
                format: Mutex::new(None),
                active: AtomicBool::new(false),
            }),
        })
    }
}

/// Applies layer changes immediately (no implicit 0.25 s animation), so the
/// preview tracks the web layout without lag during window resizes.
fn without_animation(f: impl FnOnce()) {
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    f();
    CATransaction::commit();
}

/// The `NSView*` (as an address) behind an AppKit window handle.
fn appkit_view(window: &impl HasWindowHandle) -> Option<usize> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as usize),
        _ => None,
    }
}

/// Safe form of [`make_webview_transparent`]: applies it to every WKWebView
/// in `window`'s content view. Returns how many were changed.
pub fn make_window_webviews_transparent(window: &impl HasWindowHandle) -> usize {
    let Some(ns_view) = appkit_view(window) else {
        return 0;
    };
    run_on_main(move |_mtm| {
        // SAFETY: live NSView per the handle contract (synchronous call); main thread.
        let view: &NSView = unsafe { &*(ns_view as *const NSView) };
        let Some(root) = view.window().and_then(|w| w.contentView()) else {
            return 0;
        };
        let mut found = 0;
        let mut stack = vec![root];
        while let Some(v) = stack.pop() {
            let class = v.class().name().to_str().unwrap_or_default();
            if class.contains("WKWebView") {
                let ptr = Retained::as_ptr(&v) as *mut c_void;
                // SAFETY: `v` is a live WKWebView (retained in this scope).
                if unsafe { make_webview_transparent(ptr) } {
                    found += 1;
                }
                continue;
            }
            stack.extend(v.subviews().to_vec());
        }
        found
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: make_webview_transparent, drawsBackground, WKWebView, transparent hole, underPageBackgroundColor
 * WHAT:  Stops a WKWebView from painting its opaque default background, so
 *        the native preview under it shows through wherever the page is transparent.
 * WHY:   Tauri only does this for `transparent: true` windows (which require the
 *        `macos-private-api` feature). The main window can stay opaque; only the
 *        webview must not paint. The page must then leave the preview rect unpainted:
 *        `html, body { background: transparent }` and backgrounds only on the
 *        surfaces around the preview, never on an ancestor of the preview slot.
 * WHERE: Tauri shell, after creating the main window (`with_webview(|w| w.inner())`).
 *
 * `drawsBackground` is a private KVC key (the one wry and Tauri use). Call on
 * the main thread or from any thread (dispatched synchronously).
 *
 * # Safety
 *
 * `webview` must be null or a valid `WKWebView*` alive for the duration of the call.
 */
pub unsafe fn make_webview_transparent(webview: *mut c_void) -> bool {
    if webview.is_null() {
        return false;
    }
    let addr = webview as usize;
    run_on_main(move |_mtm| {
        // SAFETY: caller guarantees a live WKWebView; KVC and selector checks on the main thread.
        unsafe {
            let wv: &AnyObject = &*(addr as *const AnyObject);
            let no = NSNumber::numberWithBool(false);
            let key = NSString::from_str("drawsBackground");
            let _: () = msg_send![wv, setValue: &*no, forKey: &*key];
            // macOS 12+: the rubber-band/overscroll area is otherwise opaque too.
            let sel = objc2::sel!(setUnderPageBackgroundColor:);
            let responds: bool = msg_send![wv, respondsToSelector: sel];
            if responds {
                let clear = NSColor::clearColor();
                let _: () = msg_send![wv, setUnderPageBackgroundColor: &*clear];
            }
            true
        }
    })
}
