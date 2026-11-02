//! Regression test for the quit hang: the app's main thread ran `shutdown`, which joined
//! the ticker thread, while the ticker sat in the event sink waiting on that same main
//! thread (a Tauri window query). Neither could move.
//!
//! Here the sink blocks the ticker (and only the ticker) until the test lets it go, like
//! the main-thread wait did. `shutdown` must still return quickly and finalize the
//! recording into a playable file. Before the fix it never returned (the test timed out).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use rec_core::api::{EngineEvent, RecorderEngine};
use rec_core::settings::{Codec, Container, RecorderSettings, Scene};
use rec_core::state::EngineStatus;
use rec_engine::engine::{EngineOptions, MacEngine};

#[test]
fn shutdown_finalizes_while_the_ticker_is_stuck_in_a_sink() {
    let dir = std::env::temp_dir().join(format!("cortex-shutdown-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let settings = RecorderSettings {
        output_dir: dir.display().to_string(),
        container: Container::Mov,
        codec: Codec::H264,
        scene: Scene::Screen,
        mic_enabled: false,
        system_audio: false,
        countdown: 0,
        ..Default::default()
    };

    let block = Arc::new(AtomicBool::new(false));
    let stuck = Arc::new(AtomicBool::new(false));
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let (tx, rx) = mpsc::channel::<EngineEvent>();
    let tx = Mutex::new(tx);
    let events = {
        let (block, stuck) = (block.clone(), stuck.clone());
        Arc::new(move |e: EngineEvent| {
            let on_ticker = std::thread::current().name() == Some("engine-ticker");
            if on_ticker && block.load(Ordering::SeqCst) {
                stuck.store(true, Ordering::SeqCst);
                // Stands in for the main-thread wait: returns only when the test ends.
                let _ = release_rx.lock().map(|r| r.recv());
                return;
            }
            let _ = tx.lock().map(|t| t.send(e));
        })
    };
    let opts = EngineOptions {
        test_pattern: true,
        ..Default::default()
    };
    let engine = MacEngine::new(opts, settings.clone(), events);
    engine
        .start(&settings)
        .expect("start a test-pattern recording");

    let deadline = Instant::now() + Duration::from_secs(10);
    while engine.state().status != EngineStatus::Recording {
        assert!(Instant::now() < deadline, "the recording never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(1500));

    block.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !stuck.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the ticker never reached the sink"
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let started = Instant::now();
        engine.shutdown();
        let _ = done_tx.send(started.elapsed());
        drop(engine);
    });
    let took = done_rx
        .recv_timeout(Duration::from_secs(20))
        .expect("shutdown hung while the ticker was stuck in the event sink");
    eprintln!("shutdown took {} ms", took.as_millis());

    let recording = rx
        .try_iter()
        .find_map(|e| match e {
            EngineEvent::RecordingFinished { recording } => Some(recording),
            _ => None,
        })
        .expect("shutdown finalized the recording and reported it");
    let path = std::path::PathBuf::from(&recording.path);
    assert!(recording.bytes > 0, "empty recording");
    rec_engine::probe::probe(&path).expect("the finalized file plays and decodes");

    drop(release_tx);
    let _ = std::fs::remove_dir_all(&dir);
}
