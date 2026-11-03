# Testing the Linux engine on a real PC

The Linux engine (`crates/rec-engine-linux`) has been built and run in an Ubuntu 24.04 arm64 container (see "Linux engine" in [Platforms](platforms.md)). A container has no Wayland session, no xdg-desktop-portal, no GPU, no camera and no PipeWire session manager, so these parts have **never run**: the ScreenCast portal and its restore token, `pipewiresrc` on a real screen, VA-API and NVENC encoders, V4L2 cameras, the GlobalShortcuts portal, PipeWire device listing, the file manager over D-Bus, and the floating windows under a real compositor. This script covers each of them. It works for the user or for an agent with a shell on that PC.

Write down the answers to step 0 with every result: most Linux differences come from the session type, the desktop and the GPU.

## 0. What PC is this

```sh
echo "$XDG_SESSION_TYPE $XDG_CURRENT_DESKTOP"; cat /etc/os-release | head -2
uname -m; lspci | grep -iE "vga|3d"
gst-inspect-1.0 --version | head -1; pipewire --version | tail -1
systemctl --user status xdg-desktop-portal --no-pager | head -3
ls /usr/share/xdg-desktop-portal/portals/
```

Expected on Ubuntu 24.04 GNOME: `wayland ubuntu:GNOME`, GStreamer 1.24, PipeWire 1.0, portals `gnome.portal` and `gtk.portal`. Fedora 41+: GStreamer 1.24+, PipeWire 1.2+.

## 1. Prerequisites

**Ubuntu 24.04 / Debian 13:**

```sh
sudo apt install build-essential curl wget file git pkg-config clang \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
  libpipewire-0.3-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libdbus-1-dev libv4l-dev \
  gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
  gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-pipewire gstreamer1.0-pulseaudio \
  gstreamer1.0-vaapi vainfo ffmpeg libsecret-tools
# GPU encoders: Intel `intel-media-va-driver-non-free`, AMD `mesa-va-drivers`; NVIDIA needs the
# proprietary driver (nvh264enc comes with gstreamer1.0-plugins-bad when CUDA is present).
```

**Fedora 41+:**

```sh
sudo dnf install @development-tools clang pkgconf-pkg-config \
  webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel libxdo-devel \
  pipewire-devel gstreamer1-devel gstreamer1-plugins-base-devel dbus-devel libv4l-devel \
  gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free pipewire-gstreamer \
  gstreamer1-plugin-openh264 libsecret
# H.264 in software and AAC need more than Fedora ships. Enable RPM Fusion, then:
sudo dnf install gstreamer1-plugins-ugly gstreamer1-plugin-libav gstreamer1-plugins-bad-freeworld \
  ffmpeg libva-utils
# GPU: Intel `intel-media-driver`; AMD `mesa-va-drivers-freeworld` (RPM Fusion) for H.264/HEVC.
```

Then Rust and Node:

```sh
curl https://sh.rustup.rs -sSf | sh          # rust-toolchain.toml picks 1.99.1
corepack enable                              # pnpm; Node 22+ from your distro or nodesource
git clone <repo> cortex && cd cortex && git checkout <branch>
pnpm install
```

## 2. Build, lint, unit and engine tests

```sh
cargo fmt --check -p rec-core -p rec-engine-linux -p cortex-desktop
pnpm --filter desktop build                  # the shell embeds these web assets
cargo clippy --no-deps -p rec-core -p rec-engine-linux -p cortex-desktop --all-targets -- -D warnings
cargo test -p rec-core -p rec-engine-linux -p cortex-desktop
CORTEX_KEEP_TEST_FILES=1 cargo test -p rec-engine-linux --test engine -- --nocapture --test-threads=1
```

The engine tests record the test pattern through the real engine (pause, discard, countdown, HEVC if x265 is installed) and print the kept files. Check one:

```sh
ffprobe -v error -count_frames -show_entries stream=index,codec_name,width,height,nb_read_frames,channels:format=duration -of compact /tmp/cortex-linux-full-*/out/*.mp4
```

Expected: two H.264 video streams (1280x720 screen, 1920x1080 camera), three AAC stereo streams, about 4.1 s.

## 3. What the engine finds

```sh
cargo run -p rec-engine-linux --example probe
```

Note the H.264, HEVC and AAC encoders it lists (the first one listed is the one it uses), the cameras and mics, and `GlobalShortcuts portal: true/false`. On a GPU PC the H.264 list should start with `vah264enc` (Intel/AMD) or `nvh264enc` (NVIDIA). If it shows only `x264enc`, run `vainfo` and `gst-inspect-1.0 va` and note the output.

## 4. Portal capture without the app

```sh
cargo run -p rec-engine-linux --example probe -- portal 5
ffprobe -v error -count_frames -show_entries stream=codec_name,width,height,nb_read_frames:format=duration -of compact cortex-probe-portal.mov
```

The system's "Share your screen" dialog must appear; pick a monitor. Expected: a 5 s file, H.264 at your screen's shape (fitted in 1080p), about 150 frames at 30 fps, with the pointer drawn in, and 3 audio tracks (system audio, default mic, mix). Play it: `mpv cortex-probe-portal.mov` (or `ffplay`).

## 5. The app

```sh
CORTEX_DEBUG_PIPELINE=1 pnpm --filter desktop tauri dev 2>&1 | tee cortex-run.log
```

or install the package you built (`pnpm --filter desktop tauri build --bundles deb` then `sudo apt install ./target/release/bundle/deb/Cortex_*_amd64.deb`, or the `.rpm` with `sudo dnf install`) and run `cortex-desktop` from a terminal so the log is visible.

A quick end-to-end check that needs no portal (records the test pattern for 4 s and quits):

```sh
CORTEX_DEBUG_DATA_DIR=/tmp/cortex-test CORTEX_DEBUG_TEST_PATTERN=1 CORTEX_DEBUG_RECORD_SECS=4 cortex-desktop
```

It prints `cortex: recorded <path>` (in `~/Videos/Cortex`).

## 6. Checklist

Record each result with the session type and desktop from step 0. `ffprobe` each file as in step 2.

| #   | Check                        | How                                                                                                                          | Expected                                                                                                                                                                            |
| --- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Portal picker                | Main window, Display target, Record.                                                                                         | The system dialog opens before the countdown. Cancelling it shows "Screen sharing was cancelled" and Cortex stays idle.                                                             |
| 2   | Restore token                | Record again with the same target.                                                                                           | No dialog, or the dialog with your last choice selected (GNOME may still show it once more). `~/.local/state/cortex/screencast.json` holds a `monitorToken`.                        |
| 3   | Each target                  | Display; Window (`CORTEX_DEBUG_DATA_DIR=/tmp/c CORTEX_DEBUG_TARGET=window:1`, the window list is empty on Wayland); an area. | Display and window: the portal offers screens or windows respectively. Area: the region picker opens; on Wayland it may not cover the screen exactly (clients can't place windows). |
| 4   | Three audio tracks           | Play something, talk, record 10 s with the default Audio tracks setting.                                                     | `ffprobe` shows 3 AAC streams. `mpv --aid=1` mix, `--aid=2` system only, `--aid=3` mic only. The meters move while recording.                                                       |
| 5   | Camera                       | Scene "Screen + camera", Circle, then Rounded rect, then a free position.                                                    | The camera sits where the stage shows it, masked to the shape, mirrored if Mirror is on. A second video stream (the raw camera, 1920x1080 or 1280x720) when Camera track is on.     |
| 6   | Camera only                  | Scene "Camera".                                                                                                              | The camera fills the frame; no portal dialog.                                                                                                                                       |
| 7   | Pause                        | Record 5 s, pause 5 s, record 5 s, stop.                                                                                     | About 10 s file; no frozen picture or silence at the cut; the timer stops while paused.                                                                                             |
| 8   | Discard                      | Record, then Discard.                                                                                                        | No file left in `~/Videos/Cortex`.                                                                                                                                                  |
| 9   | Hotkeys                      | With another app in front press Ctrl+Shift+R, again to stop.                                                                 | KDE Plasma 5.27+ and GNOME 48+ ask once to allow the shortcuts, then they work. Elsewhere the log says `global shortcuts unavailable` and only in-app controls work.                |
| 10  | Our windows in the recording | Record the screen that shows the HUD and the countdown.                                                                      | **Known Wayland limit:** they appear in the recording. Note whether they stay on top and where they open (compositors ignore positions).                                            |
| 11  | Encoder                      | Search `cortex-run.log` for the `pipeline:` block.                                                                           | The encoder line (`vah264enc`, `nvh264enc`, `x264enc`...). Any `encoder ... failed` line means it fell back; keep the line.                                                         |
| 12  | HEVC                         | Settings › Recording › Codec HEVC, 1440p or 4K.                                                                              | `ffprobe` says `hevc`, or the toast says no HEVC encoder is installed and the file is H.264.                                                                                        |
| 13  | Crash safety                 | Start recording, then `pkill -9 cortex-desktop` after 10 s.                                                                  | The file plays up to about the last second.                                                                                                                                         |
| 14  | Library                      | Open the recordings list.                                                                                                    | New files listed with thumbnails and a waveform. Quick trim says it is not available on Linux yet. Reveal opens the file manager with the file selected.                            |
| 15  | Stream keys                  | Settings › Live: save a key for YouTube, then `secret-tool search service dev.roobi.cortex.stream-key`.                      | The key is in GNOME Keyring / KWallet, and the UI says "key stored". Without a keyring the UI shows the error.                                                                      |
| 16  | Sleep and devices            | Plug a USB mic or camera in while Cortex is open.                                                                            | It appears in the lists within about 2 s.                                                                                                                                           |
| 17  | X11 session                  | Log in with "GNOME on Xorg" (or Plasma X11) and repeat 1, 4, 9, 10.                                                          | The portal still picks the screen; the HUD should stay on top and where placed.                                                                                                     |

## 7. Logs to collect

```sh
cortex-desktop 2>&1 | tee cortex-run.log                        # or the tauri dev log; add CORTEX_DEBUG_PIPELINE=1
GST_DEBUG=3 CORTEX_DEBUG_PIPELINE=1 cortex-desktop 2> cortex-gst.log   # when a recording fails
journalctl --user -b --no-pager | grep -iE "portal|pipewire|wireplumber" | tail -200 > portal.log
gst-inspect-1.0 | grep -iE "264|265|aac|pipewire|pulse|v4l2" > gst-elements.txt
vainfo > vainfo.txt 2>&1; nvidia-smi > nvidia.txt 2>&1
pw-dump > pw-dump.json
cat ~/.local/state/cortex/screencast.json
for f in ~/Videos/Cortex/*; do echo "$f"; ffprobe -v error -count_frames -show_entries stream=index,codec_name,width,height,nb_read_frames,channels:format=duration -of compact "$f"; done > ffprobe.txt
```
