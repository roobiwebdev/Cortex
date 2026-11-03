//! SOURCE OF TRUTH KEYWORDS: reveal, FileManager1 ShowItems, xdg-open, GlobalShortcuts portal, global shortcuts, shortcut activation
//! WHAT:  The desktop calls the Tauri shell makes on Linux: show a file selected in the file
//!        manager (`org.freedesktop.FileManager1.ShowItems`, else `xdg-open` on its folder),
//!        open a folder, and system-wide shortcuts through the GlobalShortcuts portal.
//! WHY:   Wayland has no global key grabs; the portal (KDE Plasma 5.27+, GNOME 48+) binds
//!        shortcuts the user confirms once. D-Bus and the portals are async (zbus), so each
//!        call runs on a short-lived or dedicated thread; the shell only sees blocking calls
//!        and a `Send` activation callback.
//! WHERE: `apps/desktop/src-tauri/src/platform/linux.rs`.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;

/// `file://` URI with the path percent-encoded.
pub fn file_uri(path: &Path) -> String {
    format!(
        "file://{}",
        crate::trash::encode_path(&path.to_string_lossy())
    )
}

/// Run an async block on its own thread with a timeout (D-Bus calls must not hang the UI).
fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    f: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(pollster::block_on(f));
    });
    rx.recv_timeout(timeout)
        .map_err(|_| "the desktop did not answer in time".to_string())?
}

/// Show `file` selected in the file manager; falls back to opening its folder.
pub fn reveal(file: &Path) -> Result<(), String> {
    let uri = file_uri(file);
    let shown = run_with_timeout(Duration::from_secs(3), async move {
        let conn = ashpd::zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        conn.call_method(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            Some("org.freedesktop.FileManager1"),
            "ShowItems",
            &(vec![uri.as_str()], ""),
        )
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
    });
    match shown {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("cortex: FileManager1.ShowItems failed ({e}); opening the folder");
            open_folder(file.parent().unwrap_or(file))
        }
    }
}

/// Open a folder in the default file manager.
pub fn open_folder(dir: &Path) -> Result<(), String> {
    let status = Command::new("xdg-open")
        .arg(dir)
        .status()
        .map_err(|e| format!("Could not run xdg-open: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("xdg-open could not open {}", dir.display()))
    }
}

/// Whether the GlobalShortcuts portal exists on this desktop (a quick D-Bus check).
/// Reads the interface's `version` property, which fails when no portal backend provides it.
pub fn global_shortcuts_available() -> bool {
    run_with_timeout(Duration::from_secs(2), async {
        let conn = ashpd::zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        let reply = conn
            .call_method(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                Some("org.freedesktop.DBus.Properties"),
                "Get",
                &("org.freedesktop.portal.GlobalShortcuts", "version"),
            )
            .await
            .map_err(|e| e.to_string())?;
        let v: ashpd::zvariant::OwnedValue =
            reply.body().deserialize().map_err(|e| e.to_string())?;
        Ok(u32::try_from(v).unwrap_or(0) > 0)
    })
    .unwrap_or(false)
}

/// One shortcut to bind: id, description shown by the desktop, preferred trigger
/// (`CTRL+SHIFT+r`, the portal's shortcuts syntax).
#[derive(Clone, Debug)]
pub struct ShortcutSpec {
    pub id: String,
    pub description: String,
    pub trigger: String,
}

/// Bind `shortcuts` through the portal and call `on_activate(id)` (on a portal thread)
/// whenever one fires, for the rest of the process. The desktop may ask the user to confirm
/// or change the keys. Returns the triggers the desktop actually assigned.
pub fn bind_global_shortcuts(
    shortcuts: Vec<ShortcutSpec>,
    on_activate: Arc<dyn Fn(&str) + Send + Sync>,
) -> Result<Vec<(String, String)>, String> {
    let (tx, rx) = mpsc::channel::<Result<Vec<(String, String)>, String>>();
    std::thread::Builder::new()
        .name("global-shortcuts".into())
        .spawn(move || {
            pollster::block_on(async move {
                let setup = async {
                    let portal = GlobalShortcuts::new().await?;
                    let session = portal.create_session(Default::default()).await?;
                    let new: Vec<NewShortcut> = shortcuts
                        .iter()
                        .map(|s| {
                            NewShortcut::new(s.id.clone(), s.description.clone())
                                .preferred_trigger(Some(s.trigger.as_str()))
                        })
                        .collect();
                    let bound = portal
                        .bind_shortcuts(&session, &new, None, Default::default())
                        .await?
                        .response()?;
                    let assigned = bound
                        .shortcuts()
                        .iter()
                        .map(|s| (s.id().to_string(), s.trigger_description().to_string()))
                        .collect::<Vec<_>>();
                    let activated = portal.receive_activated().await?;
                    Ok::<_, ashpd::Error>((portal, session, assigned, activated))
                };
                match setup.await {
                    Ok((_portal, _session, assigned, mut activated)) => {
                        let _ = tx.send(Ok(assigned));
                        while let Some(a) = activated.next().await {
                            on_activate(a.shortcut_id());
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("GlobalShortcuts portal: {e}")));
                    }
                }
            });
        })
        .map_err(|e| e.to_string())?;
    // The desktop may show a confirmation dialog; give the user time.
    rx.recv_timeout(Duration::from_secs(120))
        .map_err(|_| "the shortcuts dialog did not answer in time".to_string())?
}
