# Testing the Windows engine on a Windows PC

The Windows engine (`crates/rec-engine-windows`) was written on a Mac. There it is only type-checked for `x86_64-pc-windows-msvc` and its platform-neutral logic is unit-tested. Nothing in it has linked or run on Windows yet. This page is the script for the first runs on a real PC: set-up, the commands, a checklist of what to try, and how to collect logs when something fails.

Work through it in order. Each step says what you should see. Note the result of every step (pass, fail, or what happened instead), together with the Windows version and GPU from step 1.

## 1. The PC

Windows 11 23H2 or later (x64) is the main target. Windows 10 22H2 also works, but shows a yellow border around captures and can't hide Cortex's own windows on builds older than 19041.

In PowerShell:

```powershell
winver                                   # note the version and build (e.g. 23H2, 22631)
Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion
```

Write down the build number and the GPU. HEVC and the hardware encoder both depend on the GPU.

## 2. Prerequisites (once)

1. **Visual Studio 2022 Build Tools** with the "Desktop development with C++" workload. It brings MSVC, the Windows SDK and `rc.exe`. Installer: <https://visualstudio.microsoft.com/visual-cpp-build-tools/>.
2. **Rust** through `rustup-init.exe` (<https://rustup.rs>). Accept the default `x86_64-pc-windows-msvc` host. The repo's `rust-toolchain.toml` pins the version; rustup installs it on the first `cargo` call.
3. **Node 22 or later** (<https://nodejs.org>), then pnpm: `corepack enable` in an administrator PowerShell, or `npm install -g pnpm`.
4. **Git** (<https://git-scm.com>).
5. **WebView2** is part of Windows 11. On Windows 10, install the Evergreen runtime if `tauri dev` says it is missing.
6. Optional, for checking files: **ffmpeg/ffprobe** (`winget install Gyan.FFmpeg`), then open a new terminal.

Check the tools:

```powershell
cargo --version; rustc --version; node --version; pnpm --version; git --version
```

## 3. Get the code

```powershell
git clone <repo-url> Cortex
cd Cortex
git checkout agent/engine-windows-4f8d3
pnpm install
```

## 4. Build and unit tests

```powershell
cargo test -p rec-core -p rec-engine-windows
cargo clippy --no-deps -p rec-core -p rec-engine-windows -p cortex-desktop --all-targets -- -D warnings
cargo test -p cortex-desktop
pnpm --filter desktop check-types
pnpm --filter desktop test
```

Expected: every test passes and clippy prints no warnings. This is the first time the Windows code links, so a link error (an unresolved `__imp_...` symbol) here means a missing `windows` crate feature. Copy the full error.

## 5. Self-test recording with the test pattern (no screen, camera or mic needed)

```powershell
$env:CORTEX_DEBUG_DATA_DIR = "$env:TEMP\cortex-test"
$env:CORTEX_DEBUG_TEST_PATTERN = "1"
$env:CORTEX_DEBUG_RECORD_SECS = "5"
pnpm --filter desktop tauri dev 2>&1 | Tee-Object "$env:TEMP\cortex-pattern.log"
```

The app opens, records 5 seconds of a moving bar with a white flash every second, prints the file path and quits. Then:

```powershell
Remove-Item Env:CORTEX_DEBUG_TEST_PATTERN, Env:CORTEX_DEBUG_RECORD_SECS
$f = Get-ChildItem "$env:TEMP\cortex-test" -Recurse -Filter *.mp4 | Sort-Object LastWriteTime | Select-Object -Last 1
ffprobe -v error -show_entries stream=index,codec_type,codec_name,width,height,r_frame_rate,nb_frames,channels -of compact $f.FullName
```

Expected: one `h264` (or `hevc`) video stream at 1920x1080 and 60/1 with about 300 frames, plus the audio tracks your settings ask for (the test pattern's beep is the "system" track). Play the file in the Films & TV app or VLC: the flash and the beep should line up.

If no file appears, keep `cortex-pattern.log` (see section 8).

## 6. The real app

```powershell
Remove-Item Env:CORTEX_DEBUG_DATA_DIR -ErrorAction SilentlyContinue
pnpm --filter desktop tauri dev 2>&1 | Tee-Object "$env:TEMP\cortex-dev.log"
```

Recordings go to `%USERPROFILE%\Videos\Cortex`, unless your settings name another folder.

## 7. Checklist

Do each item. After each recording, open the file and check it plays. `ffprobe` (section 5) shows the tracks.

**Start-up and UI**

- [ ] The main window opens. The preview area shows "No live preview on this computer yet" rather than a see-through hole or a fake desktop.
- [ ] Settings › Recording: the Container control offers MP4 only, and HEVC is listed only if your GPU has an HEVC encoder.
- [ ] The display, window and app lists show your monitors and open windows. Cortex's own windows are not in the list.

**Capture targets**

- [ ] **Display**: record display 1 for about 10 s while you move a window around. The file shows the whole display at the expected size (Settings › Recording › Resolution; Native = the display's pixels).
- [ ] **Second display** (if you have one): pick it and record. The file shows that display.
- [ ] **Area**: pick an area with the region picker and record. The file shows only that area. Try it on a display with 125% or 150% scaling too: the area should match what you picked.
- [ ] **Window**: pick a window, record, and resize it during the recording. The file shows only that window; after the resize the frame keeps its size, with black where the window no longer reaches.
- [ ] **App**: pick an app. Expect a notice saying that only its main window is recorded, then a recording of that window.
- [ ] **Cursor**: with Settings › Recording › cursor off, the pointer is missing from the file; with it on, the pointer is there.
- [ ] **Capture border**: on Windows 11 there should be no yellow border around the captured display or window. On Windows 10 it shows; that is expected.

**Cortex stays out of the recording**

- [ ] Record a display while the main window, the HUD and a toast are visible on it. None of them may appear in the file.
- [ ] Open Settings during a recording: it is not in the file either.
- [ ] Take a screenshot with Win+Shift+S while the HUD shows: the HUD is missing from the screenshot (the same exclusion applies to every capture tool).

**Audio**

- [ ] **System audio**: play a YouTube video and record. The file has system audio.
- [ ] **Quiet system audio**: record 20 s with nothing playing, then play something. The audio stays in sync with the video at the end.
- [ ] **Microphone**: record while talking, with the mic enabled. The mic track has your voice. Try a USB or Bluetooth headset as well (different sample rate).
- [ ] **Tracks**: with system audio and mic both on, Settings › Audio › tracks "Both" gives 3 audio tracks (mix, system, mic), "Mix" gives 1, "Stems" gives 2. Check with ffprobe.
- [ ] **Levels**: the meters move while recording. Muting a source keeps its meter moving but silences it in the file.
- [ ] **Sync**: clap in front of the camera, or click a key with the mic on, while recording a stopwatch website. Clap and video should match within about one frame.

**Camera**

- [ ] **Screen + camera**: the camera overlay appears in the file at the corner set in Settings, in the chosen shape (try circle, rounded, squircle), mirrored when Mirror is on, with the border when one is set.
- [ ] Move the overlay in the main window during a recording: the file follows.
- [ ] **Camera only** scene: the file shows the camera full frame.
- [ ] **Camera privacy off**: Settings › Privacy & security › Camera › "Let desktop apps access your camera" off. Cortex shows the camera as needing permission; "Allow" opens that Settings page. Turn it back on.

**Pause, stop, discard**

- [ ] Pause for 5 s and resume. The file has no gap and the audio is cut at the same moment as the video.
- [ ] Discard a recording: no file is left in the output folder.
- [ ] Quit Cortex from the tray during a recording: the file is written and plays.

**Hotkeys**

- [ ] With another app in front, **Ctrl+Shift+R** starts a recording (after the countdown) and stops it.
- [ ] During the countdown, **Esc** cancels it.
- [ ] **Ctrl+Shift+S** does nothing harmful (the replay buffer is not available on Windows yet).

**Files**

- [ ] "Show in Explorer" on a finished recording opens Explorer with the file selected.
- [ ] The recordings list shows each file's duration and size.
- [ ] Quick trim: expect a "not available on Windows yet" message.

**Stream keys** (no streaming yet; only storage)

- [ ] Settings › Live: save a key for YouTube. Control Panel › Credential Manager › Windows Credentials lists "Cortex stream key:rtmps://a.rtmps.youtube.com:443/live2". Remove it in Cortex; the entry disappears.

**Optional: crash-safe MP4**

- [ ] Set `$env:CORTEX_DEBUG_FRAGMENTED_MP4 = "1"`, start `tauri dev`, record 10 s and stop. Does the file play, with all tracks? Then record again and end the process from Task Manager mid-recording. Does the partial file play up to the kill?

## 8. Collecting logs

- `tauri dev` prints the app's messages, including every `cortex:` and `rec-engine-windows:` line. The `Tee-Object` commands above save them to `%TEMP%\cortex-dev.log` or `%TEMP%\cortex-pattern.log`.
- For a crash, run with `$env:RUST_BACKTRACE = "1"` and keep the whole log.
- Media Foundation details: Event Viewer › Windows Logs › Application, filtered on the time of the failure.
- When a recording looks wrong, keep the file and add the `ffprobe` output from section 5.
- Send the logs, the Windows build, the GPU and driver version (section 1), and the checklist results.

## 9. Installers (optional)

```powershell
pnpm --filter desktop tauri build --bundles nsis,msi
```

The installers land in `target\release\bundle\nsis\` and `target\release\bundle\msi\`. The NSIS installer installs for the current user without administrator rights. They are unsigned, so SmartScreen warns on first run.
