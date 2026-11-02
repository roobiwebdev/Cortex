// Registering the command list makes every IPC command opt-in: a window can
// only call the ones its capability grants (`allow-<command>`). Keep this list
// in sync with `collect_commands!` in `src/ipc.rs`.
const COMMANDS: &[&str] = &[
    "engine_state",
    "engine_start",
    "engine_stop",
    "engine_pause",
    "engine_resume",
    "update_settings",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
