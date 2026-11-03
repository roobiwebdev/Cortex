//! Debug builds only: drive Go live through the real IPC commands, the way the
//! Go live popover and Settings > Live do, against a LOCAL test server.
//!
//! `CORTEX_DEBUG_LIVE_URL=rtmp://127.0.0.1:19351/live` (a custom destination),
//! `CORTEX_DEBUG_LIVE_KEY=test` (optional; stored with `set_stream_key` and removed
//! with `remove_stream_key` afterwards), `CORTEX_DEBUG_LIVE_SECS=N` (default 20).
//! Use with `CORTEX_DEBUG_DATA_DIR` and `CORTEX_DEBUG_TEST_PATTERN=1`. Prints
//! `cortex-live:` lines with the engine's stream states, then quits.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rec_core::api::RecorderEngine;
use tauri::Manager;

use crate::shell::Shell;

/// The webview script: settings, key, go_live, wait, stop_live, key removal, all
/// through `invoke` (so the capability allow-list applies). Failures go to a toast.
fn script(url: &str, key: Option<&str>, secs: f64) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap_or_default();
    format!(
        r#"(async () => {{
  const inv = window.__TAURI_INTERNALS__.invoke;
  const dest = {{ service: "custom", url: {url}, enabled: true, keyStored: false }};
  const key = {key};
  try {{
    const s = await inv("get_settings");
    Object.assign(s, {{ streamDests: [dest], streamCodec: "h264", streamBitrateAuto: false,
      streamBitrateKbps: 6000, keyframeSecs: 2, streamCbr: true, streamAudioKbps: 192,
      liveResolution: "1080p", fps: 60, micEnabled: false }});
    await inv("update_settings", {{ settings: s }});
    if (key) {{
      const back = await inv("set_stream_key", {{ dest, key }});
      if (JSON.stringify(back).includes(key)) throw new Error("key came back to the webview");
    }}
    await inv("go_live");
    await new Promise((r) => setTimeout(r, {ms}));
    await inv("stop_live");
  }} catch (e) {{
    await inv("push_toast", {{ toast: {{ id: "debug-live", kind: "warning", title: "Debug live failed", detail: JSON.stringify(e) }} }}).catch(() => {{}});
  }} finally {{
    if (key) await inv("remove_stream_key", {{ dest }}).catch(() => {{}});
  }}
}})();"#,
        url = q(url),
        key = key.map_or("null".into(), q),
        ms = (secs * 1000.0) as u64,
    )
}

/// Starts the run 4 s after launch if `CORTEX_DEBUG_LIVE_URL` is set.
pub fn maybe_start(app: &tauri::AppHandle, shell: Arc<Shell>) {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let Some(url) = var("CORTEX_DEBUG_LIVE_URL") else {
        return;
    };
    let key = var("CORTEX_DEBUG_LIVE_KEY");
    let secs = var("CORTEX_DEBUG_LIVE_SECS")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(20.0);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        let js = script(&url, key.as_deref(), secs);
        match app.get_webview_window("main") {
            Some(w) => {
                if let Err(e) = w.eval(&js) {
                    eprintln!("cortex-live: eval failed: {e}");
                }
            }
            None => eprintln!("cortex-live: no main window"),
        }
        let t0 = Instant::now();
        let (mut last, mut was_live, mut key_seen) = (String::new(), false, false);
        // Until the stream has started and stopped again (or a time-out).
        while t0.elapsed().as_secs_f64() < secs + 20.0 {
            let st = shell.engine().state();
            let now = format!("live={} streams={:?}", st.live, st.streams);
            if now != last {
                println!("cortex-live: {:6.2}s {now}", t0.elapsed().as_secs_f64());
                last = now;
            }
            let stored = shell.settings().stream_dests.iter().any(|d| d.key_stored);
            if stored && !key_seen {
                key_seen = true;
                println!("cortex-live: key stored in the Keychain (keyStored=true)");
            }
            was_live |= st.live;
            if was_live && !st.live {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        std::thread::sleep(Duration::from_secs(2));
        let dest = shell.settings().stream_dests.first().cloned();
        let left = dest.as_ref().is_some_and(|d| {
            rec_engine::live::keychain::get(&rec_engine::engine::dest_url(d)).is_some()
        });
        println!("cortex-live: done (went live: {was_live}; key still in Keychain: {left})");
        app.exit(0);
    });
}
