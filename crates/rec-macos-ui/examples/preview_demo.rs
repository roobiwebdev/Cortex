//! Opens a window, attaches a PreviewView under a stand-in "web" label, and
//! feeds synthetic IOSurface-backed BGRA frames at 60 fps, then exits.
//!
//! cargo run --release -p rec-macos-ui --example preview_demo -- [width height seconds]
//! (defaults: 1920 1080 4)
//!
//! Frames are pre-rendered into a small ring so the measured CPU is the
//! display path (sample-buffer wrap + enqueue + composite), not pixel generation.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSFont, NSTextField,
    NSWindow, NSWindowStyleMask,
};
use objc2_core_foundation::{CFRetained, CGPoint, CGRect, CGSize};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferPool,
    CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::NSString;
use rec_engine::util::{PIX_BGRA, make_pool};
use rec_macos_ui::{FloatingKind, PreviewView, configure_floating};

const RING: usize = 8;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, d: usize| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (w, h, secs) = (arg(0, 1920), arg(1, 1080), arg(2, 4));

    let mtm = MainThreadMarker::new().expect("run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let window = make_window(
        mtm,
        CGRect::new(CGPoint::new(200.0, 200.0), CGSize::new(960.0, 620.0)),
        true,
    );
    window.setTitle(&NSString::from_str(&format!(
        "rec-macos-ui preview demo {w}x{h}"
    )));
    window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
    let content = window.contentView().expect("content view");

    // Stand-in for the WKWebView: added first, must stay ABOVE the preview.
    let label = NSTextField::labelWithString(&NSString::from_str("WEB LAYER (above preview)"), mtm);
    label.setFont(Some(&NSFont::boldSystemFontOfSize(28.0)));
    label.setTextColor(Some(&NSColor::systemYellowColor()));
    label.setFrame(CGRect::new(
        CGPoint::new(60.0, 520.0),
        CGSize::new(600.0, 40.0),
    ));
    content.addSubview(&label);

    window.makeKeyAndOrderFront(None);
    app.activate();

    // SAFETY: `window` is a live NSWindow owned by this function for the whole run.
    let preview = unsafe { PreviewView::attach(Retained::as_ptr(&window).cast_mut().cast()) }
        .expect("attach");
    // Web-layout coordinates: 40pt from the left and top, 16:9 slot.
    preview.set_frame(40.0, 40.0, 880.0, 495.0);
    preview.set_corner_radius(14.0);
    preview.show();
    // Same type the engine takes in MacEngine::set_preview_sink.
    let sink: rec_engine::writer::PreviewFn = preview.sink();

    // A floating HUD to exercise the panel helpers.
    let hud = make_window(
        mtm,
        CGRect::new(CGPoint::new(1200.0, 700.0), CGSize::new(260.0, 70.0)),
        false,
    );
    hud.setBackgroundColor(Some(&NSColor::systemRedColor()));
    // SAFETY: `hud` is a live NSWindow owned by this function.
    let na =
        unsafe { configure_floating(Retained::as_ptr(&hud).cast_mut().cast(), FloatingKind::Hud) };
    hud.orderFrontRegardless();
    println!(
        "window={} hud={} hud_level={} hud_sharing={:?} non_activation={:?}",
        window.windowNumber(),
        hud.windowNumber(),
        hud.level(),
        hud.sharingType(),
        na
    );

    let frames = render_ring(w, h);
    println!("prerendered {RING} frames {w}x{h}; feeding 60 fps for {secs}s");
    let sent = Arc::new(AtomicU64::new(0));
    let sent2 = sent.clone();
    let frames = SendFrames(frames);
    std::thread::spawn(move || {
        let frames = frames;
        let start = Instant::now();
        let period = Duration::from_nanos(1_000_000_000 / 60);
        let mut n: u64 = 0;
        while start.elapsed() < Duration::from_secs(secs as u64) {
            let f = &frames.0[(n as usize) % RING];
            sink(f.clone());
            n += 1;
            sent2.store(n, Ordering::Relaxed);
            let next = start + period * n as u32;
            if let Some(d) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(d);
            }
        }
        println!("sent {n} frames in {:.2}s", start.elapsed().as_secs_f64());
        DispatchQueue::main().exec_async(|| {
            // SAFETY: blocks on the main queue run on the main thread.
            let mtm = unsafe { MainThreadMarker::new_unchecked() };
            NSApplication::sharedApplication(mtm).terminate(None);
        });
    });

    let _keep = (label, hud, preview, sent);
    app.run();
}

fn make_window(mtm: MainThreadMarker, rect: CGRect, titled: bool) -> Retained<NSWindow> {
    let style = if titled {
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable
    } else {
        NSWindowStyleMask::Borderless
    };
    // SAFETY: standard AppKit window creation on the main thread; we keep the
    // Retained, so the window must not release itself on close.
    unsafe {
        let w = NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        w.setReleasedWhenClosed(false);
        w
    }
}

/// Pixel buffers are immutable after rendering and CoreVideo retains are atomic.
struct SendFrames(Vec<CFRetained<CVPixelBuffer>>);
// SAFETY: the buffers are only read (retained and handed to the sink) after creation.
unsafe impl Send for SendFrames {}

/// A diagonal gradient with a white bar; each ring frame shifts both, so cycling shows motion.
fn render_ring(w: usize, h: usize) -> Vec<CFRetained<CVPixelBuffer>> {
    let pool = make_pool(w, h, PIX_BGRA);
    (0..RING)
        .map(|k| {
            let mut raw = std::ptr::null_mut();
            // SAFETY: the pool returns a +1 IOSurface-backed BGRA buffer; we write
            // within `h * stride` bytes while the base address is locked.
            unsafe {
                CVPixelBufferPool::create_pixel_buffer(
                    None,
                    &pool,
                    std::ptr::NonNull::from(&mut raw),
                );
                let b = CFRetained::from_raw(std::ptr::NonNull::new(raw).expect("pixel buffer"));
                CVPixelBufferLockBaseAddress(&b, CVPixelBufferLockFlags(0));
                let base = CVPixelBufferGetBaseAddress(&b).cast::<u8>();
                let stride = CVPixelBufferGetBytesPerRow(&b);
                let phase = k * 256 / RING;
                let bar = k * w / RING;
                for y in 0..h {
                    let row = std::slice::from_raw_parts_mut(base.add(y * stride).cast::<u32>(), w);
                    let g = (y * 255 / h) as u32;
                    for (x, px) in row.iter_mut().enumerate() {
                        *px = if x >= bar && x < bar + w / 40 {
                            0xFFFF_FFFF
                        } else {
                            let b = ((x * 255 / w + phase) & 255) as u32;
                            let r = (255 - ((x + y) * 255 / (w + h) + phase) % 256) as u32;
                            0xFF00_0000 | (r << 16) | (g << 8) | b
                        };
                    }
                }
                CVPixelBufferUnlockBaseAddress(&b, CVPixelBufferLockFlags(0));
                b
            }
        })
        .collect()
}
