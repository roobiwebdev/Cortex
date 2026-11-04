//! SOURCE OF TRUTH KEYWORDS: GstMedia, MediaLibrary, Discoverer, inspect, thumbnails, waveform, trash, recordings list
//! WHAT:  The recordings list's media backend on Linux, over GStreamer: file facts through
//!        the Discoverer, JPEG thumbnails from keyframes, the first audio track's peaks, and
//!        moving files to the freedesktop trash. Export (Quick trim) is not built yet and
//!        says so; `capabilities.quickTrim` is false.
//! WHY:   The shell lists recordings only when `inspect` succeeds, and draws the trim
//!        timeline from thumbnails and the waveform. The first video track is the screen and
//!        the first audio track is the mix, as the engine writes them.
//! WHERE: `platform::linux::Media` in the Tauri shell.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_pbutils as pbutils;
use gstreamer_pbutils::prelude::*;
use rec_core::platform::{AudioTrackInfo, ExportOptions, MediaError, MediaInfo, MediaLibrary};

use crate::pipeline::quote;

pub struct GstMedia;

fn failed(e: impl std::fmt::Display) -> MediaError {
    MediaError::Failed(e.to_string())
}

fn uri(path: &Path) -> Result<String, MediaError> {
    gst::glib::filename_to_uri(path, None)
        .map(|u| u.to_string())
        .map_err(failed)
}

fn ready() -> Result<(), MediaError> {
    if crate::session::init() {
        Ok(())
    } else {
        Err(MediaError::Failed("GStreamer is not installed".into()))
    }
}

/// Parse `text`, go to PAUSED and wait for the preroll.
fn paused(text: &str) -> Result<gst::Pipeline, MediaError> {
    let p = gst::parse::launch(text)
        .map_err(failed)?
        .downcast::<gst::Pipeline>()
        .map_err(|_| failed("not a pipeline"))?;
    p.set_state(gst::State::Paused).map_err(failed)?;
    let (r, _, _) = p.state(gst::ClockTime::from_seconds(10));
    if r.is_err() {
        let _ = p.set_state(gst::State::Null);
        return Err(failed("could not open the file"));
    }
    Ok(p)
}

fn appsink(p: &gst::Pipeline) -> Result<gst_app::AppSink, MediaError> {
    p.by_name("snk")
        .and_then(|e| e.downcast::<gst_app::AppSink>().ok())
        .ok_or_else(|| failed("no appsink"))
}

/// Fill the empty `slots` by playing the video from the start: each gets the first frame at
/// or after its target time, or the last frame when the track ends before it. Stops as soon
/// as every slot is filled.
fn fill_by_decoding(
    text: &str,
    targets: &[u64],
    slots: &mut [Option<Vec<u8>>],
) -> Result<(), MediaError> {
    let p = paused(text)?;
    let sink = appsink(&p)?;
    p.set_state(gst::State::Playing).map_err(failed)?;
    let mut last: Option<Vec<u8>> = None;
    while slots.iter().any(Option::is_none) {
        let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(5)) else {
            break;
        };
        let Some(buf) = sample.buffer() else { continue };
        let pts = buf.pts().map(|t| t.nseconds()).unwrap_or(0);
        let Ok(map) = buf.map_readable() else {
            continue;
        };
        let jpeg = map.as_slice().to_vec();
        for (slot, t) in slots.iter_mut().zip(targets) {
            if slot.is_none() && pts >= *t {
                *slot = Some(jpeg.clone());
            }
        }
        last = Some(jpeg);
    }
    let _ = p.set_state(gst::State::Null);
    if let Some(last) = last {
        for slot in slots.iter_mut().filter(|s| s.is_none()) {
            *slot = Some(last.clone());
        }
    }
    Ok(())
}

/**
 * SOURCE OF TRUTH KEYWORDS: frames_for_every_slot, thumbnails, timeline strip, missing frames
 * WHAT:  One frame per slot: an empty slot takes the nearest earlier frame, or the first one
 *        when none comes before. None when there is no frame at all.
 * WHY:   The trim timeline lays out exactly as many tiles as it asked for.
 * WHERE: `GstMedia::thumbnails`.
 */
fn frames_for_every_slot(slots: Vec<Option<Vec<u8>>>) -> Option<Vec<Vec<u8>>> {
    let first = slots.iter().flatten().next()?.clone();
    let mut prev = first;
    Some(
        slots
            .into_iter()
            .map(|s| {
                if let Some(f) = s {
                    prev = f;
                }
                prev.clone()
            })
            .collect(),
    )
}

impl MediaLibrary for GstMedia {
    fn inspect(&self, path: &Path) -> Result<MediaInfo, MediaError> {
        ready()?;
        let d = pbutils::Discoverer::new(gst::ClockTime::from_seconds(10)).map_err(failed)?;
        let info = d.discover_uri(&uri(path)?).map_err(failed)?;
        let videos = info.video_streams();
        let first = videos.first();
        let fps = first
            .map(|v| {
                let f = v.framerate();
                if f.denom() > 0 {
                    f64::from(f.numer()) / f64::from(f.denom())
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let hevc = first
            .and_then(|v| v.caps())
            .and_then(|c| c.structure(0).map(|s| s.name() == "video/x-h265"))
            .unwrap_or(false);
        Ok(MediaInfo {
            duration_secs: info.duration().map(|t| t.seconds_f64()).unwrap_or(0.0),
            width: first.map(|v| v.width()).unwrap_or(0),
            height: first.map(|v| v.height()).unwrap_or(0),
            fps,
            video_tracks: videos.len() as u32,
            audio: info
                .audio_streams()
                .iter()
                .map(|a| AudioTrackInfo {
                    channels: a.channels(),
                    enabled: true,
                })
                .collect(),
            hevc,
            playable: first.is_some(),
        })
    }

    fn thumbnails(
        &self,
        path: &Path,
        count: u32,
        max_width: f64,
    ) -> Result<Vec<Vec<u8>>, MediaError> {
        ready()?;
        let w = ((max_width.clamp(16.0, 4096.0) as u32) / 2) * 2;
        let text = format!(
            "filesrc location={} ! qtdemux name=d d.video_0 ! queue ! decodebin ! videoconvert ! videoscale ! video/x-raw,format=I420,width={w},pixel-aspect-ratio=1/1 ! jpegenc quality=80 ! appsink name=snk sync=false",
            quote(&path.to_string_lossy())
        );
        let p = paused(&text)?;
        let sink = appsink(&p)?;
        let duration = p
            .query_duration::<gst::ClockTime>()
            .map(|d| d.nseconds())
            .unwrap_or(0);
        let n = count.max(1);
        let targets: Vec<u64> = (0..n)
            .map(|i| duration * (2 * u64::from(i) + 1) / (2 * u64::from(n)))
            .collect();
        let mut slots: Vec<Option<Vec<u8>>> = vec![None; targets.len()];
        for (slot, t) in slots.iter_mut().zip(&targets) {
            let sought = p.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::from_nseconds(*t),
            );
            // A seek or preroll can miss (a slow machine, a video track that ends before
            // the audio); the pass below fills that slot.
            if sought.is_err() {
                continue;
            }
            let _ = p.state(gst::ClockTime::from_seconds(5));
            if let Some(sample) = sink.try_pull_preroll(gst::ClockTime::from_seconds(5))
                && let Some(map) = sample.buffer().and_then(|b| b.map_readable().ok())
            {
                *slot = Some(map.as_slice().to_vec());
            }
        }
        let _ = p.set_state(gst::State::Null);
        if slots.iter().any(Option::is_none) {
            fill_by_decoding(&text, &targets, &mut slots)?;
        }
        frames_for_every_slot(slots).ok_or_else(|| failed("no video frames"))
    }

    fn waveform(&self, path: &Path, buckets: u32) -> Result<Vec<f32>, MediaError> {
        let info = self.inspect(path)?;
        if info.audio.is_empty() || buckets == 0 || info.duration_secs <= 0.0 {
            return Ok(vec![]);
        }
        let text = format!(
            "filesrc location={} ! qtdemux name=d d.audio_0 ! queue ! decodebin ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,channels=1,rate=8000 ! appsink name=snk sync=false",
            quote(&path.to_string_lossy())
        );
        let p = paused(&text)?;
        let sink = appsink(&p)?;
        p.set_state(gst::State::Playing).map_err(failed)?;
        let mut peaks = vec![0f32; buckets as usize];
        let total = info.duration_secs;
        while let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_seconds(5)) {
            let Some(buf) = sample.buffer() else { continue };
            let start = buf.pts().map(|t| t.seconds_f64()).unwrap_or(0.0);
            let Ok(map) = buf.map_readable() else {
                continue;
            };
            let bytes = map.as_slice();
            let n = bytes.len() / 4;
            for (k, s) in bytes.as_chunks::<4>().0.iter().enumerate() {
                let v = f32::from_le_bytes(*s).abs();
                let t = start + k as f64 / 8000.0;
                let b = ((t / total) * f64::from(buckets)) as usize;
                if let Some(slot) = peaks.get_mut(b.min(buckets as usize - 1)) {
                    *slot = slot.max(v.min(1.0));
                }
            }
            if n == 0 {
                break;
            }
        }
        let _ = p.set_state(gst::State::Null);
        Ok(peaks)
    }

    fn export(
        &self,
        _options: &ExportOptions,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(f64),
    ) -> Result<(), MediaError> {
        Err(MediaError::Failed(
            "Quick trim isn't available on Linux yet".into(),
        ))
    }

    fn trash(&self, path: &Path) -> Result<(), MediaError> {
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .ok_or_else(|| failed("no home folder"))?;
        let dir = crate::trash::trash_dir(&home, std::env::var("XDG_DATA_HOME").ok().as_deref());
        let when = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        crate::trash::move_to_trash(path, &dir, &when)
            .map(|_| ())
            .map_err(|e| failed(format!("Could not move it to the trash: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_gets_a_frame() {
        let f = |b: u8| Some(vec![b]);
        assert_eq!(frames_for_every_slot(vec![None, None]), None);
        assert_eq!(
            frames_for_every_slot(vec![None, f(1), None, f(2)]),
            Some(vec![vec![1], vec![1], vec![1], vec![2]])
        );
    }
}
