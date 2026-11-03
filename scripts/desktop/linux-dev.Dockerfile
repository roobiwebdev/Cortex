# Linux build and test image for the Cortex desktop app (Tauri shell + rec-engine-linux).
# Ubuntu 24.04, the CI runner's distro. Used on a Mac through Docker Desktop (arm64) and
# usable as-is on x86_64. See docs/engineering/platforms.md ("Linux in Docker").
#
#   docker build -t cortex-linux-dev -f scripts/desktop/linux-dev.Dockerfile scripts/desktop
#   docker run --rm -it -v "$PWD":/src -v cortex-linux-target:/target \
#     -v cortex-linux-cargo:/usr/local/cargo/registry cortex-linux-dev bash
#
# Node is not installed: the web assets are platform-neutral, so `apps/desktop/dist` is
# built on the host and `cargo tauri build` runs here with `beforeBuildCommand` cleared.
FROM ubuntu:24.04

ARG RUST_VERSION=1.99.1
ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    CARGO_TARGET_DIR=/target \
    APPIMAGE_EXTRACT_AND_RUN=1

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl wget file git pkg-config build-essential clang \
      # Tauri 2 on Linux
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
      libssl-dev libxdo-dev \
      # Engine: PipeWire, GStreamer, D-Bus, V4L2
      libpipewire-0.3-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
      gstreamer1.0-pipewire gstreamer1.0-pulseaudio \
      libdbus-1-dev libv4l-dev \
      # Runtime tests: an audio daemon, a Secret Service, ffprobe
      pipewire pipewire-pulse wireplumber pulseaudio-utils dbus dbus-x11 \
      gnome-keyring libsecret-tools ffmpeg xvfb xauth \
      # AppImage / deb / rpm bundling
      rpm squashfs-tools desktop-file-utils xdg-utils \
    && rm -rf /var/lib/apt/lists/*

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal \
      --default-toolchain ${RUST_VERSION} -c clippy,rustfmt \
    # rust-toolchain.toml lists the Apple targets; install them once so runs stay offline.
    && rustup target add --toolchain ${RUST_VERSION} aarch64-apple-darwin x86_64-apple-darwin \
    && cargo install tauri-cli --version "^2" --locked \
    && rm -rf /usr/local/cargo/registry /usr/local/cargo/git

WORKDIR /src
