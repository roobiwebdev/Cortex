//! Records through the same `Shell` methods the Tauri commands call, with Rec's
//! synthetic test pattern (no Screen Recording permission, mic off), and checks
//! the files with ffprobe and the events the webview would get.
//!
//! cargo test -p cortex-desktop --test record -- --nocapture

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use cortex_desktop_lib::shell::{Outbound, Shell};
use rec_core::settings::{Codec, Container, RecorderSettings, Resolution, Scene};
use rec_core::state::{EngineStatus, Recording};
use rec_engine::engine::EngineOptions;

fn ffprobe(path: &str) -> std::collections::HashMap<String, String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate,nb_read_frames",
            "-of",
            "default=noprint_wrappers=1",
            path,
        ])
        .output()
        .expect("run ffprobe");
    assert!(out.status.success(), "ffprobe failed on {path}");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    println!("ffprobe {path}\n{}", text.trim());
    text.lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Wait for `RecordingFinished`, counting state events by status on the way.
fn wait_finished(rx: &mpsc::Receiver<Outbound>, seen: &mut Vec<EngineStatus>) -> Recording {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left).expect("RecordingFinished within 30 s") {
            Outbound::State(s) => {
                if seen.last() != Some(&s.status) {
                    seen.push(s.status);
                }
            }
            Outbound::RecordingFinished(r) => return r,
            Outbound::Error {
                message,
                recoverable,
            } => {
                assert!(recoverable, "engine error: {message}");
                println!("warning: {message}");
            }
            _ => {}
        }
    }
}

#[test]
fn records_the_test_pattern_through_the_shell() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("cortex-shell-record-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (tx, rx) = mpsc::channel();
    let emit = Arc::new(move |o: Outbound| {
        let _ = tx.send(o);
    });
    let opts = EngineOptions {
        test_pattern: true,
        // The writer cuts the file at exactly 3 s of recorded time.
        max_duration_secs: Some(3.0),
        ..Default::default()
    };
    let shell = Shell::open(opts, dir.join("settings.json"), emit);
    assert!(shell.load_error.is_none());

    // The UI's settings flow: update (validated + persisted), then start.
    let stored = shell
        .update_settings(RecorderSettings {
            output_dir: dir.join("out").display().to_string(),
            container: Container::Mov,
            codec: Codec::H264,
            resolution: Resolution::R1080p,
            fps: 60,
            scene: Scene::Screen,
            mic_enabled: false,
            system_audio: true,
            countdown: 1, // not a legal choice: validated to 3
            ..Default::default()
        })
        .expect("update_settings");
    assert_eq!(stored.countdown, 3, "settings are validated in Rust");
    let stored = shell
        .update_settings(RecorderSettings {
            countdown: 0,
            ..stored
        })
        .unwrap();

    // 1. Start, let the 3 s cap end it: exactly 180 frames at 60 fps.
    let mut seen = vec![];
    shell.start().expect("start");
    let rec = wait_finished(&rx, &mut seen);
    println!("recording 1: {rec:?}; states {seen:?}");
    let p = ffprobe(&rec.path);
    assert!(matches!(p["codec_name"].as_str(), "h264" | "hevc"));
    assert_eq!(p["r_frame_rate"], "60/1");
    assert_eq!(p["nb_read_frames"], "180");
    assert_eq!((p["width"].as_str(), p["height"].as_str()), ("1920", "1080"));
    assert!(seen.contains(&EngineStatus::Recording));

    // 2. Countdown, pause/resume, then the stop command.
    let _ = shell.update_settings(RecorderSettings {
        countdown: 3,
        ..stored
    });
    // Let the Idle state that follows the first file arrive.
    std::thread::sleep(Duration::from_millis(300));
    while rx.try_recv().is_ok() {}
    let mut seen = vec![];
    shell.start().expect("start with countdown");
    assert_eq!(shell.state().status, EngineStatus::Countdown);
    assert!(shell.stop().is_err(), "can't stop during the countdown");
    let until_recording = Instant::now() + Duration::from_secs(6);
    while shell.state().status != EngineStatus::Recording {
        assert!(Instant::now() < until_recording, "countdown never ended");
        std::thread::sleep(Duration::from_millis(50));
    }
    // Pausing needs the first frame; wait until some time is recorded.
    let until_frames = Instant::now() + Duration::from_secs(10);
    while shell.state().elapsed_secs < 0.8 {
        assert!(Instant::now() < until_frames, "no frames recorded");
        std::thread::sleep(Duration::from_millis(50));
    }
    shell.pause().expect("pause");
    std::thread::sleep(Duration::from_millis(300));
    shell.resume().expect("resume");
    std::thread::sleep(Duration::from_millis(500));
    shell.stop().expect("stop");
    let rec = wait_finished(&rx, &mut seen);
    println!("recording 2: {rec:?}; states {seen:?}");
    for s in [
        EngineStatus::Countdown,
        EngineStatus::Recording,
        EngineStatus::Paused,
        EngineStatus::Finalizing,
    ] {
        assert!(seen.contains(&s), "{s:?} in {seen:?}");
    }
    let p = ffprobe(&rec.path);
    let frames: u32 = p["nb_read_frames"].parse().unwrap();
    assert!((60..180).contains(&frames), "about 1.3 s recorded, got {frames} frames");

    // Reveal is limited to the output folder.
    assert!(shell.reveal_recording("/etc/hosts").is_err());

    shell.shutdown();
    let saved = std::fs::read_to_string(dir.join("settings.json")).expect("settings persisted");
    assert!(saved.contains("\"countdown\": 3"), "{saved}");
    let _ = std::fs::remove_dir_all(&dir);
}
