//! ScreenCaptureKit: display video, system audio, and microphone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_core_graphics::{CGDisplayCopyDisplayMode, CGDisplayMode, kCGColorSpaceSRGB, kCGDisplayStreamYCbCrMatrix_ITU_R_709_2};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSObject, NSObjectProtocol, NSString};
use objc2_screen_capture_kit::*;
use dispatch2::DispatchQueue;

use crate::record::Shared;
use crate::util::*;
use crate::writer::AudioKind;

pub struct DisplayInfo {
    pub id: u32,
    pub points: (i64, i64),
    pub pixels: (usize, usize),
    /// Place on the desktop in points (the main display's top-left is 0,0).
    pub frame: rec_core::state::Rect,
    pub display: Retained<SCDisplay>,
}

impl DisplayInfo {
    /// Pixels per point (2 on Retina).
    pub fn scale(&self) -> f64 {
        self.pixels.0 as f64 / self.points.0.max(1) as f64
    }
}

fn rect(r: objc2_core_foundation::CGRect) -> rec_core::state::Rect {
    rec_core::state::Rect { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height }
}

fn shareable_content() -> Result<Retained<SCShareableContent>> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |content: *mut SCShareableContent, err: *mut NSError| {
        let r = if let Some(c) = unsafe { Retained::retain(content) } {
            Ok(SendBox(c))
        } else {
            let msg = unsafe { err.as_ref() }
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "unknown error".into());
            Err(msg)
        };
        let _ = tx.send(r);
    });
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(c)) => Ok(c.0),
        Ok(Err(e)) => bail!(
            "ScreenCaptureKit refused: {e}\n  -> grant Screen & System Audio Recording to Rec (see `rec permissions`)"
        ),
        Err(_) => bail!("timed out waiting for ScreenCaptureKit shareable content"),
    }
}

/// The main display's size in pixels, from CoreGraphics (works without the
/// Screen Recording grant). 1920x1080 if it can't be read.
pub fn main_display_pixels() -> (usize, usize) {
    let mode = CGDisplayCopyDisplayMode(objc2_core_graphics::CGMainDisplayID());
    let px = (CGDisplayMode::pixel_width(mode.as_deref()), CGDisplayMode::pixel_height(mode.as_deref()));
    if px.0 == 0 || px.1 == 0 { (1920, 1080) } else { px }
}

pub fn displays() -> Result<Vec<DisplayInfo>> {
    let content = shareable_content()?;
    let mut out = vec![];
    for d in unsafe { content.displays() }.iter() {
        let id = unsafe { d.displayID() };
        let mode = CGDisplayCopyDisplayMode(id);
        let pixels = (
            CGDisplayMode::pixel_width(mode.as_deref()),
            CGDisplayMode::pixel_height(mode.as_deref()),
        );
        let frame = rect(unsafe { d.frame() });
        out.push(DisplayInfo { id, points: unsafe { (d.width() as i64, d.height() as i64) }, pixels, frame, display: d });
    }
    Ok(out)
}

pub struct WindowInfo {
    pub id: u32,
    pub app: String,
    /// The owning app's bundle identifier ("" if unknown).
    pub bundle_id: String,
    pub title: String,
    /// Size in points.
    pub size: (f64, f64),
    /// Place on the desktop in points (as `DisplayInfo::frame`).
    pub frame: rec_core::state::Rect,
    /// The owning process (0 if unknown).
    pub pid: i32,
    pub window: Retained<SCWindow>,
}

/// On-screen, normal-layer windows that have a title or an owning app, front
/// to back (ScreenCaptureKit's order). This process's own windows are left
/// out: they are never recorded.
pub fn windows() -> Result<Vec<WindowInfo>> {
    windows_in(&*shareable_content()?)
}

fn windows_in(content: &SCShareableContent) -> Result<Vec<WindowInfo>> {
    let me = std::process::id() as i32;
    let mut out = vec![];
    for w in unsafe { content.windows() }.iter() {
        unsafe {
            if !w.isOnScreen() || w.windowLayer() != 0 {
                continue;
            }
            let owner = w.owningApplication();
            let pid = owner.as_ref().map(|a| a.processID()).unwrap_or(0);
            if pid == me {
                continue;
            }
            let app = owner.as_ref().map(|a| a.applicationName().to_string()).unwrap_or_default();
            let bundle_id = owner.as_ref().map(|a| a.bundleIdentifier().to_string()).unwrap_or_default();
            let title = w.title().map(|t| t.to_string()).unwrap_or_default();
            let f = w.frame();
            if (app.is_empty() && title.is_empty()) || f.size.width < 40.0 || f.size.height < 40.0 {
                continue;
            }
            out.push(WindowInfo { id: w.windowID(), app, bundle_id, title, size: (f.size.width, f.size.height), frame: rect(f), pid, window: w });
        }
    }
    Ok(out)
}

/// Find a window by numeric ID, or case-insensitive substring of
/// "<app> <title>". Prefers the largest match.
pub fn find_window(sel: &str) -> Result<WindowInfo> {
    let ws = windows()?;
    if let Ok(id) = sel.parse::<u32>() {
        if let Some(w) = ws.into_iter().find(|w| w.id == id) {
            return Ok(w);
        }
        bail!("no on-screen window with id {id} (see `rec-rs devices`)");
    }
    let lower = sel.to_lowercase();
    ws.into_iter()
        .filter(|w| format!("{} {}", w.app, w.title).to_lowercase().contains(&lower))
        .max_by(|a, b| (a.size.0 * a.size.1).total_cmp(&(b.size.0 * b.size.1)))
        .ok_or_else(|| anyhow!("no on-screen window matches {sel:?} (see `rec-rs devices`)"))
}

/// A running app to record, and where its windows are.
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub app: Retained<SCRunningApplication>,
    /// The app's on-screen windows, front to back (may be empty).
    pub windows: Vec<WindowInfo>,
}

/// Find a running app by bundle ID (exact, any case) or by a case-insensitive
/// substring of its name. Prefers an app with on-screen windows. Never this
/// process: our own windows are never recorded.
pub fn find_app(sel: &str) -> Result<AppInfo> {
    let content = shareable_content()?;
    let me = std::process::id() as i32;
    let lower = sel.trim().to_lowercase();
    let wins = windows_in(&*content)?;
    let mut best: Option<AppInfo> = None;
    for a in unsafe { content.applications() }.iter() {
        let (pid, name, bundle_id) = unsafe {
            (a.processID(), a.applicationName().to_string(), a.bundleIdentifier().to_string())
        };
        let hit = bundle_id.to_lowercase() == lower || (!name.is_empty() && name.to_lowercase().contains(&lower));
        if !hit {
            continue;
        }
        if pid == me {
            bail!("{name} is this app; its own windows are never recorded");
        }
        let windows: Vec<WindowInfo> = wins.iter().filter(|w| w.pid == pid).map(WindowInfo::clone_ref).collect();
        let exact = bundle_id.to_lowercase() == lower;
        let better = match &best {
            None => true,
            Some(b) => {
                let b_exact = b.bundle_id.to_lowercase() == lower;
                (exact, !windows.is_empty()) > (b_exact, !b.windows.is_empty())
            }
        };
        if better {
            best = Some(AppInfo { name, bundle_id, app: a, windows });
        }
    }
    best.ok_or_else(|| anyhow!("no running app matches {sel:?} (see `rec-rs devices`)"))
}

impl WindowInfo {
    fn clone_ref(&self) -> WindowInfo {
        WindowInfo {
            id: self.id,
            app: self.app.clone(),
            bundle_id: self.bundle_id.clone(),
            title: self.title.clone(),
            size: self.size,
            frame: self.frame,
            pid: self.pid,
            window: self.window.clone(),
        }
    }
}

/// Index into `displays` of the display holding most of `frame` (desktop points).
pub fn display_index_of(frame: &rec_core::state::Rect, displays: &[DisplayInfo]) -> Option<usize> {
    let frames: Vec<_> = displays.iter().map(|d| d.frame).collect();
    rec_core::state::display_index_for(frame, &frames)
}

pub enum Target {
    /// A display, optionally cropped to a region (points, top-left origin).
    Display(DisplayInfo, Option<objc2_core_foundation::CGRect>),
    Window(WindowInfo),
    /// Every window of one app on this display, including windows it opens later.
    App(DisplayInfo, AppInfo),
}

impl Target {
    pub fn describe(&self) -> String {
        match self {
            Target::Display(d, None) => format!("display {}", d.id),
            Target::Display(d, Some(r)) => format!(
                "display {} region {}x{}+{}+{}",
                d.id, r.size.width, r.size.height, r.origin.x, r.origin.y
            ),
            Target::Window(w) => format!("window \"{} {}\" ({})", w.app, w.title, w.id),
            Target::App(d, a) => format!("app {} ({}) on display {}", a.name, a.bundle_id, d.id),
        }
    }
}

pub struct ScreenConfig {
    pub target: Target,
    pub width: usize,
    pub height: usize,
    pub fps: u32,
    pub pixel_format: u32,
    pub cursor: bool,
    pub clicks: bool,
    pub system_audio: bool,
    /// None = no mic; Some(None) = default mic; Some(Some(id)) = specific device.
    pub mic: Option<Option<String>>,
}

pub struct Ivars {
    shared: Arc<Shared>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RecScreenOutput"]
    #[ivars = Ivars]
    pub struct ScreenOutput;

    unsafe impl NSObjectProtocol for ScreenOutput {}

    unsafe impl SCStreamOutput for ScreenOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        unsafe fn did_output(&self, _stream: &SCStream, sb: &CMSampleBuffer, kind: SCStreamOutputType) {
            self.handle(sb, kind);
        }
    }

    unsafe impl SCStreamDelegate for ScreenOutput {
        #[unsafe(method(stream:didStopWithError:))]
        unsafe fn did_stop(&self, _stream: &SCStream, error: &NSError) {
            let msg = format!("screen capture stopped: {} [{} {}]", error.localizedDescription(), error.domain(), error.code());
            self.ivars().shared.fail(msg);
        }
    }
);

impl ScreenOutput {
    fn create(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(Ivars { shared });
        unsafe { msg_send![super(this), init] }
    }

    fn handle(&self, sb: &CMSampleBuffer, kind: SCStreamOutputType) {
        unsafe {
            if !sb.is_valid() {
                return;
            }
        }
        let shared = &self.ivars().shared;
        if kind == SCStreamOutputType::Screen {
            if !frame_complete(sb) {
                return;
            }
            let Some(pb) = (unsafe { sb.image_buffer() }) else { return };
            let pts: CMTime = unsafe { sb.presentation_time_stamp() };
            let pb: objc2_core_foundation::CFRetained<CVPixelBuffer> = pb;
            shared.with(|s| s.push_screen(pts, pb));
        } else if kind == SCStreamOutputType::Audio {
            shared.with(|s| s.push_audio(AudioKind::System, sb));
        } else if kind == SCStreamOutputType::Microphone {
            shared.with(|s| s.push_audio(AudioKind::Mic, sb));
        }
    }
}

/// Only `Complete` frames carry new pixels; idle/blank/suspended do not.
fn frame_complete(sb: &CMSampleBuffer) -> bool {
    unsafe {
        let Some(arr) = sb.sample_attachments_array(false) else { return false };
        let arr: &NSArray<NSDictionary<NSString, AnyObject>> =
            &*(&*arr as *const objc2_core_foundation::CFArray as *const NSArray<NSDictionary<NSString, AnyObject>>);
        let Some(info) = arr.firstObject() else { return false };
        let Some(status) = info.objectForKey(SCStreamFrameInfoStatus) else { return false };
        let Ok(n) = status.downcast::<NSNumber>() else { return false };
        n.integerValue() == SCFrameStatus::Complete.0
    }
}

pub struct ScreenCapture {
    stream: Retained<SCStream>,
    _output: Retained<ScreenOutput>,
    _queues: Vec<dispatch2::DispatchRetained<DispatchQueue>>,
}

/// Set by GUI hosts (the Cortex app): their windows must never be recorded,
/// so a capture that can't exclude them fails instead of starting.
static REQUIRE_SELF_EXCLUSION: AtomicBool = AtomicBool::new(false);

/// Call once from an app that shows windows on screen. The CLI has none to
/// hide, so it leaves this off and records even when ScreenCaptureKit
/// doesn't list the process.
pub fn require_self_exclusion(on: bool) {
    REQUIRE_SELF_EXCLUSION.store(on, Ordering::SeqCst);
}

/// What to do about our own windows, given whether ScreenCaptureKit listed
/// this process on the first look and (if it was needed) after a refresh.
#[derive(Debug, PartialEq, Eq)]
enum SelfExclusion {
    /// Found: exclude it.
    Exclude,
    /// Not listed, but there is nothing that must be hidden (CLI).
    NotNeeded,
    /// Not listed after a refresh, and our windows must stay out.
    Fail,
    /// Not listed on the first look: refresh the content and look again.
    Retry,
}

fn self_exclusion(first: bool, after_refresh: Option<bool>, required: bool) -> SelfExclusion {
    match (first, after_refresh) {
        (true, _) | (false, Some(true)) => SelfExclusion::Exclude,
        (false, None) => SelfExclusion::Retry,
        (false, Some(false)) if required => SelfExclusion::Fail,
        (false, Some(false)) => SelfExclusion::NotNeeded,
    }
}

/// This process's entry in ScreenCaptureKit's application list, looked up
/// twice (a just-opened window can be missing from the first snapshot).
/// Errors when a GUI host's windows can't be excluded.
fn own_application() -> Result<Option<Retained<SCRunningApplication>>> {
    let me = std::process::id() as i32;
    let required = REQUIRE_SELF_EXCLUSION.load(Ordering::SeqCst);
    let find = || -> Result<Option<Retained<SCRunningApplication>>> {
        let c = shareable_content();
        if required {
            Ok(unsafe { c?.applications() }.iter().find(|a| unsafe { a.processID() } == me))
        } else {
            Ok(c.ok().and_then(|c| unsafe { c.applications() }.iter().find(|a| unsafe { a.processID() } == me)))
        }
    };
    let first = find()?;
    let mut after = None;
    loop {
        match self_exclusion(first.is_some(), after.as_ref().map(Option::is_some), required) {
            SelfExclusion::Exclude => return Ok(first.or_else(|| after.flatten())),
            SelfExclusion::NotNeeded => return Ok(None),
            SelfExclusion::Fail => bail!(
                "could not hide this app's own windows from the recording (ScreenCaptureKit did not list the app), so recording did not start; try again"
            ),
            SelfExclusion::Retry => {
                std::thread::sleep(Duration::from_millis(100));
                after = Some(find()?);
            }
        }
    }
}

#[cfg(test)]
mod self_exclusion_tests {
    use super::{SelfExclusion::*, self_exclusion};

    #[test]
    fn own_windows_are_excluded_or_the_capture_fails() {
        assert_eq!(self_exclusion(true, None, true), Exclude);
        assert_eq!(self_exclusion(false, None, true), Retry);
        assert_eq!(self_exclusion(false, Some(true), true), Exclude);
        // A GUI host never silently records its own windows.
        assert_eq!(self_exclusion(false, Some(false), true), Fail);
        // The CLI has no windows: record without the exclusion.
        assert_eq!(self_exclusion(false, None, false), Retry);
        assert_eq!(self_exclusion(false, Some(false), false), NotNeeded);
    }
}

unsafe impl Send for ScreenCapture {}

impl ScreenCapture {
    pub fn start(cfg: &ScreenConfig, shared: Arc<Shared>) -> Result<Self> {
        unsafe {
            let conf = SCStreamConfiguration::new();
            let filter = match &cfg.target {
                Target::Display(d, region) => {
                    if let Some(r) = region {
                        conf.setSourceRect(*r);
                    }
                    // Exclude our own windows (the preview, HUD, toasts),
                    // which would otherwise be recorded as a hall of mirrors.
                    let own = own_application()?;
                    if std::env::var_os("REC_DEBUG").is_some() {
                        eprintln!("rec: debug: excluding own app from capture: {}", own.is_some());
                    }
                    match own {
                        Some(app) => SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                            SCContentFilter::alloc(),
                            &d.display,
                            &NSArray::from_retained_slice(&[app]),
                            &NSArray::new(),
                        ),
                        None => SCContentFilter::initWithDisplay_excludingWindows(
                            SCContentFilter::alloc(),
                            &d.display,
                            &NSArray::new(),
                        ),
                    }
                }
                Target::Window(w) => {
                    SCContentFilter::initWithDesktopIndependentWindow(SCContentFilter::alloc(), &w.window)
                }
                // Only the named app is included, so our own windows stay out
                // (`find_app` refuses this process).
                Target::App(d, a) => SCContentFilter::initWithDisplay_includingApplications_exceptingWindows(
                    SCContentFilter::alloc(),
                    &d.display,
                    &NSArray::from_retained_slice(std::slice::from_ref(&a.app)),
                    &NSArray::new(),
                ),
            };
            conf.setWidth(cfg.width);
            conf.setHeight(cfg.height);
            // Slightly under the frame period, so capture jitter never makes
            // ScreenCaptureKit skip a frame the pacer needs (as OBS does).
            conf.setMinimumFrameInterval(CMTime::new(9, cfg.fps as i32 * 10));
            conf.setPixelFormat(cfg.pixel_format);
            conf.setQueueDepth(8);
            conf.setShowsCursor(cfg.cursor);
            conf.setShowMouseClicks(cfg.clicks);
            conf.setScalesToFit(true);
            conf.setPreservesAspectRatio(true);
            conf.setCaptureResolution(SCCaptureResolutionType::Best);
            conf.setColorSpaceName(kCGColorSpaceSRGB);
            if cfg.pixel_format == PIX_420V {
                conf.setColorMatrix(kCGDisplayStreamYCbCrMatrix_ITU_R_709_2);
            }
            conf.setCapturesAudio(cfg.system_audio);
            conf.setSampleRate(48000);
            conf.setChannelCount(2);
            conf.setExcludesCurrentProcessAudio(true);
            if let Some(dev) = &cfg.mic {
                conf.setCaptureMicrophone(true);
                if let Some(id) = dev {
                    conf.setMicrophoneCaptureDeviceID(Some(&NSString::from_str(id)));
                }
            }

            let output = ScreenOutput::create(shared);
            let delegate = ProtocolObject::from_ref(&*output);
            let stream = SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &conf, Some(delegate));
            let out_proto: &ProtocolObject<dyn SCStreamOutput> = ProtocolObject::from_ref(&*output);
            let vq = DispatchQueue::new("rec.screen.video", None);
            let aq = DispatchQueue::new("rec.screen.audio", None);
            let mq = DispatchQueue::new("rec.screen.mic", None);
            stream
                .addStreamOutput_type_sampleHandlerQueue_error(out_proto, SCStreamOutputType::Screen, Some(&vq))
                .map_err(|e| anyhow!("add screen output: {}", e.localizedDescription()))?;
            if cfg.system_audio {
                stream
                    .addStreamOutput_type_sampleHandlerQueue_error(out_proto, SCStreamOutputType::Audio, Some(&aq))
                    .map_err(|e| anyhow!("add audio output: {}", e.localizedDescription()))?;
            }
            if cfg.mic.is_some() {
                stream
                    .addStreamOutput_type_sampleHandlerQueue_error(out_proto, SCStreamOutputType::Microphone, Some(&mq))
                    .map_err(|e| anyhow!("add microphone output: {}", e.localizedDescription()))?;
            }
            wait_err(|b| stream.startCaptureWithCompletionHandler(Some(b)))
                .context("starting screen capture")?;
            Ok(ScreenCapture { stream, _output: output, _queues: vec![vq, aq, mq] })
        }
    }

    pub fn stop(&self) -> Result<()> {
        unsafe { wait_err(|b| self.stream.stopCaptureWithCompletionHandler(Some(b))) }
    }
}

fn wait_err(f: impl FnOnce(&block2::DynBlock<dyn Fn(*mut NSError)>)) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |err: *mut NSError| {
        let r = match unsafe { err.as_ref() } {
            None => Ok(()),
            Some(e) => Err(format!("{} [{} {}]", e.localizedDescription(), e.domain(), e.code())),
        };
        let _ = tx.send(r);
    });
    f(&block);
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => bail!("{e}"),
        Err(_) => bail!("timed out"),
    }
}

/// Icons already made, by bundle ID (None = the app had no readable icon).
static ICONS: std::sync::Mutex<Option<std::collections::HashMap<String, Option<String>>>> = std::sync::Mutex::new(None);

/// A running app's icon as a 64 px PNG `data:` URL, cached per bundle ID.
/// None when no running app has this bundle ID or its icon can't be read.
pub fn app_icon(bundle_id: &str) -> Option<String> {
    if let Some(hit) = ICONS.lock().ok()?.get_or_insert_default().get(bundle_id) {
        return hit.clone();
    }
    let icon = read_app_icon(bundle_id);
    // A running app's icon doesn't change; one that isn't running yet may start later.
    if icon.is_some() {
        ICONS.lock().ok()?.get_or_insert_default().insert(bundle_id.into(), icon.clone());
    }
    icon
}

fn read_app_icon(bundle_id: &str) -> Option<String> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSRunningApplication};
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle_id));
    let image = apps.firstObject()?.icon()?;
    let mut rect = objc2_foundation::NSRect::new(objc2_foundation::NSPoint::new(0.0, 0.0), objc2_foundation::NSSize::new(64.0, 64.0));
    // SAFETY: `rect` is a valid, writable NSRect for the call; no context or hints are passed.
    let cg = unsafe { image.CGImageForProposedRect_context_hints(&mut rect, None, None) }?;
    let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg);
    // SAFETY: an empty properties dictionary is valid for the PNG encoder.
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }?;
    Some(format!("data:image/png;base64,{}", base64(&png.to_vec())))
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { A[(n >> shift) as usize & 63] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod icon_tests {
    #[test]
    fn base64_matches_rfc4648() {
        for (raw, enc) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(super::base64(raw.as_bytes()), enc);
        }
    }
}
