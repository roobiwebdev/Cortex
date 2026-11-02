//! Cortex desktop shell: windows, menu bar icon, and the typed IPC layer over
//! the macOS recorder engine (`rec-engine-macos`, driven through `shell::Shell`).

mod ipc;
pub mod shell;
mod windows;

use std::sync::Arc;

use rec_engine::engine::EngineOptions;
use tauri::{Manager, RunEvent};

use crate::shell::{Outbound, Shell};

pub fn run() {
    let ipc = ipc::builder();

    let app = tauri::Builder::default()
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            // ~/Library/Application Support/dev.roobi.cortex/settings.json
            let settings_path = app.path().app_data_dir()?.join("settings.json");
            let opts = EngineOptions {
                home: app.path().home_dir()?,
                ..Default::default()
            };
            let emit = ipc::emitter(app.handle().clone());
            let shell = Shell::open(opts, settings_path, emit.clone());
            if let Some(message) = &shell.load_error {
                eprintln!("cortex: {message}");
                emit(Outbound::Error {
                    message: message.clone(),
                    recoverable: true,
                });
            }
            // On the main thread, as the device notifications require.
            shell.engine().watch_devices();
            app.manage(Arc::new(shell));
            windows::setup(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Cortex app");

    app.run(|app, event| match event {
        // Quit finalizes a running recording and writes pending settings first.
        RunEvent::Exit => {
            if let Some(shell) = app.try_state::<Arc<Shell>>() {
                shell.shutdown();
            }
        }
        // Clicking the Dock icon brings the main window back after it was closed.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => windows::show_main(app),
        _ => {}
    });
}
