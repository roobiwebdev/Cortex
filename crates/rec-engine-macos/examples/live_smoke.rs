//! Drive Go live through the `RecorderEngine` API with Rec's test pattern
//! (no Screen Recording permission, mic off) against LOCAL test servers only.
//! Prints every stream state change and error so reconnects can be watched.
//!
//! REC_STREAM_KEY=test cargo run --release -p rec-engine-macos --example live_smoke -- \
//!     [--url rtmp://127.0.0.1:1935/live]... [--secs 30] [--record] [--replay]
//!     [--codec h264|hevc|auto] [--out DIR]
//!
//! `--record` records a file for the whole run while streaming; `--replay`
//! turns the replay buffer on and saves it half-way through.

use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use rec_core::api::{EngineEvent, RecorderEngine};
use rec_core::settings::{
    Codec, Container, RecorderSettings, Resolution, Scene, StreamCodec, StreamDest, StreamService,
};
use rec_engine::engine::{EngineOptions, MacEngine};

fn main() {
    let mut urls = vec![];
    let (mut secs, mut record, mut replay) = (30.0f64, false, false);
    let mut codec = StreamCodec::H264;
    let mut out = std::env::temp_dir().join("cortex-live-smoke");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--url" => urls.extend(args.next()),
            "--secs" => secs = args.next().and_then(|s| s.parse().ok()).unwrap_or(secs),
            "--record" => record = true,
            "--replay" => replay = true,
            "--codec" => {
                codec = args
                    .next()
                    .and_then(|s| StreamCodec::parse(&s))
                    .unwrap_or_default()
            }
            "--out" => out = args.next().map(Into::into).unwrap_or(out),
            other => panic!("unknown argument {other}"),
        }
    }
    if urls.is_empty() {
        urls.push("rtmp://127.0.0.1:1935/live".into());
    }
    let settings = RecorderSettings {
        output_dir: out.display().to_string(),
        container: Container::Mp4,
        codec: Codec::H264,
        resolution: Resolution::R1080p,
        fps: 60,
        scene: Scene::Screen,
        mic_enabled: false,
        system_audio: true,
        countdown: 0,
        replay_enabled: replay,
        replay_secs: 15,
        stream_dests: urls
            .iter()
            .map(|u| StreamDest {
                service: StreamService::Custom,
                url: Some(u.clone()),
                enabled: true,
                key_stored: false,
            })
            .collect(),
        live_resolution: Resolution::R1080p,
        stream_bitrate_kbps: 6000,
        stream_bitrate_auto: false,
        stream_codec: codec,
        keyframe_secs: 2.0,
        stream_cbr: true,
        stream_audio_kbps: 192,
        ..Default::default()
    };
    let (tx, rx) = mpsc::channel();
    let events = Arc::new(move |e: EngineEvent| {
        let _ = tx.send(e);
    });
    let opts = EngineOptions {
        test_pattern: true,
        ..Default::default()
    };
    let engine = MacEngine::new(opts, settings.clone(), events);
    let t0 = Instant::now();
    let log = |msg: String| println!("live: {:6.2}s {msg}", t0.elapsed().as_secs_f64());
    if let Err(e) = engine.go_live() {
        eprintln!("live: FAIL go_live: {e:?}");
        std::process::exit(1);
    }
    log("go_live ok".into());
    if record {
        engine.start(&settings).expect("start recording");
        log("recording started".into());
    }
    let (mut last_streams, mut errors, mut replay_saved) = (String::new(), 0, None);
    let mut replay_asked = false;
    let end = t0 + Duration::from_secs_f64(secs);
    while Instant::now() < end {
        if replay && !replay_asked && t0.elapsed().as_secs_f64() > secs / 2.0 {
            replay_asked = true;
            match engine.save_replay() {
                Ok(()) => log("save_replay requested".into()),
                Err(e) => log(format!("save_replay FAILED: {e:?}")),
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(EngineEvent::StateChanged(s)) => {
                let now = format!("{:?}", s.streams);
                if now != last_streams {
                    log(format!("live={} streams {now}", s.live));
                    last_streams = now;
                }
            }
            Ok(EngineEvent::Error {
                message,
                recoverable,
            }) => {
                errors += 1;
                log(format!("error (recoverable={recoverable}): {message}"));
            }
            Ok(EngineEvent::ReplaySaved { path }) => {
                log(format!("replay saved {path}"));
                replay_saved = Some(path);
            }
            Ok(EngineEvent::RecordingFinished { recording }) => {
                log(format!("recording finished early {}", recording.path))
            }
            _ => {}
        }
    }
    if record {
        engine.stop().expect("stop recording");
    }
    engine.stop_live().expect("stop_live");
    log("stop_live ok".into());
    let wait = Instant::now() + Duration::from_secs(15);
    while Instant::now() < wait {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(EngineEvent::RecordingFinished { recording }) => log(format!(
                "recording {} ({:.2} s, {}x{})",
                recording.path, recording.duration_secs, recording.width, recording.height
            )),
            Ok(EngineEvent::ReplaySaved { path }) => {
                log(format!("replay saved {path}"));
                replay_saved = Some(path);
            }
            Ok(EngineEvent::Error { message, .. }) => {
                errors += 1;
                log(format!("error: {message}"));
            }
            _ => {}
        }
        if (!record || engine.state().status == rec_core::state::EngineStatus::Idle)
            && (!replay || replay_saved.is_some())
        {
            break;
        }
    }
    engine.shutdown();
    log(format!(
        "done: {errors} error event(s), replay {replay_saved:?}"
    ));
}
