//! SOURCE OF TRUTH KEYWORDS: WinMedia, MediaLibrary, inspect, IMFSourceReader, MF_PD_DURATION, Recycle Bin, SHFileOperationW, FOF_ALLOWUNDO
//! WHAT:  The recordings library backend on Windows: `inspect` reads duration, size, frame
//!        rate and tracks with a Media Foundation source reader; `trash` moves a file to
//!        the Recycle Bin. Thumbnails, the waveform and trim export are not built yet and
//!        say so (`PlatformCapabilities::quick_trim` is false).
//! WHY:   The recordings list needs `inspect` for every file; the rest is Quick trim, which
//!        is a later step on Windows (Media Foundation remux and re-encode).
//! WHERE: The shell's `trim.rs` through `platform::media()`.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use rec_core::platform::{AudioTrackInfo, ExportOptions, MediaError, MediaInfo, MediaLibrary};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToUInt64};
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
    SHFileOperationW,
};
use windows::core::PCWSTR;

use super::util::{com_init, mf_startup, msg, wide};

pub struct WinMedia;

fn not_yet(what: &str) -> MediaError {
    MediaError::Failed(format!("{what} on Windows is not available yet"))
}

fn unpack(v: u64) -> (u32, u32) {
    ((v >> 32) as u32, v as u32)
}

impl MediaLibrary for WinMedia {
    fn inspect(&self, path: &Path) -> Result<MediaInfo, MediaError> {
        com_init();
        mf_startup().map_err(MediaError::Failed)?;
        let url = wide(&path.display().to_string());
        let fail =
            |e: windows::core::Error| MediaError::Failed(msg("Could not read the recording", &e));
        // SAFETY: Media Foundation reads on a reader for this file; the PROPVARIANT is
        // cleared after use.
        unsafe {
            let reader = MFCreateSourceReaderFromURL(PCWSTR(url.as_ptr()), None).map_err(fail)?;
            let mut pv = reader
                .GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
                .map_err(fail)?;
            let duration = PropVariantToUInt64(&pv).unwrap_or(0);
            let _ = PropVariantClear(&mut pv);
            let mut info = MediaInfo {
                duration_secs: duration as f64 / 1e7,
                width: 0,
                height: 0,
                fps: 0.0,
                video_tracks: 0,
                audio: vec![],
                hevc: false,
                playable: false,
            };
            let mut i = 0u32;
            while let Ok(t) = reader.GetCurrentMediaType(i) {
                let major = t.GetGUID(&MF_MT_MAJOR_TYPE).unwrap_or_default();
                if major == MFMediaType_Video {
                    if info.video_tracks == 0 {
                        let (w, h) = unpack(t.GetUINT64(&MF_MT_FRAME_SIZE).unwrap_or(0));
                        let (n, d) = unpack(t.GetUINT64(&MF_MT_FRAME_RATE).unwrap_or(0));
                        info.width = w;
                        info.height = h;
                        info.fps = if d > 0 {
                            f64::from(n) / f64::from(d)
                        } else {
                            0.0
                        };
                        info.hevc = t.GetGUID(&MF_MT_SUBTYPE).ok() == Some(MFVideoFormat_HEVC);
                    }
                    info.video_tracks += 1;
                } else if major == MFMediaType_Audio {
                    info.audio.push(AudioTrackInfo {
                        channels: t.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap_or(2),
                        enabled: reader
                            .GetStreamSelection(i)
                            .map(|b| b.as_bool())
                            .unwrap_or(true),
                    });
                }
                i += 1;
            }
            info.playable = info.video_tracks > 0 && info.duration_secs > 0.0;
            Ok(info)
        }
    }

    fn thumbnails(
        &self,
        _path: &Path,
        _count: u32,
        _max_width: f64,
    ) -> Result<Vec<Vec<u8>>, MediaError> {
        Err(not_yet("Timeline thumbnails"))
    }

    fn waveform(&self, _path: &Path, _buckets: u32) -> Result<Vec<f32>, MediaError> {
        Err(not_yet("The waveform"))
    }

    fn export(
        &self,
        _options: &ExportOptions,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(f64),
    ) -> Result<(), MediaError> {
        Err(not_yet("Quick trim"))
    }

    fn trash(&self, path: &Path) -> Result<(), MediaError> {
        // SHFileOperation takes a list ending in two NULs.
        let mut from = wide(&path.display().to_string());
        from.push(0);
        let mut op = SHFILEOPSTRUCTW {
            wFunc: FO_DELETE,
            pFrom: PCWSTR(from.as_ptr()),
            fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_SILENT.0 | FOF_NOERRORUI.0)
                as u16,
            ..Default::default()
        };
        // SAFETY: `op` and the double-NUL path list outlive the call.
        let r = unsafe { SHFileOperationW(&mut op) };
        if r == 0 && !op.fAnyOperationsAborted.as_bool() {
            Ok(())
        } else {
            Err(MediaError::Failed(format!(
                "Could not move {} to the Recycle Bin (error {r})",
                path.display()
            )))
        }
    }
}
