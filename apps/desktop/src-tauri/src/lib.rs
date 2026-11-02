//! Cortex desktop shell: windows, menu bar icon, and the typed IPC layer over
//! the recorder engine (a stub until `rec-engine-macos` lands).

mod engine;
mod ipc;
mod windows;

use std::{thread, time::Duration};

use tauri::{Manager, RunEvent};

use crate::engine::StubEngine;

pub fn run() {
    let ipc = ipc::builder();

    let app = tauri::Builder::default()
        .manage(StubEngine::default())
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            windows::setup(app)?;

            // Stub clock: advances `elapsedSecs` while "recording" so the HUD moves.
            let handle = app.handle().clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if let Some(state) = handle.state::<StubEngine>().tick() {
                        ipc::broadcast(&handle, state);
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Cortex app");

    app.run(|app, event| {
        // Clicking the Dock icon brings the main window back after it was closed.
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            windows::show_main(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
