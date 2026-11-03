//! SOURCE OF TRUTH KEYWORDS: Camera, MFEnumDeviceSources, IMFSourceReader, RGB32, list_cameras, symbolic link, camera thread, pick_camera_format
//! WHAT:  Cameras through Media Foundation: the list (friendly name + symbolic link as the
//!        id) and a reader thread that opens one at the format `pick_camera_format` picks,
//!        asks the source reader for RGB32, and hands each frame to a callback as
//!        top-down BGRX rows.
//! WHY:   Media Foundation is the standard camera path on Windows. RGB32 out of the source
//!        reader (`MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING` converts NV12/YUY2/MJPG) keeps
//!        the overlay renderer simple; overlay frames are small, so the CPU copy is cheap.
//! WHERE: `win::session` (overlay and camera-only scene), `win::devices` (camera list).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use rec_core::state::InputDevice;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::core::{GUID, Interface, PWSTR};

use super::util::{com_init, mf_startup, msg};
use crate::devices::{CamFormat, pick_camera_format};

/// A camera frame: `width`x`height`, top-down rows of `width * 4` bytes (B, G, R, X).
pub struct CamFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub type OnFrame = Box<dyn FnMut(CamFrame) + Send>;

fn activates() -> Vec<IMFActivate> {
    let _ = mf_startup();
    let mut out = vec![];
    // SAFETY: attribute set-up, then enumeration with valid out-pointers; every entry is
    // taken (owned) and the array freed.
    unsafe {
        let mut attrs = None;
        if MFCreateAttributes(&mut attrs, 1).is_err() {
            return out;
        }
        let Some(attrs) = attrs else { return out };
        if attrs
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .is_err()
        {
            return out;
        }
        let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        if MFEnumDeviceSources(&attrs, &mut list, &mut count).is_err() || list.is_null() {
            return out;
        }
        for i in 0..count as usize {
            if let Some(a) = (*list.add(i)).take() {
                out.push(a);
            }
        }
        CoTaskMemFree(Some(list as *const _));
    }
    out
}

fn string_attr(a: &IMFActivate, key: &GUID) -> Option<String> {
    let mut p = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: GetAllocatedString returns a CoTaskMem string we free after copying.
    unsafe {
        a.GetAllocatedString(key, &mut p, &mut len).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// The cameras plugged in now; the id is the device's symbolic link.
pub fn list_cameras() -> Vec<InputDevice> {
    com_init();
    activates()
        .iter()
        .filter_map(|a| {
            Some(InputDevice {
                id: string_attr(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)?,
                name: string_attr(a, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                    .unwrap_or_else(|| "Camera".into()),
                connected: true,
            })
        })
        .collect()
}

fn unpack(v: u64) -> (u32, u32) {
    ((v >> 32) as u32, v as u32)
}

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

/// Open `id` (or the first camera) at the best format up to `max_height`.
fn open(id: Option<&str>, max_height: u32) -> Result<(IMFSourceReader, (u32, u32)), String> {
    let all = activates();
    let a = id
        .and_then(|id| {
            all.iter().find(|a| {
                string_attr(a, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK).as_deref()
                    == Some(id)
            })
        })
        .or(all.first())
        .ok_or_else(|| "No camera found".to_string())?;
    // SAFETY: Media Foundation calls on live objects with valid pointers.
    unsafe {
        let source: IMFMediaSource = a.ActivateObject().map_err(|e| msg("Camera", &e))?;
        let mut attrs = None;
        MFCreateAttributes(&mut attrs, 1).map_err(|e| msg("Camera", &e))?;
        let attrs = attrs.ok_or("Camera: no attributes")?;
        attrs
            .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .map_err(|e| msg("Camera", &e))?;
        let reader =
            MFCreateSourceReaderFromMediaSource(&source, &attrs).map_err(|e| msg("Camera", &e))?;
        // The native formats, then the best one under the cap.
        let mut types = vec![];
        let mut formats = vec![];
        let mut i = 0;
        while let Ok(t) = reader.GetNativeMediaType(VIDEO, i) {
            let (w, h) = unpack(t.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0));
            let (n, d) = unpack(t.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(0));
            formats.push(CamFormat {
                width: w,
                height: h,
                fps: if d > 0 {
                    f64::from(n) / f64::from(d)
                } else {
                    0.0
                },
            });
            types.push(t);
            i += 1;
        }
        if let Some(best) = pick_camera_format(&formats, max_height).and_then(|i| types.get(i)) {
            let _ = reader.SetCurrentMediaType(VIDEO, None, best);
        }
        let out = MFCreateMediaType().map_err(|e| msg("Camera", &e))?;
        out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|e| msg("Camera", &e))?;
        out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)
            .map_err(|e| msg("Camera", &e))?;
        reader
            .SetCurrentMediaType(VIDEO, None, &out)
            .map_err(|e| msg("The camera can't deliver RGB frames", &e))?;
        let cur = reader
            .GetCurrentMediaType(VIDEO)
            .map_err(|e| msg("Camera", &e))?;
        let size = unpack(cur.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0));
        if size.0 == 0 || size.1 == 0 {
            return Err("The camera reported no frame size".into());
        }
        Ok((reader, size))
    }
}

/// Copy a sample's RGB32 pixels into top-down rows.
fn read_frame(sample: &IMFSample, (w, h): (u32, u32)) -> Option<CamFrame> {
    let row = w as usize * 4;
    let mut data = vec![0u8; row * h as usize];
    // SAFETY: the buffer is locked while its rows are copied; pitch and scanline come from
    // Lock2D (a negative pitch is a bottom-up image).
    unsafe {
        let buf = sample.ConvertToContiguousBuffer().ok()?;
        if let Ok(b2) = buf.cast::<IMF2DBuffer>() {
            let mut p: *mut u8 = std::ptr::null_mut();
            let mut pitch = 0i32;
            b2.Lock2D(&mut p, &mut pitch).ok()?;
            if !p.is_null() {
                for y in 0..h as isize {
                    let src = p.offset(y * pitch as isize);
                    std::ptr::copy_nonoverlapping(
                        src,
                        data.as_mut_ptr().add(y as usize * row),
                        row,
                    );
                }
            }
            let _ = b2.Unlock2D();
        } else {
            let mut p: *mut u8 = std::ptr::null_mut();
            let mut len = 0u32;
            buf.Lock(&mut p, None, Some(&mut len)).ok()?;
            if !p.is_null() {
                let n = (len as usize).min(data.len());
                std::ptr::copy_nonoverlapping(p, data.as_mut_ptr(), n);
            }
            let _ = buf.Unlock();
        }
    }
    Some(CamFrame {
        width: w,
        height: h,
        data,
    })
}

/// A running camera.
pub struct Camera {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub size: (u32, u32),
}

impl Camera {
    /// Open the camera and start delivering frames to `on_frame` on its own thread.
    pub fn start(
        id: Option<String>,
        max_height: u32,
        mut on_frame: OnFrame,
    ) -> Result<Camera, String> {
        mf_startup()?;
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<(u32, u32), String>>();
        let thread = std::thread::Builder::new()
            .name("mf-camera".into())
            .spawn(move || {
                com_init();
                let (reader, size) = match open(id.as_deref(), max_height) {
                    Ok(x) => x,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                };
                let _ = tx.send(Ok(size));
                while !s.load(Ordering::SeqCst) {
                    let mut flags = 0u32;
                    let mut sample: Option<IMFSample> = None;
                    // SAFETY: synchronous read with valid out-pointers.
                    let r = unsafe {
                        reader.ReadSample(VIDEO, 0, None, Some(&mut flags), None, Some(&mut sample))
                    };
                    if r.is_err()
                        || flags
                            & (MF_SOURCE_READERF_ERROR.0 as u32
                                | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)
                            != 0
                    {
                        break;
                    }
                    if let Some(f) = sample.as_ref().and_then(|s| read_frame(s, size)) {
                        on_frame(f);
                    }
                }
            })
            .map_err(|e| format!("Could not start the camera: {e}"))?;
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(size)) => Ok(Camera {
                stop,
                thread: Some(thread),
                size,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                Err("The camera did not start".into())
            }
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
