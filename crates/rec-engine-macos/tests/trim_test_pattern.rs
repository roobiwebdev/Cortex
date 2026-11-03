//! Quick trim end to end: record Rec's test pattern for 6 s through `MacEngine` (no Screen
//! Recording grant needed), then trim 1.0–4.0 s in both modes and check the results with
//! ffprobe and AVFoundation:
//! - Exact: 3.0 s ± 1 frame, first frame at 0, 180 frames.
//! - Fast: no re-encode (same codec), at most one keyframe interval longer than 3.0 s.
//! - Both: the same streams as the source, and the file decodes (`probe`).
//!
//! It also checks thumbnails, the waveform, track dropping on a 3-audio-track file (made with
//! ffmpeg, as the writer's mix/system/mic layout) and cancelling. Skips without ffprobe.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rec_core::api::{EngineEvent, RecorderEngine};
use rec_core::library::{KeepTracks, TrimMode};
use rec_core::settings::{Codec, Container, RecorderSettings, Resolution, Scene};
use rec_engine::engine::{EngineOptions, MacEngine};
use rec_engine::trim::{self, ExportOptions};

const FPS: f64 = 60.0;

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn ffprobe(args: &[&str], file: &Path) -> String {
    let out = Command::new("ffprobe")
        .args(["-v", "error"])
        .args(args)
        .arg(file)
        .output()
        .expect("run ffprobe");
    assert!(out.status.success(), "ffprobe failed on {}", file.display());
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// "codec_type:codec_name" per stream, in order.
fn streams(file: &Path) -> Vec<String> {
    ffprobe(
        &[
            "-show_entries",
            "stream=codec_type,codec_name",
            "-of",
            "csv=p=0",
        ],
        file,
    )
    .lines()
    .map(|l| l.to_string())
    .collect()
}

fn format_duration(file: &Path) -> f64 {
    ffprobe(
        &["-show_entries", "format=duration", "-of", "csv=p=0"],
        file,
    )
    .parse()
    .expect("duration")
}

fn record(dir: &Path, secs: f64) -> PathBuf {
    let settings = RecorderSettings {
        output_dir: dir.display().to_string(),
        container: Container::Mov,
        codec: Codec::H264,
        resolution: Resolution::R1080p,
        fps: FPS as u32,
        scene: Scene::Screen,
        mic_enabled: false,
        system_audio: true,
        countdown: 0,
        ..Default::default()
    };
    let (tx, rx) = mpsc::channel();
    let events = Arc::new(move |e: EngineEvent| {
        let _ = tx.send(e);
    });
    let opts = EngineOptions {
        test_pattern: true,
        max_duration_secs: Some(secs),
        ..Default::default()
    };
    let engine = MacEngine::new(opts, settings.clone(), events);
    engine.start(&settings).expect("start");
    let path = loop {
        match rx.recv_timeout(Duration::from_secs(40)) {
            Ok(EngineEvent::RecordingFinished { recording }) => {
                break PathBuf::from(recording.path);
            }
            Ok(EngineEvent::Error {
                message,
                recoverable: false,
            }) => panic!("recording failed: {message}"),
            Ok(_) => {}
            Err(_) => panic!("no RecordingFinished within 40 s"),
        }
    };
    engine.shutdown();
    path
}

fn export(src: &Path, dst: &Path, mode: TrimMode, keep: KeepTracks) -> Vec<f64> {
    let mut seen = vec![];
    trim::export(
        &ExportOptions {
            src,
            dst,
            start_secs: 1.0,
            end_secs: 4.0,
            mode,
            keep,
        },
        &AtomicBool::new(false),
        &mut |p| seen.push(p),
    )
    .unwrap_or_else(|e| panic!("{mode:?} export failed: {e:#}"));
    seen
}

#[test]
fn trims_a_test_pattern_recording_in_both_modes() {
    if !have("ffprobe") {
        eprintln!("trim test skipped: ffprobe is not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cortex-trim-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = record(&dir, 6.0);
    let all = KeepTracks::default();

    // The library's view of the file.
    let info = trim::inspect(&src).expect("inspect");
    println!("source: {info:?}");
    assert!(
        (info.duration_secs - 6.0).abs() < 0.1,
        "duration {}",
        info.duration_secs
    );
    assert_eq!(
        (info.width, info.height, info.video_tracks),
        (1920, 1080, 1)
    );
    assert_eq!(info.audio.len(), 1);
    assert!(info.playable && !info.hevc);
    let source_streams = streams(&src);
    println!("source streams: {source_streams:?}");

    let thumbs = trim::thumbnails(&src, 6, 160.0).expect("thumbnails");
    assert_eq!(thumbs.len(), 6);
    assert!(
        thumbs
            .iter()
            .all(|j| j.starts_with(&[0xFF, 0xD8]) && j.len() > 500)
    );
    let wave = trim::waveform(&src, 120).expect("waveform");
    assert_eq!(wave.len(), 120);
    let loud = wave.iter().filter(|v| **v > 0.05).count();
    println!(
        "waveform: {loud} of 120 buckets above 0.05, max {:.2}",
        wave.iter().cloned().fold(0.0, f32::max)
    );
    assert!(
        loud > 0,
        "the test pattern's beeps should show in the waveform"
    );

    // Keyframe spacing of the source bounds how much longer a passthrough cut may be.
    let keys: Vec<f64> = ffprobe(
        &[
            "-select_streams",
            "v:0",
            "-skip_frame",
            "nokey",
            "-show_entries",
            "frame=pts_time",
            "-of",
            "csv=p=0",
        ],
        &src,
    )
    .lines()
    .filter_map(|l| l.trim().trim_end_matches(',').parse().ok())
    .collect();
    let gop = keys
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(0.0, f64::max)
        .max(1.0 / FPS);
    println!("source keyframes at {keys:?} (max gap {gop:.3} s)");

    // Exact: re-encoded, frame-accurate.
    let exact = dir.join("exact.mov");
    let p = export(&src, &exact, TrimMode::Exact, all);
    assert_eq!(p.last().copied(), Some(1.0));
    let d = format_duration(&exact);
    let frames: u32 = ffprobe(
        &[
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
        ],
        &exact,
    )
    .parse()
    .expect("frames");
    println!(
        "exact: {d:.4} s, {frames} frames, streams {:?}",
        streams(&exact)
    );
    assert!((d - 3.0).abs() <= 1.0 / FPS + 0.03, "exact duration {d}");
    assert!((179..=181).contains(&frames), "exact frames {frames}");
    assert_eq!(streams(&exact).len(), source_streams.len());
    rec_engine::probe::probe(&exact).expect("exact file decodes");

    // Fast: passthrough, same codecs, cut on the keyframe before 1.0 s at worst.
    let fast = dir.join("fast.mov");
    export(&src, &fast, TrimMode::Fast, all);
    let d = format_duration(&fast);
    println!("fast: {d:.4} s, streams {:?}", streams(&fast));
    assert!(
        d >= 3.0 - 1.0 / FPS && d <= 3.0 + gop + 1.0 / FPS,
        "fast duration {d} (gop {gop})"
    );
    assert_eq!(
        streams(&fast),
        source_streams,
        "passthrough keeps every stream and codec"
    );
    rec_engine::probe::probe(&fast).expect("fast file decodes");
    // No re-encode: the fast file's video packets are a run of the source's, byte for byte in size.
    let sizes = |f: &Path| -> Vec<u64> {
        ffprobe(
            &[
                "-select_streams",
                "v:0",
                "-show_entries",
                "packet=size",
                "-of",
                "csv=p=0",
            ],
            f,
        )
        .lines()
        .filter_map(|l| l.trim().trim_end_matches(',').parse().ok())
        .collect()
    };
    let (src_sizes, fast_sizes) = (sizes(&src), sizes(&fast));
    println!(
        "fast: {} video packets (source {})",
        fast_sizes.len(),
        src_sizes.len()
    );
    assert!(fast_sizes.len() >= 180);
    assert!(
        src_sizes
            .windows(fast_sizes.len())
            .any(|w| w == fast_sizes.as_slice()),
        "fast packets are not a run of the source's"
    );
    let fast_info = trim::inspect(&fast).expect("inspect fast");
    assert!(
        (fast_info.duration_secs - 3.0).abs() <= 1.0 / FPS + 0.03,
        "AVFoundation duration {}",
        fast_info.duration_secs
    );

    // The writer's mix/system/mic layout: stems are kept, enabled flags carried over, and
    // dropping one stem drops the mix too.
    if have("ffmpeg") {
        let three = dir.join("three.mov");
        let ok = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&src)
            .args([
                "-map", "0:v", "-map", "0:a", "-map", "0:a", "-map", "0:a", "-c", "copy",
            ])
            .args([
                "-disposition:a:0",
                "default",
                "-disposition:a:1",
                "0",
                "-disposition:a:2",
                "0",
            ])
            .arg(&three)
            .status()
            .expect("run ffmpeg")
            .success();
        assert!(ok, "ffmpeg could not build the 3-track file");
        let kept_all = dir.join("three-all.mov");
        export(&three, &kept_all, TrimMode::Fast, all);
        assert_eq!(streams(&kept_all).len(), 4, "every track kept");
        let no_system = dir.join("three-mic.mov");
        export(
            &three,
            &no_system,
            TrimMode::Fast,
            KeepTracks {
                system: false,
                ..all
            },
        );
        assert_eq!(streams(&no_system).len(), 2, "video + mic only");
        let mic_only = trim::inspect(&no_system).expect("inspect");
        assert!(
            mic_only.audio.iter().all(|a| a.enabled),
            "a kept stem plays without the mix"
        );
        let silent = dir.join("three-silent.mov");
        export(
            &three,
            &silent,
            TrimMode::Fast,
            KeepTracks {
                system: false,
                mic: false,
                camera: true,
            },
        );
        assert_eq!(streams(&silent).len(), 1, "video only");
    }

    // Cancelling before the export starts leaves no file behind.
    let cancelled = dir.join("cancelled.mov");
    let err = trim::export(
        &ExportOptions {
            src: &src,
            dst: &cancelled,
            start_secs: 1.0,
            end_secs: 4.0,
            mode: TrimMode::Exact,
            keep: all,
        },
        &AtomicBool::new(true),
        &mut |_| {},
    )
    .expect_err("a cancelled export fails");
    assert_eq!(err.to_string(), "cancelled");
    assert!(!cancelled.exists());

    // An existing destination is refused, never overwritten.
    assert!(
        trim::export(
            &ExportOptions {
                src: &src,
                dst: &fast,
                start_secs: 1.0,
                end_secs: 4.0,
                mode: TrimMode::Fast,
                keep: all,
            },
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .is_err()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
