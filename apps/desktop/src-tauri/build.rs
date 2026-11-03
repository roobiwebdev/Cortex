// Registering the command list makes every IPC command opt-in: a window can
// only call the ones its capability grants (`allow-<command>`). Keep this list
// in sync with `collect_commands!` in `src/ipc.rs`.
const COMMANDS: &[&str] = &[
    "engine_state",
    "start",
    "cancel_countdown",
    "stop",
    "pause",
    "resume",
    "discard",
    "save_replay",
    "go_live",
    "stop_live",
    "update_settings",
    "get_settings",
    "devices",
    "app_icon",
    "request_permission",
    "check_permissions",
    "relaunch",
    "open_system_settings",
    "reveal_recording",
    "open_recordings_folder",
    "set_stream_key",
    "remove_stream_key",
    "import_rec_settings",
    "set_preview_rect",
    "set_preview_enabled",
    "fit_floating",
    "open_floating",
    "hide_floating",
    "window_ready",
    "push_toast",
    "open_window",
    "list_recordings",
    "rename_recording",
    "recording_thumbnails",
    "recording_waveform",
    "trim_recording",
    "cancel_trim",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
