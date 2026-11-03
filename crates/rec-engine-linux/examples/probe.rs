//! What the Linux engine finds on this PC, and short checks of its parts.
//!
//!   cargo run -p rec-engine-linux --example probe              # capabilities, encoders, devices
//!   cargo run -p rec-engine-linux --example probe -- secrets   # Secret Service round trip
//!   cargo run -p rec-engine-linux --example probe -- audio 4   # 4 s: test video + real system audio + mic
//!   cargo run -p rec-engine-linux --example probe -- portal 5  # 5 s of a portal-picked screen
//!   cargo run -p rec-engine-linux --example probe -- engine 5  # LinuxEngine, test pattern, 5 s (0 = until killed)
//!
//! Recordings go to the current folder; check them with ffprobe.

#[cfg(target_os = "linux")]
fn main() {
    use rec_core::platform::Secrets;
    use rec_core::settings::{RecorderSettings, Scene};
    use rec_engine_linux::encoders::{AAC_ENCODERS, H264_ENCODERS, HEVC_ENCODERS};
    use rec_engine_linux::recording::{ScreenSource, Sources, plan_recording};
    use rec_engine_linux::session::{MaskSpec, Session, has_factory, init};
    use std::time::Duration;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
    if !init() {
        eprintln!("GStreamer could not start");
        std::process::exit(1);
    }
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    match args.first().map(String::as_str) {
        None | Some("info") => {
            println!("session: {session_type:?} desktop: {desktop:?}");
            println!("gstreamer: {}", gstreamer::version_string());
            for (what, list) in [
                ("h264", &H264_ENCODERS[..]),
                ("hevc", &HEVC_ENCODERS[..]),
                ("aac", &AAC_ENCODERS[..]),
            ] {
                let found: Vec<_> = list.iter().filter(|f| has_factory(f)).collect();
                println!("{what} encoders installed: {found:?}");
            }
            for f in [
                "pipewiresrc",
                "pulsesrc",
                "v4l2src",
                "compositor",
                "mp4mux",
                "qtmux",
            ] {
                println!("{f}: {}", has_factory(f));
            }
            println!(
                "GlobalShortcuts portal: {}",
                rec_engine_linux::desktop::global_shortcuts_available()
            );
            match rec_engine_linux::devices::DeviceList::start() {
                Some(d) => {
                    std::thread::sleep(Duration::from_millis(500));
                    let (cams, mics) = d.current();
                    println!("cameras: {cams:#?}");
                    println!("mics: {mics:#?}");
                }
                None => println!("no device monitor"),
            }
        }
        Some("secrets") => {
            let s = rec_engine_linux::SecretService;
            let account = "cortex-probe-test";
            match s.set(account, "probe-secret") {
                Ok(()) => println!("set: ok"),
                Err(e) => {
                    println!("set failed: {e}");
                    std::process::exit(1);
                }
            }
            println!("get: {:?}", s.get(account).map(|v| v == "probe-secret"));
            println!("delete: {:?}", s.delete(account));
            println!("get after delete: {:?}", s.get(account));
        }
        Some(mode @ ("audio" | "portal")) => {
            let mut keep: Option<Box<dyn Send>> = None;
            let screen = if mode == "portal" {
                let picked = rec_engine_linux::portal::pick(
                    rec_engine_linux::portal_state::PickKind::Monitor,
                    true,
                    None,
                    Duration::from_secs(120),
                )
                .unwrap_or_else(|e| {
                    eprintln!("portal: {e}");
                    std::process::exit(1)
                });
                let size = picked.stream.size.unwrap_or((1920, 1080));
                println!("portal stream: node {} size {size:?}", picked.stream.node);
                let src = picked.stream.source();
                keep = Some(Box::new(picked.stream));
                ScreenSource {
                    src,
                    size,
                    crop: None,
                }
            } else {
                ScreenSource {
                    src: "videotestsrc is-live=true ! video/x-raw,width=1280,height=720,framerate=30/1"
                        .into(),
                    size: (1280, 720),
                    crop: None,
                }
            };
            let sources = Sources {
                screen: Some(screen),
                camera: None,
                system: Some("pulsesrc device=@DEFAULT_MONITOR@".into()),
                mic: Some("pulsesrc".into()),
            };
            let s = RecorderSettings {
                scene: Scene::Screen,
                fps: 30,
                ..RecorderSettings::default()
            };
            let planned = plan_recording(&s, &sources, &has_factory).unwrap_or_else(|e| {
                eprintln!("plan: {e}");
                std::process::exit(1)
            });
            let file = std::env::current_dir()
                .unwrap_or_default()
                .join(format!("cortex-probe-{mode}.mov"));
            let sess = Session::start(&planned, &file, MaskSpec::from_settings(&s, 720), keep)
                .unwrap_or_else(|e| {
                    eprintln!("start: {e}");
                    std::process::exit(1)
                });
            let mut peak = [0f32; 2];
            for _ in 0..secs * 10 {
                std::thread::sleep(Duration::from_millis(100));
                let l = sess.take_levels();
                peak = [peak[0].max(l[0]), peak[1].max(l[1])];
            }
            let stats = sess.finish();
            println!("file: {}", file.display());
            println!("stats: {stats:?}");
            println!("peak levels [system, mic]: {peak:?}");
        }
        Some("engine") => {
            use rec_core::api::{EngineEvent, RecorderEngine};
            use std::sync::Arc;
            let home = std::env::current_dir().unwrap_or_default();
            let s = RecorderSettings {
                output_dir: home.display().to_string(),
                countdown: 0,
                fps: 30,
                resolution: rec_core::settings::Resolution::R720p,
                ..RecorderSettings::default()
            };
            let (tx, rx) = std::sync::mpsc::channel();
            let tx = std::sync::Mutex::new(tx);
            let events: rec_core::api::EventSink = Arc::new(move |e: EngineEvent| {
                if !matches!(e, EngineEvent::Levels(_) | EngineEvent::Progress(_))
                    && let Ok(t) = tx.lock()
                {
                    let _ = t.send(e);
                }
            });
            let opts = rec_core::platform::EngineOptions {
                home,
                test_pattern: true,
                max_duration_secs: (secs > 0).then_some(secs as f64),
                ..Default::default()
            };
            let engine = rec_engine_linux::LinuxEngine::new(opts, s.clone(), events);
            println!("capabilities: {:?}", engine.capabilities());
            if let Err(e) = engine.start(&s) {
                eprintln!("start: {e}");
                std::process::exit(1);
            }
            println!("recording: {:?}", engine.state().status);
            while let Ok(e) = rx.recv() {
                println!("event: {e:?}");
                if matches!(e, EngineEvent::RecordingFinished { .. }) {
                    break;
                }
            }
            engine.shutdown();
        }
        Some(other) => {
            eprintln!("unknown command {other}");
            std::process::exit(2);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the probe runs on Linux only");
}
