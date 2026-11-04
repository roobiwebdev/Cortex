//! Headless runtime tests of the Linux engine: real GStreamer pipelines fed by the test
//! pattern (`videotestsrc`, `audiotestsrc`) through `RecorderEngine`, checked by reading the
//! files back with `GstMedia`. Screen capture through the portal needs a desktop session and
//! is not covered here. Set `CORTEX_KEEP_TEST_FILES=1` to keep the recordings for ffprobe.
#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rec_core::api::{EngineEvent, RecorderEngine};
use rec_core::platform::{EngineOptions, MediaLibrary};
use rec_core::settings::{AudioTracks, Codec, Container, RecorderSettings, Resolution, Scene};
use rec_core::state::{EngineStatus, Recording};
use rec_engine_linux::{GstMedia, LinuxEngine};

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cortex-linux-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Recordings run one at a time, as in the app: several at once would compete for the CPU
/// and each would drop frames that a lone recording keeps.
static ONE_RECORDING: Mutex<()> = Mutex::new(());

struct Rig {
    engine: LinuxEngine,
    events: mpsc::Receiver<EngineEvent>,
    levels: Arc<Mutex<[f32; 2]>>,
    _turn: std::sync::MutexGuard<'static, ()>,
}

fn rig(home: &Path, max_secs: Option<f64>) -> (Rig, RecorderSettings) {
    let turn = ONE_RECORDING.lock().unwrap_or_else(|p| p.into_inner());
    let (tx, rx) = mpsc::channel();
    let levels = Arc::new(Mutex::new([0f32; 2]));
    let lv = levels.clone();
    let tx = Mutex::new(tx);
    let sink: rec_core::api::EventSink = Arc::new(move |e: EngineEvent| {
        if let EngineEvent::Levels(l) = &e {
            let mut g = lv.lock().unwrap();
            g[0] = g[0].max(l.system);
            g[1] = g[1].max(l.mic);
            return;
        }
        let _ = tx.lock().unwrap().send(e);
    });
    let settings = RecorderSettings {
        output_dir: home.join("out").display().to_string(),
        countdown: 0,
        fps: 30,
        resolution: Resolution::R720p,
        scene: Scene::ScreenCam,
        camera_track: true,
        mic_enabled: true,
        system_audio: true,
        audio_tracks: AudioTracks::Both,
        container: Container::Mp4,
        ..RecorderSettings::default()
    };
    let opts = EngineOptions {
        home: home.to_path_buf(),
        test_pattern: true,
        max_duration_secs: max_secs,
        ..EngineOptions::default()
    };
    let engine = LinuxEngine::new(opts, settings.clone(), sink);
    (
        Rig {
            engine,
            events: rx,
            levels,
            _turn: turn,
        },
        settings,
    )
}

fn wait_finished(rx: &mpsc::Receiver<EngineEvent>, timeout: Duration) -> Recording {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(EngineEvent::RecordingFinished { recording }) => return recording,
            Ok(EngineEvent::Error {
                message,
                recoverable: false,
            }) => panic!("engine error: {message}"),
            _ => {}
        }
    }
    panic!("no RecordingFinished within {timeout:?}");
}

/// Frames in video track `track` of `path`, counted from the demuxed packets.
fn video_frames(path: &Path, track: u32) -> u64 {
    use gstreamer as gst;
    use gstreamer::prelude::*;
    assert!(rec_engine_linux::session::init(), "GStreamer");
    let text = format!(
        "filesrc location=\"{}\" ! qtdemux name=d d.video_{track} ! appsink name=snk sync=false",
        path.display()
    );
    let p = gst::parse::launch(&text)
        .unwrap()
        .downcast::<gst::Pipeline>()
        .unwrap();
    let sink = p
        .by_name("snk")
        .unwrap()
        .downcast::<gstreamer_app::AppSink>()
        .unwrap();
    p.set_state(gst::State::Playing).unwrap();
    let mut n = 0;
    while sink
        .try_pull_sample(gst::ClockTime::from_seconds(5))
        .is_some()
    {
        n += 1;
    }
    let _ = p.set_state(gst::State::Null);
    n
}

fn keep() -> bool {
    std::env::var_os("CORTEX_KEEP_TEST_FILES").is_some()
}

#[test]
fn records_screen_camera_and_three_audio_tracks_with_a_pause() {
    let home = temp_dir("full");
    let (r, settings) = rig(&home, None);
    assert!(
        r.engine.capabilities().recording,
        "GStreamer encoders found"
    );
    r.engine.start(&settings).expect("start");
    let started = Instant::now();
    assert_eq!(r.engine.state().status, EngineStatus::Recording);
    std::thread::sleep(Duration::from_millis(2000));
    r.engine.pause().expect("pause");
    let paused = Instant::now();
    assert_eq!(r.engine.state().status, EngineStatus::Paused);
    let at_pause = r.engine.state().elapsed_secs;
    std::thread::sleep(Duration::from_millis(1500));
    let during = r.engine.state().elapsed_secs;
    assert!(
        (during - at_pause).abs() < 0.2,
        "clock frozen while paused: {at_pause} -> {during}"
    );
    r.engine.resume().expect("resume");
    let resumed = Instant::now();
    std::thread::sleep(Duration::from_millis(2000));
    r.engine.stop().expect("stop");
    let stopped = Instant::now();
    let rec = wait_finished(&r.events, Duration::from_secs(30));
    // What this thread did, to tell a slow test from a wrong engine.
    println!(
        "recorded {:.3} s, paused {:.3} s, recorded {:.3} s",
        (paused - started).as_secs_f64(),
        (resumed - paused).as_secs_f64(),
        (stopped - resumed).as_secs_f64()
    );
    println!("recording: {rec:?}");
    assert_eq!((rec.width, rec.height), (1280, 720));
    assert_eq!(rec.audio_tracks, 3);
    assert!(rec.has_camera_track);
    // 4 s recorded around a 1.5 s pause.
    assert!(
        (3.4..=4.8).contains(&rec.duration_secs),
        "pause cut out: {}",
        rec.duration_secs
    );
    let levels = *r.levels.lock().unwrap();
    assert!(
        levels[0] > 0.1 && levels[1] > 0.1,
        "meters moved: {levels:?}"
    );

    let info = GstMedia.inspect(Path::new(&rec.path)).expect("inspect");
    println!("inspect: {info:?}");
    assert_eq!(info.video_tracks, 2, "screen + camera track");
    assert_eq!(info.audio.len(), 3, "mix, system, mic");
    assert!(info.audio.iter().all(|a| a.channels == 2));
    assert_eq!((info.width, info.height), (1280, 720));
    assert!(
        (3.4..=4.8).contains(&info.duration_secs),
        "file duration {}",
        info.duration_secs
    );
    assert!(
        (rec.duration_secs - info.duration_secs).abs() < 0.1,
        "reported {} s, the file holds {} s",
        rec.duration_secs,
        info.duration_secs
    );
    assert!(!info.hevc);
    // Video may drop frames on a machine too slow for real time, but never most of them:
    // at least half the frames the recorded time calls for, in each video track.
    let expected = rec.duration_secs * 30.0;
    for track in 0..2 {
        let frames = video_frames(Path::new(&rec.path), track);
        println!("video track {track}: {frames} frames of {expected:.0}");
        assert!(
            frames as f64 >= expected * 0.5,
            "video track {track} kept {frames} of {expected:.0} frames"
        );
    }

    let thumbs = GstMedia
        .thumbnails(Path::new(&rec.path), 4, 160.0)
        .expect("thumbnails");
    assert_eq!(thumbs.len(), 4);
    assert!(thumbs.iter().all(|j| j.starts_with(&[0xFF, 0xD8])), "JPEGs");
    let wave = GstMedia
        .waveform(Path::new(&rec.path), 50)
        .expect("waveform");
    assert_eq!(wave.len(), 50);
    assert!(wave.iter().filter(|p| **p > 0.1).count() > 40, "{wave:?}");

    r.engine.shutdown();
    if keep() {
        println!("kept: {}", rec.path);
    } else {
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[test]
fn discard_deletes_the_file() {
    let home = temp_dir("discard");
    let (r, settings) = rig(&home, None);
    r.engine.start(&settings).expect("start");
    std::thread::sleep(Duration::from_millis(1200));
    r.engine.discard().expect("discard");
    let deadline = Instant::now() + Duration::from_secs(10);
    while r.engine.state().status != EngineStatus::Idle && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(r.engine.state().status, EngineStatus::Idle);
    let left: Vec<_> = std::fs::read_dir(home.join("out"))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "discarded file removed: {left:?}");
    r.engine.shutdown();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn countdown_then_auto_stop_at_max_duration_mov_mix_only() {
    let home = temp_dir("countdown");
    let (r, mut settings) = rig(&home, Some(2.0));
    settings.countdown = 3;
    settings.container = Container::Mov;
    settings.audio_tracks = AudioTracks::Mix;
    settings.scene = Scene::Screen;
    r.engine.start(&settings).expect("start");
    assert_eq!(r.engine.state().status, EngineStatus::Countdown);
    let rec = wait_finished(&r.events, Duration::from_secs(30));
    assert!(rec.path.ends_with(".mov"));
    assert_eq!(rec.audio_tracks, 1);
    assert!(!rec.has_camera_track);
    assert!(
        (1.8..=2.6).contains(&rec.duration_secs),
        "{}",
        rec.duration_secs
    );
    let info = GstMedia.inspect(Path::new(&rec.path)).expect("inspect");
    assert_eq!(info.video_tracks, 1);
    assert_eq!(info.audio.len(), 1);
    r.engine.shutdown();
    if !keep() {
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[test]
fn hevc_when_an_encoder_exists() {
    let home = temp_dir("hevc");
    let (r, mut settings) = rig(&home, Some(1.5));
    if !r.engine.capabilities().hevc {
        println!("no HEVC encoder installed; skipped");
        return;
    }
    settings.codec = Codec::Hevc;
    settings.scene = Scene::Screen;
    settings.mic_enabled = false;
    r.engine.start(&settings).expect("start");
    let rec = wait_finished(&r.events, Duration::from_secs(40));
    let info = GstMedia.inspect(Path::new(&rec.path)).expect("inspect");
    assert!(info.hevc, "{info:?}");
    assert_eq!(info.audio.len(), 1, "system only");
    r.engine.shutdown();
    if !keep() {
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[test]
fn trash_moves_a_recording_out_of_the_folder() {
    let home = temp_dir("trash");
    let file = home.join("rec-x.mp4");
    std::fs::write(&file, b"x").unwrap();
    let trash = rec_engine_linux::trash::trash_dir(&home, None);
    rec_engine_linux::trash::move_to_trash(&file, &trash, "2026-11-03T10:00:00").unwrap();
    assert!(!file.exists());
    assert!(trash.join("files/rec-x.mp4").exists());
    let _ = std::fs::remove_dir_all(&home);
}
