mod app;

// The GUI (`app`) reaches the engine through `crate::<module>` paths.
use rec_engine::args::{RecordArgs, parse_record_args, prepare};
use rec_engine::{camera, compose, hotkey, live, mixer, outlatency, play, probe, record, scale, screen, settings, tcc, util, writer};

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rec-rs", version, about = "Native macOS screen recorder (ScreenCaptureKit + VideoToolbox + AVAssetWriter)")]
struct Cli {
    /// Without a subcommand, rec starts the menu bar app.
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

// RecordArgs is large but parsed once; boxing it buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Cmd {
    /// Record the screen (plus system audio, mic, and optional webcam overlay).
    Record(RecordArgs),
    /// The app: window with live preview and sources, settings, menu bar
    /// item and a global Cmd+Shift+R hotkey (what `open dist/Rec.app` starts).
    Gui {
        /// Start recording immediately (e.g. from a Login Item or script).
        #[arg(long)]
        start_recording: bool,
        /// Open the settings window at launch.
        #[arg(long)]
        settings: bool,
        /// Capture this window (ID from `rec-rs devices`) instead of the display.
        #[arg(long, value_name = "ID")]
        window: Option<u32>,
    },
    /// List displays, windows, cameras and microphones.
    Devices,
    /// Stop a running recording (CLI or app) and print its file.
    Stop,
    /// Save the replay buffer of the running recorder (CLI or app) to MP4.
    SaveReplay,
    /// Store a stream key in the macOS Keychain for a destination (reads the
    /// key from stdin; never echoed or logged).
    StreamKey {
        /// Stream URL (rtmp://, rtmps://, https://.../whip) or a service:
        /// youtube, twitch, kick.
        dest: String,
        /// Remove the stored key instead.
        #[arg(long)]
        delete: bool,
    },
    /// Pause the running recording (resume with `rec-rs resume`).
    Pause,
    /// Resume a paused recording.
    Resume,
    /// Show (and optionally request) screen, microphone and camera permissions.
    Permissions {
        /// Trigger the system permission prompts for anything not yet granted.
        #[arg(long)]
        request: bool,
        /// Seconds to wait for prompt answers when requesting.
        #[arg(long, default_value_t = 60)]
        wait: u64,
    },
    /// Request Screen & System Audio Recording (run by `permissions --request`).
    #[command(hide = true)]
    RequestScreen,
    /// Check a recording with AVFoundation (the engine QuickTime uses).
    Probe { file: PathBuf },
    /// Play a file in an AVPlayer window (reference player for sync tests).
    #[command(hide = true)]
    Play {
        file: PathBuf,
        #[arg(long, default_value_t = 15.0)]
        secs: f64,
        /// Window content size in points, WIDTHxHEIGHT.
        #[arg(long, default_value = "640x360")]
        size: String,
    },
}


/// `rec-rs stream-key DEST`: store (or delete) a stream key in the Keychain.
fn stream_key(dest: &str, delete: bool) -> Result<()> {
    let url = live::destination_url(dest);
    if delete {
        live::keychain::delete(&url)?;
        println!("removed the stream key for {url}");
        return Ok(());
    }
    let tty = unsafe { libc::isatty(0) } == 1;
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    if tty {
        eprint!("stream key for {url}: ");
        unsafe {
            libc::tcgetattr(0, &mut old);
            let mut t = old;
            t.c_lflag &= !libc::ECHO;
            libc::tcsetattr(0, libc::TCSANOW, &t);
        }
    }
    let mut key = String::new();
    let r = std::io::stdin().read_line(&mut key);
    if tty {
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &old) };
        eprintln!();
    }
    r?;
    let key = key.trim();
    if key.is_empty() {
        bail!("no key given");
    }
    live::keychain::set(&url, key)?;
    println!("saved the stream key for {url} in the Keychain");
    Ok(())
}

fn record(a: RecordArgs) -> Result<()> {
    let (opts, target) = prepare(a)?;
    let out = opts.output.clone();
    let quiet = opts.quiet;
    let wrote = opts.write;
    let stats = record::run(opts, target)?;
    if !wrote {
        if !quiet {
            eprintln!("rec: stopped ({} video frames, {} dropped)", stats.video_frames_out, stats.video_drops);
        }
        return Ok(());
    }
    if !quiet {
        eprintln!(
            "rec: saved {} ({} video frames, {} duplicated, {} dropped; audio buffers sys {} mic {} mix {}, {} dropped, {} clipped samples)",
            out.display(),
            stats.video_frames_out,
            stats.video_dups,
            stats.video_drops,
            stats.audio_sys_buffers,
            stats.audio_mic_buffers,
            stats.audio_mix_buffers,
            stats.audio_drops,
            stats.audio_clipped_samples
        );
        if stats.cam_track_frames > 0 {
            eprintln!("rec: camera track: {} frames, {} skipped", stats.cam_track_frames, stats.cam_track_drops);
        }
    }
    if std::env::var_os("REC_DEBUG").is_some() {
        let avg = |sum: f64, n: u64| if n == 0 { 0.0 } else { sum / n as f64 * 1000.0 };
        eprintln!(
            "rec: debug: encoder busy retries {}, audio gaps filled {}",
            stats.video_retries,
            stats.audio_gaps_filled
        );
        eprintln!(
            "rec: debug: frame arrival latency  screen avg {:.1} ms max {:.1} ms ({} frames)  camera avg {:.1} ms max {:.1} ms ({} frames)",
            avg(stats.screen_latency_sum, stats.screen_frames_in),
            stats.screen_latency_max * 1000.0,
            stats.screen_frames_in,
            avg(stats.cam_latency_sum, stats.cam_frames_in),
            stats.cam_latency_max * 1000.0,
            stats.cam_frames_in
        );
    }
    println!("{}", out.display());
    Ok(())
}

fn devices() -> Result<()> {
    println!("Displays:");
    match screen::displays() {
        Ok(ds) => {
            for (i, d) in ds.iter().enumerate() {
                println!(
                    "  [{}] id {}  {}x{} pixels ({}x{} points)",
                    i + 1,
                    d.id,
                    d.pixels.0, d.pixels.1, d.points.0, d.points.1
                );
            }
        }
        Err(e) => println!("  unavailable: {e}"),
    }
    println!("Windows (for --window):");
    match screen::windows() {
        Ok(ws) => {
            for w in ws {
                println!("  {:>6}  {} — {}  ({}x{} pt)", w.id, w.app, w.title, w.size.0, w.size.1);
            }
        }
        Err(e) => println!("  unavailable: {e}"),
    }
    println!("Cameras:");
    for c in camera::cameras() {
        println!("  {}  [{}]", c.name, c.id);
    }
    println!("Microphones:");
    for m in camera::microphones() {
        println!("  {}  [{}]", m.name, m.id);
    }
    Ok(())
}

fn main() {
    // Behave like a normal CLI when piped into `head` etc.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    tcc::disclaim_and_reexec();
    let cli = Cli::parse();
    let r = match cli.cmd.unwrap_or(Cmd::Gui { start_recording: false, settings: false, window: None }) {
        Cmd::Gui { start_recording, settings, window } => app::run(start_recording, settings, window),
        Cmd::Record(a) => record(a),
        Cmd::Devices => {
            let r = devices();
            if let Some(l) = outlatency::output_latency() {
                println!("Output latency (system audio is shifted by this): {:.1} ms", l * 1000.0);
            }
            r
        }
        Cmd::Stop => record::stop_running(),
        Cmd::Pause => record::signal_running(libc::SIGUSR1),
        Cmd::Resume => record::signal_running(libc::SIGUSR2),
        Cmd::SaveReplay => record::signal_running(libc::SIGINFO),
        Cmd::StreamKey { dest, delete } => stream_key(&dest, delete),
        Cmd::Permissions { request, wait } => {
            if request {
                tcc::request_all(true, true, true, std::time::Duration::from_secs(wait));
            }
            let (s, m, c) = tcc::report();
            println!("screen recording : {s}");
            println!("microphone       : {m}");
            println!("camera           : {c}");
            Ok(())
        }
        Cmd::RequestScreen => {
            tcc::request_screen_here();
            Ok(())
        }
        Cmd::Probe { file } => probe::probe(&file),
        Cmd::Play { file, secs, size } => play::play(&file, secs, &size),
    };
    if let Err(e) = r {
        eprintln!("rec: error: {e:#}");
        std::process::exit(1);
    }
}
