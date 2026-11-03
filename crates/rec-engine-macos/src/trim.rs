//! SOURCE OF TRUTH KEYWORDS: quick trim, inspect, thumbnails, waveform, export, passthrough, re-encode, trash, AudioRole
//! WHAT:  The AVFoundation side of the recordings library and Quick trim: read a file's tracks,
//!        make timeline thumbnails (JPEG) and waveform peaks, export a time range with the kept
//!        tracks (Fast = AVMutableComposition + passthrough, Exact = re-encode), and move a file
//!        to the Trash.
//! WHY:   Passthrough keeps every kept track with no quality loss and is frame-accurate in Apple
//!        players through edit lists (engineering plan §8, NEW-TR-04); Exact re-encodes for
//!        strict players (NEW-TR-05). Track roles follow the writer's layout (mix, system, mic).
//! WHERE: the Tauri shell's `trim.rs` commands; `tests/trim_test_pattern.rs`.

#![allow(deprecated)]

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImageCompressionFactor};
use objc2_av_foundation::*;
use objc2_core_foundation::CGSize;
use objc2_core_media::{
    CMAudioFormatDescriptionGetStreamBasicDescription, CMFormatDescription, CMSampleBuffer, CMTime,
    CMTimeRange,
};
use objc2_foundation::{NSDictionary, NSFileManager, NSString, NSURL};
use rec_core::library::{KeepTracks, TrimMode};

use crate::util::*;

/// One audio track as the file stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioTrackInfo {
    pub channels: u32,
    pub enabled: bool,
}

/// What the recordings list shows about a file.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// Video tracks: the screen first, then a separate camera track if there is one.
    pub video_tracks: u32,
    pub audio: Vec<AudioTrackInfo>,
    /// The first video track is HEVC (`hvc1`/`hev1`).
    pub hevc: bool,
    pub playable: bool,
}

/// What an audio track holds, from the writer's track order (mix, system, mic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioRole {
    /// System audio and mic mixed (or the only track, when that can't be told apart).
    Mix,
    System,
    Mic,
}

/// Roles by position and channel count: 3 tracks are mix, system, mic; 2 are the system
/// and mic stems; a lone mono track is the mic; any other single track counts as the mix.
pub fn audio_roles(channels: &[u32]) -> Vec<AudioRole> {
    use AudioRole::*;
    match channels {
        [_, _, _] => vec![Mix, System, Mic],
        [_, _] => vec![System, Mic],
        [1] => vec![Mic],
        _ => vec![Mix; channels.len()],
    }
}

/// For each source audio track: `None` to drop it, or `Some(enabled)`. The mix is only kept
/// when both system and mic are (it holds both); a lone mix is kept when either is. While a
/// mix is kept the tracks keep their source enabled flags (players play only the mix);
/// without it every kept stem is enabled so the file still plays its audio.
pub fn audio_plan(
    roles: &[AudioRole],
    src_enabled: &[bool],
    keep: KeepTracks,
) -> Vec<Option<bool>> {
    let has_stems = roles.iter().any(|r| *r != AudioRole::Mix);
    let kept: Vec<bool> = roles
        .iter()
        .map(|r| match r {
            AudioRole::Mix if has_stems => keep.system && keep.mic,
            AudioRole::Mix => keep.system || keep.mic,
            AudioRole::System => keep.system,
            AudioRole::Mic => keep.mic,
        })
        .collect();
    let mix_kept = roles
        .iter()
        .zip(&kept)
        .any(|(r, k)| *k && *r == AudioRole::Mix);
    kept.iter()
        .enumerate()
        .map(|(i, k)| {
            k.then(|| {
                if mix_kept {
                    src_enabled.get(i).copied().unwrap_or(true)
                } else {
                    true
                }
            })
        })
        .collect()
}

fn asset(path: &Path) -> Result<Retained<AVURLAsset>> {
    let abs = std::path::absolute(path)?;
    if !abs.is_file() {
        bail!("{} does not exist", abs.display());
    }
    let s = abs
        .to_str()
        .ok_or_else(|| anyhow!("the path is not UTF-8"))?;
    // SAFETY: a file URL from a valid string; AVURLAsset copies what it needs.
    unsafe {
        let url = NSURL::fileURLWithPath(&ns(s));
        Ok(AVURLAsset::URLAssetWithURL_options(&url, None))
    }
}

fn file_url(path: &Path) -> Result<Retained<NSURL>> {
    let s = path
        .to_str()
        .ok_or_else(|| anyhow!("the path is not UTF-8"))?;
    Ok(NSURL::fileURLWithPath(&ns(s)))
}

/// The first format description of a track, as CoreMedia's type.
///
/// # Safety
/// `t` must be a live track; the returned reference lives as long as the array it came from,
/// which the caller keeps alive through the closure.
unsafe fn with_format<R>(t: &AVAssetTrack, f: impl FnOnce(&CMFormatDescription) -> R) -> Option<R> {
    // SAFETY: AVAssetTrack.formatDescriptions holds CMFormatDescriptionRefs (toll-free CF
    // objects); the array, and so each element, stays alive for this call.
    unsafe {
        let fds = t.formatDescriptions();
        let fd = fds.firstObject()?;
        let fd: &CMFormatDescription = &*(Retained::as_ptr(&fd) as *const CMFormatDescription);
        Some(f(fd))
    }
}

fn media_video() -> &'static AVMediaType {
    // SAFETY: an AVFoundation constant, set for the life of the process.
    unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo")
}
fn media_audio() -> &'static AVMediaType {
    // SAFETY: as above.
    unsafe { AVMediaTypeAudio }.expect("AVMediaTypeAudio")
}

/// Duration, size, frame rate and track layout of a recording.
pub fn inspect(path: &Path) -> Result<MediaInfo> {
    let asset = asset(path)?;
    // SAFETY: synchronous property reads on a local file asset; the arrays are kept alive.
    unsafe {
        let videos = asset.tracksWithMediaType(media_video());
        let audios = asset.tracksWithMediaType(media_audio());
        let (mut width, mut height, mut fps, mut hevc) = (0, 0, 0.0, false);
        if let Some(v) = videos.firstObject() {
            let sz = v.naturalSize();
            width = sz.width.round() as u32;
            height = sz.height.round() as u32;
            fps = v.nominalFrameRate() as f64;
            hevc = with_format(&v, |fd| {
                matches!(&fd.media_sub_type().to_be_bytes(), b"hvc1" | b"hev1")
            })
            .unwrap_or(false);
        }
        let audio = audios
            .iter()
            .map(|t| AudioTrackInfo {
                channels: with_format(&t, |fd| {
                    CMAudioFormatDescriptionGetStreamBasicDescription(fd)
                        .as_ref()
                        .map_or(0, |a| a.mChannelsPerFrame)
                })
                .unwrap_or(0),
                enabled: t.isEnabled(),
            })
            .collect();
        Ok(MediaInfo {
            duration_secs: secs(asset.duration()).max(0.0),
            width,
            height,
            fps,
            video_tracks: videos.count() as u32,
            audio,
            hevc,
            playable: asset.isPlayable(),
        })
    }
}

/// `count` JPEG thumbnails at most `max_width` px wide, spread evenly over the file (the
/// middle of each slot). Frames may be up to half a slot off, which keeps it fast.
pub fn thumbnails(path: &Path, count: u32, max_width: f64) -> Result<Vec<Vec<u8>>> {
    let asset = asset(path)?;
    // SAFETY: AVAssetImageGenerator and NSBitmapImageRep are used on this thread only; every
    // object passed in is alive for the call.
    unsafe {
        let duration = secs(asset.duration());
        if duration.is_nan() || duration <= 0.0 || count == 0 {
            return Ok(vec![]);
        }
        let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
        generator.setAppliesPreferredTrackTransform(true);
        generator.setMaximumSize(CGSize {
            width: max_width,
            height: 0.0,
        });
        let slot = duration / count as f64;
        let tol = CMTime::with_seconds(slot / 2.0, 600);
        generator.setRequestedTimeToleranceBefore(tol);
        generator.setRequestedTimeToleranceAfter(tol);
        let key: &NSString = NSImageCompressionFactor;
        let props: Retained<NSDictionary<NSString, AnyObject>> = dict(vec![(key, num_f(0.72))]);
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let t = CMTime::with_seconds(slot * (i as f64 + 0.5), 600);
            let image = generator
                .copyCGImageAtTime_actualTime_error(t, std::ptr::null_mut())
                .map_err(|e| anyhow!("thumbnail: {}", e.localizedDescription()))?;
            let rep = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
            let data = rep
                .representationUsingType_properties(NSBitmapImageFileType::JPEG, &props)
                .ok_or_else(|| anyhow!("thumbnail: JPEG encoding failed"))?;
            out.push(data.to_vec());
        }
        Ok(out)
    }
}

/// Peak level (0..1) of the played audio track in each of `buckets` equal time slices.
/// Empty when the file has no audio.
pub fn waveform(path: &Path, buckets: u32) -> Result<Vec<f32>> {
    let asset = asset(path)?;
    // SAFETY: AVAssetReader runs on this thread; sample buffers are read and released here.
    unsafe {
        let duration = secs(asset.duration());
        let audios = asset.tracksWithMediaType(media_audio());
        let track = audios
            .iter()
            .find(|t| t.isEnabled())
            .or_else(|| audios.firstObject());
        let (Some(track), true) = (track, duration > 0.0 && buckets > 0) else {
            return Ok(vec![]);
        };
        let reader = AVAssetReader::assetReaderWithAsset_error(&asset)
            .map_err(|e| anyhow!("waveform: {}", e.localizedDescription()))?;
        let settings = dict(vec![
            (&*ns("AVFormatIDKey"), num_i(fourcc(b"lpcm") as i64)),
            (&*ns("AVLinearPCMBitDepthKey"), num_i(32)),
            (&*ns("AVLinearPCMIsFloatKey"), num_b(true)),
            (&*ns("AVLinearPCMIsNonInterleaved"), num_b(false)),
            (&*ns("AVLinearPCMIsBigEndianKey"), num_b(false)),
        ]);
        let out = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            &track,
            Some(&settings),
        );
        out.setAlwaysCopiesSampleData(false);
        reader.addOutput(&out);
        if !reader.startReading() {
            bail!("waveform: the audio could not be read");
        }
        let mut peaks = vec![0f32; buckets as usize];
        let per_sec = buckets as f64 / duration;
        let mut bytes: Vec<u8> = Vec::new();
        while let Some(sb) = out.copyNextSampleBuffer() {
            let sb: Retained<CMSampleBuffer> = sb;
            let frames = sb.num_samples().max(0) as usize;
            let Some(block) = sb.data_buffer() else {
                continue;
            };
            let Some(rate) = sb.format_description().and_then(|fd| {
                CMAudioFormatDescriptionGetStreamBasicDescription(&fd)
                    .as_ref()
                    .map(|a| a.mSampleRate)
            }) else {
                continue;
            };
            let len = block.data_length();
            if frames == 0 || len == 0 || rate <= 0.0 {
                continue;
            }
            bytes.resize(len, 0);
            let dst = std::ptr::NonNull::new(bytes.as_mut_ptr().cast()).expect("non-null vec");
            if block.copy_data_bytes(0, len, dst) != 0 {
                continue;
            }
            let channels = (len / 4 / frames).max(1);
            let t0 = secs(sb.presentation_time_stamp());
            for (i, frame) in bytes.chunks_exact(4 * channels).enumerate() {
                let b = ((t0 + i as f64 / rate) * per_sec) as isize;
                let Some(p) = usize::try_from(b).ok().and_then(|b| peaks.get_mut(b)) else {
                    continue;
                };
                for s in frame.as_chunks::<4>().0 {
                    let v = f32::from_le_bytes(*s).abs();
                    if v > *p {
                        *p = v.min(1.0);
                    }
                }
            }
        }
        if reader.status() != AVAssetReaderStatus::Completed {
            bail!("waveform: reading the audio failed");
        }
        Ok(peaks)
    }
}

/// What `export` writes.
pub struct ExportOptions<'a> {
    pub src: &'a Path,
    /// Must not exist yet; `.mp4` writes MPEG-4, anything else QuickTime.
    pub dst: &'a Path,
    pub start_secs: f64,
    pub end_secs: f64,
    pub mode: TrimMode,
    pub keep: KeepTracks,
}

/// Write `start..end` of `src` to `dst` with the kept tracks. `progress` gets 0..1 about ten
/// times a second; setting `cancel` stops the export (the error says "cancelled") and
/// removes the partial file.
pub fn export(o: &ExportOptions, cancel: &AtomicBool, progress: &mut dyn FnMut(f64)) -> Result<()> {
    let asset = asset(o.src)?;
    if o.dst.exists() {
        bail!("{} already exists", o.dst.display());
    }
    let (start, end) = (o.start_secs, o.end_secs);
    // SAFETY: the composition, export session and source asset live on this thread until the
    // export finished; the completion block only sends on a channel.
    unsafe {
        let comp = AVMutableComposition::composition();
        let videos = asset.tracksWithMediaType(media_video());
        let audios = asset.tracksWithMediaType(media_audio());
        let channels: Vec<u32> = audios
            .iter()
            .map(|t| {
                with_format(&t, |fd| {
                    CMAudioFormatDescriptionGetStreamBasicDescription(fd)
                        .as_ref()
                        .map_or(0, |a| a.mChannelsPerFrame)
                })
                .unwrap_or(0)
            })
            .collect();
        let enabled: Vec<bool> = audios.iter().map(|t| t.isEnabled()).collect();
        let plan = audio_plan(&audio_roles(&channels), &enabled, o.keep);

        let mut hevc = false;
        let mut added = 0;
        let mut add = |t: &AVAssetTrack, enabled: bool| -> Result<()> {
            let tr = t.timeRange();
            let (ts, te) = (secs(tr.start), secs(tr.start) + secs(tr.duration));
            let (s, e) = (start.max(ts), end.min(te));
            if e - s < 0.001 {
                return Ok(());
            }
            let scale = t.naturalTimeScale().max(600);
            let ct = comp
                .addMutableTrackWithMediaType_preferredTrackID(&t.mediaType(), 0)
                .ok_or_else(|| anyhow!("could not add a track to the trimmed file"))?;
            let range = CMTimeRange::new(
                CMTime::with_seconds(s, scale),
                CMTime::with_seconds(e - s, scale),
            );
            ct.insertTimeRange_ofTrack_atTime_error(
                range,
                t,
                CMTime::with_seconds(s - start, scale),
            )
            .map_err(|e| anyhow!("trim: {}", e.localizedDescription()))?;
            ct.setPreferredTransform(t.preferredTransform());
            ct.setLanguageCode(t.languageCode().as_deref());
            ct.setExtendedLanguageTag(t.extendedLanguageTag().as_deref());
            ct.setEnabled(enabled);
            added += 1;
            Ok(())
        };
        for (i, v) in videos.iter().enumerate() {
            // The screen is always kept; later video tracks are the separate camera track.
            if i == 0 {
                hevc = with_format(&v, |fd| {
                    matches!(&fd.media_sub_type().to_be_bytes(), b"hvc1" | b"hev1")
                })
                .unwrap_or(false);
            } else if !o.keep.camera {
                continue;
            }
            add(&v, v.isEnabled())?;
        }
        for (t, p) in audios.iter().zip(&plan) {
            if let Some(enabled) = p {
                add(&t, *enabled)?;
            }
        }
        if added == 0 {
            bail!("nothing to export in that range");
        }

        let preset: &NSString = match (o.mode, hevc) {
            (TrimMode::Fast, _) => AVAssetExportPresetPassthrough,
            (TrimMode::Exact, true) => AVAssetExportPresetHEVCHighestQuality,
            (TrimMode::Exact, false) => AVAssetExportPresetHighestQuality,
        };
        let session = AVAssetExportSession::exportSessionWithAsset_presetName(&comp, preset)
            .ok_or_else(|| anyhow!("this file can't be exported with that mode"))?;
        let mp4 = o
            .dst
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
        let file_type = if mp4 {
            AVFileTypeMPEG4
        } else {
            AVFileTypeQuickTimeMovie
        };
        let url = file_url(o.dst)?;
        session.setOutputURL(Some(&url));
        session.setOutputFileType(file_type);
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }
        let (tx, rx) = mpsc::channel::<()>();
        let done = RcBlock::new(move || {
            let _ = tx.send(());
        });
        session.exportAsynchronouslyWithCompletionHandler(&done);
        let mut cancelled = false;
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !cancelled && cancel.load(Ordering::Relaxed) {
                        cancelled = true;
                        session.cancelExport();
                    }
                    progress(session.progress().clamp(0.0, 1.0) as f64);
                }
            }
        }
        let status = session.status();
        if status == AVAssetExportSessionStatus::Completed {
            progress(1.0);
            return Ok(());
        }
        let _ = std::fs::remove_file(o.dst);
        if status == AVAssetExportSessionStatus::Cancelled || cancelled {
            bail!("cancelled");
        }
        let why = session
            .error()
            .map(|e| e.localizedDescription().to_string())
            .unwrap_or_default();
        bail!("the export failed: {why}")
    }
}

/// Move `path` to the user's Trash (never deletes outright).
pub fn trash(path: &Path) -> Result<()> {
    let url = file_url(path)?;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|e| {
            anyhow!(
                "could not move it to the Trash: {}",
                e.localizedDescription()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::AudioRole::*;
    use super::*;

    const ALL: KeepTracks = KeepTracks {
        camera: true,
        system: true,
        mic: true,
    };

    #[test]
    fn roles_follow_the_writer_layout() {
        assert_eq!(audio_roles(&[2, 2, 1]), vec![Mix, System, Mic]);
        assert_eq!(audio_roles(&[2, 1]), vec![System, Mic]);
        assert_eq!(audio_roles(&[1]), vec![Mic]);
        assert_eq!(audio_roles(&[2]), vec![Mix]);
        assert_eq!(audio_roles(&[]), Vec::<AudioRole>::new());
    }

    #[test]
    fn keeping_everything_keeps_source_flags() {
        let p = audio_plan(&[Mix, System, Mic], &[true, false, false], ALL);
        assert_eq!(p, vec![Some(true), Some(false), Some(false)]);
    }

    #[test]
    fn dropping_one_stem_drops_the_mix_and_enables_the_other() {
        let k = KeepTracks {
            system: false,
            ..ALL
        };
        assert_eq!(
            audio_plan(&[Mix, System, Mic], &[true, false, false], k),
            vec![None, None, Some(true)]
        );
        let k = KeepTracks { mic: false, ..ALL };
        assert_eq!(
            audio_plan(&[System, Mic], &[true, true], k),
            vec![Some(true), None]
        );
    }

    #[test]
    fn a_lone_mix_stays_while_either_is_kept() {
        let k = KeepTracks {
            system: false,
            ..ALL
        };
        assert_eq!(audio_plan(&[Mix], &[true], k), vec![Some(true)]);
        let none = KeepTracks {
            system: false,
            mic: false,
            camera: true,
        };
        assert_eq!(audio_plan(&[Mix], &[true], none), vec![None]);
        assert_eq!(
            audio_plan(&[Mix, System, Mic], &[true, false, false], none),
            vec![None, None, None]
        );
    }
}
