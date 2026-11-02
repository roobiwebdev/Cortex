//! Drive `MacEngine` through the `RecorderEngine` API with Rec's test pattern
//! (no Screen Recording permission, mic off) for 3 s, then check the file with
//! ffprobe: H.264, 1920x1080, 60 fps, 180 frames.
//!
//! cargo run --release -p rec-engine-macos --example engine_smoke [OUTPUT_DIR]

use std::process::Command;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rec_core::api::{EngineEvent, RecorderEngine};
use rec_core::settings::{Codec, Container, RecorderSettings, Resolution, Scene};
use rec_engine::engine::{EngineOptions, MacEngine};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("cortex-engine-smoke"));
    let settings = RecorderSettings {
        output_dir: dir.display().to_string(),
        container: Container::Mov,
        codec: Codec::H264,
        resolution: Resolution::R1080p,
        fps: 60,
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
        max_duration_secs: Some(3.0),
        ..Default::default()
    };
    let engine = MacEngine::new(opts, settings.clone(), events);
    engine.start(&settings).expect("start");
    println!("smoke: status after start = {:?}", engine.state().status);

    let (mut levels, mut progress, mut states) = (0, 0, vec![]);
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let recording = loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(left) {
            Ok(EngineEvent::Levels(_)) => levels += 1,
            Ok(EngineEvent::Progress(_)) => progress += 1,
            Ok(EngineEvent::StateChanged(s)) => states.push(s.status),
            Ok(EngineEvent::RecordingFinished { recording }) => break recording,
            Ok(EngineEvent::Error {
                message,
                recoverable,
            }) => {
                eprintln!("smoke: error (recoverable={recoverable}): {message}");
                if !recoverable {
                    std::process::exit(1);
                }
            }
            Ok(_) => {}
            Err(_) => {
                eprintln!("smoke: FAIL no RecordingFinished within 30 s");
                std::process::exit(1);
            }
        }
    };
    // The Idle StateChanged follows RecordingFinished.
    while let Ok(e) = rx.recv_timeout(Duration::from_millis(300)) {
        if let EngineEvent::StateChanged(s) = e {
            states.push(s.status);
        }
    }
    println!("smoke: states {states:?}; {levels} level and {progress} progress events");
    println!(
        "smoke: recording {} ({:.3} s, {} bytes, {}x{} @ {} fps, {} audio track(s))",
        recording.path,
        recording.duration_secs,
        recording.bytes,
        recording.width,
        recording.height,
        recording.fps,
        recording.audio_tracks
    );

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
            &recording.path,
        ])
        .output()
        .expect("run ffprobe");
    let text = String::from_utf8_lossy(&out.stdout);
    println!("smoke: ffprobe\n{}", text.trim());
    let field = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .unwrap_or("")
            .to_string()
    };
    let checks = [
        ("codec_name", "h264"),
        ("width", "1920"),
        ("height", "1080"),
        ("r_frame_rate", "60/1"),
        ("nb_read_frames", "180"),
    ];
    let mut ok = out.status.success();
    for (k, want) in checks {
        let got = field(k);
        if got != want {
            eprintln!("smoke: FAIL {k} = {got:?}, want {want:?}");
            ok = false;
        }
    }
    if let Err(e) = rec_engine::probe::probe(std::path::Path::new(&recording.path)) {
        eprintln!("smoke: FAIL AVFoundation probe: {e:#}");
        ok = false;
    }
    engine.shutdown();
    if ok {
        println!("smoke: PASS");
    } else {
        std::process::exit(1);
    }
}
