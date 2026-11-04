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
    "open_link",
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
    "platform",
    "set_hotkey_practice",
    "list_recordings",
    "rename_recording",
    "recording_thumbnails",
    "recording_waveform",
    "trim_recording",
    "cancel_trim",
    "update_status",
    "check_for_updates",
    "install_update",
];

fn main() {
    let mut attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    // Windows: tauri-build embeds the Common Controls v6 manifest in the app binary only,
    // so the unit-test binaries die at load with STATUS_ENTRYPOINT_NOT_FOUND. Embed the same
    // manifest through the linker instead, which covers the app and its tests alike.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
