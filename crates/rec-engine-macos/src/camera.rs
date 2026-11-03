//! Webcam capture (AVCaptureSession) and device enumeration.

use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_av_foundation::*;
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::kCVPixelBufferPixelFormatTypeKey;
use objc2_foundation::{NSArray, NSObject, NSObjectProtocol};
use dispatch2::DispatchQueue;

use crate::record::Shared;
use crate::util::*;

pub struct Device {
    pub id: String,
    pub name: String,
}

fn discover(types: &[&AVCaptureDeviceType], media: &AVMediaType) -> Vec<Retained<AVCaptureDevice>> {
    unsafe {
        let arr = NSArray::from_slice(types);
        let s = AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
            &arr,
            Some(media),
            AVCaptureDevicePosition::Unspecified,
        );
        s.devices().iter().collect()
    }
}

fn cameras_raw() -> Vec<Retained<AVCaptureDevice>> {
    unsafe {
        discover(
            &[
                AVCaptureDeviceTypeBuiltInWideAngleCamera,
                AVCaptureDeviceTypeExternal,
                AVCaptureDeviceTypeContinuityCamera,
            ],
            AVMediaTypeVideo.unwrap(),
        )
    }
}

fn describe(d: &AVCaptureDevice) -> Device {
    unsafe { Device { id: d.uniqueID().to_string(), name: d.localizedName().to_string() } }
}

pub fn cameras() -> Vec<Device> {
    cameras_raw().iter().map(|d| describe(d)).collect()
}

pub fn microphones() -> Vec<Device> {
    unsafe {
        discover(&[AVCaptureDeviceTypeMicrophone], AVMediaTypeAudio.unwrap())
            .iter()
            .map(|d| describe(d))
            .collect()
    }
}

/// A saved or requested mic resolved against the connected devices.
pub struct MicPick {
    /// Unique ID to open; None = the system default mic.
    pub id: Option<String>,
    /// Why the requested mic wasn't used (it is not connected).
    pub warning: Option<String>,
}

/// Resolve a mic selector without failing: by unique ID, then by name, then
/// the system default with a warning. Audio never blocks a session.
pub fn pick_mic(sel: &str, name: Option<&str>) -> MicPick {
    let mics = microphones();
    match match_mic(&mics, sel, name) {
        Some(id) => MicPick { id: Some(id), warning: None },
        None => {
            let label = name.filter(|n| !n.is_empty()).unwrap_or(sel);
            MicPick { id: None, warning: Some(format!("Microphone \u{201c}{label}\u{201d} not connected; using the default mic")) }
        }
    }
}

fn match_mic(mics: &[Device], sel: &str, name: Option<&str>) -> Option<String> {
    // Exact unique ID.
    if let Some(m) = mics.iter().find(|m| m.id == sel) {
        return Some(m.id.clone());
    }
    // USB unique IDs end in a port/instance index (`…:SERIAL:4`) that can
    // change between plugs; match on the rest.
    let stem = |id: &str| id.rsplit_once(':').filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit())).map(|(s, _)| s.to_string());
    if let Some(st) = stem(sel)
        && let Some(m) = mics.iter().find(|m| stem(&m.id).as_deref() == Some(st.as_str()))
    {
        return Some(m.id.clone());
    }
    // Saved display name (exact, then substring), then the selector as a name.
    for n in name.into_iter().chain([sel]).filter(|n| !n.is_empty()) {
        let lower = n.to_lowercase();
        if let Some(m) = mics.iter().find(|m| m.name.to_lowercase() == lower) {
            return Some(m.id.clone());
        }
        if let Some(m) = mics.iter().find(|m| m.name.to_lowercase().contains(&lower)) {
            return Some(m.id.clone());
        }
    }
    None
}

/// Call `f` on the main queue whenever a capture device is connected or
/// disconnected. The observers live for the process.
pub fn on_device_change(f: impl Fn() + Clone + 'static) {
    use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
    unsafe {
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        for name in [AVCaptureDeviceWasConnectedNotification, AVCaptureDeviceWasDisconnectedNotification] {
            let f = f.clone();
            let block = block2::RcBlock::new(move |_n: std::ptr::NonNull<NSNotification>| f());
            let token = center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&queue), &block);
            std::mem::forget(token);
        }
    }
}

/// SOURCE OF TRUTH KEYWORDS: FormatInfo, pick_format, camera resolution, 1080p, 720p, activeFormat
/// WHAT:  One capture format of a camera: its size and best frame rate.
/// WHY:   The capture format is chosen from the device's own list, so the
///        choice is a pure function that tests can cover.
/// WHERE: `Camera::start`, `Camera::planned_size`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormatInfo {
    pub width: usize,
    pub height: usize,
    pub max_fps: f64,
}

/// SOURCE OF TRUTH KEYWORDS: pick_format, camera format, cameraResolution, 1080p fallback, NEW-CT-03
/// WHAT:  The format to capture at for a `max_height` (720 or 1080).
/// WHY:   1080p when the camera has it, falling back to the largest smaller
///        format, so the camera track is as sharp as the setting allows.
///        Formats under 24 fps (some 4:3 stills-like modes) are used only
///        when nothing else fits; ties go to the higher frame rate.
/// WHERE: `Camera::start`, `Camera::planned_size`.
pub fn pick_format(formats: &[FormatInfo], max_height: usize) -> Option<usize> {
    let best = |min_fps: f64| {
        formats
            .iter()
            .enumerate()
            .filter(|(_, f)| f.height <= max_height && f.max_fps >= min_fps)
            .max_by(|(_, a), (_, b)| {
                (a.width * a.height).cmp(&(b.width * b.height)).then(a.max_fps.total_cmp(&b.max_fps))
            })
            .map(|(i, _)| i)
    };
    best(24.0).or_else(|| best(0.0)).or_else(|| {
        // Nothing small enough: the smallest format there is.
        formats.iter().enumerate().min_by_key(|(_, f)| f.width * f.height).map(|(i, _)| i)
    })
}

fn format_size(f: &AVCaptureDeviceFormat) -> (usize, usize) {
    let d = unsafe { objc2_core_media::CMVideoFormatDescriptionGetDimensions(&f.formatDescription()) };
    (d.width.max(0) as usize, d.height.max(0) as usize)
}

fn formats_of(dev: &AVCaptureDevice) -> Vec<(Retained<AVCaptureDeviceFormat>, FormatInfo)> {
    unsafe {
        dev.formats()
            .iter()
            .map(|f| {
                let (width, height) = format_size(&f);
                let max_fps = f.videoSupportedFrameRateRanges().iter().map(|r| r.maxFrameRate()).fold(0.0, f64::max);
                (f, FormatInfo { width, height, max_fps })
            })
            .collect()
    }
}

pub struct CamIvars {
    /// Where frames go; switched when the camera moves to another session.
    shared: std::sync::Mutex<Arc<Shared>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RecCameraOutput"]
    #[ivars = CamIvars]
    pub struct CameraOutput;

    unsafe impl NSObjectProtocol for CameraOutput {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for CameraOutput {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn did_output(&self, _o: &AVCaptureOutput, sb: &CMSampleBuffer, _c: &AVCaptureConnection) {
            unsafe {
                if let Some(pb) = sb.image_buffer() {
                    // REC_DEBUG: the camera's color tags (once).
                    static LOGGED: std::sync::Once = std::sync::Once::new();
                    if std::env::var_os("REC_DEBUG").is_some() {
                        LOGGED.call_once(|| {
                            let a = pb.attachments(objc2_core_video::CVAttachmentMode::ShouldPropagate);
                            eprintln!("rec: debug: camera frame attachments: {a:?}");
                        });
                    }
                    let pts = sb.presentation_time_stamp();
                    let shared = self.ivars().shared.lock().unwrap().clone();
                    shared.with(|s| s.push_cam(pts, pb));
                }
            }
        }
    }
);

pub struct Camera {
    session: Retained<AVCaptureSession>,
    out: Retained<CameraOutput>,
    _queue: dispatch2::DispatchRetained<DispatchQueue>,
    pub name: String,
    /// The device's unique ID.
    pub id: String,
    /// Frame size delivered (the chosen capture format).
    pub size: (usize, usize),
    /// The `max_height` it was started with.
    pub max_height: usize,
    /// When it started: auto-exposure and white balance settle over the
    /// first second or two, so a fresh camera looks dark at first.
    pub started: std::time::Instant,
}

unsafe impl Send for Camera {}

impl Camera {
    /// The device `sel` names: None = the system's preferred camera, else a
    /// unique ID or name substring.
    fn device(sel: Option<&str>) -> Result<Retained<AVCaptureDevice>> {
        let cams = cameras_raw();
        match sel {
            // System-preferred camera (what FaceTime would use), else the
            // built-in camera, else anything.
            None => unsafe { AVCaptureDevice::systemPreferredCamera() }
                .or_else(|| {
                    cams.iter()
                        .find(|d| unsafe { d.deviceType().to_string() == AVCaptureDeviceTypeBuiltInWideAngleCamera.to_string() })
                        .cloned()
                })
                .or_else(|| cams.into_iter().next()),
            Some(s) => {
                let lower = s.to_lowercase();
                cams.into_iter().find(|d| unsafe {
                    d.uniqueID().to_string() == s || d.localizedName().to_string().to_lowercase().contains(&lower)
                })
            }
        }
        .ok_or_else(|| anyhow!("no camera found (see `rec-rs devices`)"))
    }

    /// The unique ID of the camera `sel` names (see `device`).
    pub fn resolve(sel: Option<&str>) -> Result<String> {
        Ok(unsafe { Self::device(sel)?.uniqueID().to_string() })
    }

    /// Send frames to another session's sink (the camera keeps running,
    /// already exposed, instead of restarting).
    pub fn retarget(&self, shared: Arc<Shared>) {
        *self.out.ivars().shared.lock().unwrap() = shared;
    }

    /// `sel`: None = first camera, else unique ID or name substring.
    /// `max_height`: the tallest capture format to use (720 or 1080); the
    /// best format at or below it is chosen (see `pick_format`) and the
    /// frames are delivered at exactly that size (`Camera::size`).
    pub fn start(sel: Option<&str>, max_height: usize, shared: Arc<Shared>) -> Result<Self> {
        let dev = Self::device(sel)?;
        unsafe {
            let name = dev.localizedName().to_string();
            let session = AVCaptureSession::new();
            session.beginConfiguration();
            let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&dev)
                .map_err(|e| anyhow!("camera {name}: {}", e.localizedDescription()))?;
            if !session.canAddInput(&input) {
                bail!("cannot use camera {name}");
            }
            session.addInput(&input);
            // Pick the device format directly (the session presets stop at
            // 1280x720 on some cameras and never offer 1080p on others). The
            // lock is held until the session runs, or it may reset the format.
            let formats = formats_of(&dev);
            let infos: Vec<FormatInfo> = formats.iter().map(|f| f.1).collect();
            let chosen = pick_format(&infos, max_height).filter(|_| dev.lockForConfiguration().is_ok());
            match chosen {
                Some(i) => dev.setActiveFormat(&formats[i].0),
                None if session.canSetSessionPreset(AVCaptureSessionPreset1280x720) => {
                    session.setSessionPreset(AVCaptureSessionPreset1280x720)
                }
                None => session.setSessionPreset(AVCaptureSessionPresetHigh),
            }
            let size = match chosen {
                Some(i) => (infos[i].width, infos[i].height),
                None => format_size(&dev.activeFormat()),
            };
            let output = AVCaptureVideoDataOutput::new();
            // Frames at exactly `size`, whatever the session does, so the
            // camera track's encoder always gets the size it was opened with.
            output.setVideoSettings(Some(&dict(vec![
                (cf_ns(kCVPixelBufferPixelFormatTypeKey), num_i(PIX_BGRA as i64)),
                (cf_ns(objc2_core_video::kCVPixelBufferWidthKey), num_i(size.0 as i64)),
                (cf_ns(objc2_core_video::kCVPixelBufferHeightKey), num_i(size.1 as i64)),
            ])));
            output.setAlwaysDiscardsLateVideoFrames(true);
            let delegate = Self::make_delegate(shared);
            let queue = DispatchQueue::new("rec.camera", None);
            output.setSampleBufferDelegate_queue(Some(ProtocolObject::from_ref(&*delegate)), Some(&queue));
            if !session.canAddOutput(&output) {
                if chosen.is_some() {
                    dev.unlockForConfiguration();
                }
                bail!("cannot add camera output");
            }
            session.addOutput(&output);
            session.commitConfiguration();
            session.startRunning();
            if chosen.is_some() {
                dev.unlockForConfiguration();
            }
            if !session.isRunning() {
                bail!("camera {name} failed to start");
            }
            if std::env::var_os("REC_DEBUG").is_some() {
                eprintln!("rec: debug: camera {name}: {}x{} (max height {max_height})", size.0, size.1);
            }
            let id = dev.uniqueID().to_string();
            Ok(Camera { session, out: delegate, _queue: queue, name, id, size, max_height, started: std::time::Instant::now() })
        }
    }

    /// The frame size `start(sel, max_height)` would deliver, without
    /// opening the camera (None: no such camera).
    pub fn planned_size(sel: Option<&str>, max_height: usize) -> Option<(usize, usize)> {
        let dev = Self::device(sel).ok()?;
        let formats = formats_of(&dev);
        let infos: Vec<FormatInfo> = formats.iter().map(|f| f.1).collect();
        Some(match pick_format(&infos, max_height) {
            Some(i) => (infos[i].width, infos[i].height),
            None => unsafe { format_size(&dev.activeFormat()) },
        })
    }

    fn make_delegate(shared: Arc<Shared>) -> Retained<CameraOutput> {
        let this = CameraOutput::alloc().set_ivars(CamIvars { shared: std::sync::Mutex::new(shared) });
        unsafe { msg_send![super(this), init] }
    }

    pub fn stop(&self) {
        unsafe { self.session.stopRunning() };
    }
}

pub struct MicIvars {
    shared: Arc<Shared>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "RecMicOutput"]
    #[ivars = MicIvars]
    pub struct MicOutput;

    unsafe impl NSObjectProtocol for MicOutput {}

    unsafe impl AVCaptureAudioDataOutputSampleBufferDelegate for MicOutput {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn did_output(&self, _o: &AVCaptureOutput, sb: &CMSampleBuffer, _c: &AVCaptureConnection) {
            self.ivars().shared.with(|s| s.push_audio(crate::writer::AudioKind::Mic, sb));
        }
    }
);

/// Microphone via AVCaptureSession (fallback / test path; the default mic path
/// is ScreenCaptureKit's built-in microphone capture).
pub struct Mic {
    session: Retained<AVCaptureSession>,
    _out: Retained<MicOutput>,
    _queue: dispatch2::DispatchRetained<DispatchQueue>,
}

unsafe impl Send for Mic {}

impl Mic {
    pub fn start(id: Option<&str>, shared: Arc<Shared>) -> Result<Self> {
        unsafe {
            let media = AVMediaTypeAudio.unwrap();
            let dev = match id {
                Some(id) => AVCaptureDevice::deviceWithUniqueID(&objc2_foundation::NSString::from_str(id)),
                None => AVCaptureDevice::defaultDeviceWithMediaType(media),
            }
            .ok_or_else(|| anyhow!("microphone not found"))?;
            let session = AVCaptureSession::new();
            let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&dev)
                .map_err(|e| anyhow!("microphone: {}", e.localizedDescription()))?;
            if !session.canAddInput(&input) {
                bail!("cannot use microphone");
            }
            session.addInput(&input);
            let output = AVCaptureAudioDataOutput::new();
            let this = MicOutput::alloc().set_ivars(MicIvars { shared });
            let delegate: Retained<MicOutput> = msg_send![super(this), init];
            let queue = DispatchQueue::new("rec.mic", None);
            output.setSampleBufferDelegate_queue(Some(ProtocolObject::from_ref(&*delegate)), Some(&queue));
            if !session.canAddOutput(&output) {
                bail!("cannot add microphone output");
            }
            session.addOutput(&output);
            session.startRunning();
            Ok(Mic { session, _out: delegate, _queue: queue })
        }
    }

    pub fn stop(&self) {
        unsafe { self.session.stopRunning() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, name: &str) -> Device {
        Device { id: id.into(), name: name.into() }
    }

    fn fmt(width: usize, height: usize, max_fps: f64) -> FormatInfo {
        FormatInfo { width, height, max_fps }
    }

    #[test]
    fn camera_format_prefers_the_largest_within_the_cap() {
        // FaceTime HD 1080p-style list.
        let f = [fmt(640, 480, 30.0), fmt(1280, 720, 30.0), fmt(1920, 1080, 30.0), fmt(1440, 1080, 30.0)];
        assert_eq!(pick_format(&f, 1080), Some(2));
        assert_eq!(pick_format(&f, 720), Some(1));
        // No 1080p: fall back to 720p.
        assert_eq!(pick_format(&f[..2], 1080), Some(1));
        // Same size: the faster one; slow formats only as a last resort.
        let g = [fmt(1920, 1080, 15.0), fmt(1280, 720, 60.0), fmt(1280, 720, 30.0)];
        assert_eq!(pick_format(&g, 1080), Some(1));
        assert_eq!(pick_format(&g[..1], 1080), Some(0));
        // Only bigger formats: the smallest of them.
        assert_eq!(pick_format(&[fmt(3840, 2160, 30.0), fmt(2560, 1440, 30.0)], 1080), Some(1));
        assert_eq!(pick_format(&[], 1080), None);
    }

    #[test]
    fn mic_matching() {
        let mics = [dev("BuiltInMicrophoneDevice", "MacBook Pro Microphone"), dev("AppleUSBAudioEngine:DJI:DJIPocket3:123456789ABCDEF:2", "OsmoPocket3")];
        let dji = Some("AppleUSBAudioEngine:DJI:DJIPocket3:123456789ABCDEF:2".to_string());
        // Same device on another port.
        assert_eq!(match_mic(&mics, "AppleUSBAudioEngine:DJI:DJIPocket3:123456789ABCDEF:4", None), dji);
        // By saved name.
        assert_eq!(match_mic(&mics, "gone-id", Some("OsmoPocket3")), dji);
        // By name substring (CLI).
        assert_eq!(match_mic(&mics, "macbook", None).as_deref(), Some("BuiltInMicrophoneDevice"));
        assert_eq!(match_mic(&mics[..1], "AppleUSBAudioEngine:DJI:DJIPocket3:123456789ABCDEF:4", Some("OsmoPocket3")), None);
    }
}
