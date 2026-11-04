#!/bin/sh
# Cortex installer for macOS and Linux.
#
#   curl -fsSL https://cortex.roobi.dev/install.sh | sh
#
# macOS: copies Cortex.app from the DMG into /Applications (or ~/Applications when
# /Applications is not writable). Linux: installs the AppImage as ~/.local/bin/cortex
# with a menu entry and icon.
#
# A file that curl downloads has no com.apple.quarantine attribute, so Gatekeeper
# does not block the first launch the way it does for a browser download.
#
# Environment overrides:
#   CORTEX_VERSION  release tag to install, e.g. v0.1.0 (default: the latest release)
#   INSTALL_DIR     where to install (macOS: /Applications, Linux: ~/.local/bin)
#   NO_OPEN         set to 1 to skip launching Cortex after installing
#   CORTEX_API_URL  test only: release JSON URL to read instead of the GitHub API
#
# Uses only POSIX sh, curl, sed, grep, mktemp and uname, plus the OS tools
# (hdiutil, ditto, open on macOS). See docs/engineering/releasing.md.

set -eu

REPO="roobiwebdev/cortex-releases"
APP_NAME="Cortex"
PROCESS_NAME="cortex-desktop"

say() { printf '%s\n' "$*"; }
step() { printf '==> %s\n' "$*"; }
warn() { printf 'Warning: %s\n' "$*" >&2; }
fail() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

want_open() {
  case "${NO_OPEN:-}" in
    "" | 0 | false | no) return 0 ;;
    *) return 1 ;;
  esac
}

command -v curl >/dev/null 2>&1 || fail "curl is required."

TMP_DIR=""
MOUNT_POINT=""
cleanup() {
  if [ -n "$MOUNT_POINT" ] && [ -d "$MOUNT_POINT" ]; then
    hdiutil detach "$MOUNT_POINT" -quiet >/dev/null 2>&1 || hdiutil detach "$MOUNT_POINT" -force -quiet >/dev/null 2>&1 || true
  fi
  if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t cortex-install)

# Normalise CORTEX_VERSION to a tag (v1.2.3).
TAG=""
if [ -n "${CORTEX_VERSION:-}" ]; then
  case "$CORTEX_VERSION" in
    v*) TAG="$CORTEX_VERSION" ;;
    *) TAG="v$CORTEX_VERSION" ;;
  esac
fi

if [ -n "${CORTEX_API_URL:-}" ]; then
  API_URL="$CORTEX_API_URL"
elif [ -n "$TAG" ]; then
  API_URL="https://api.github.com/repos/$REPO/releases/tags/$TAG"
else
  API_URL="https://api.github.com/repos/$REPO/releases/latest"
fi

RELEASE_JSON="$TMP_DIR/release.json"
API_OK=0

fetch_release() {
  step "Looking up ${TAG:-the latest} Cortex release"
  if curl -fsSL --retry 3 -H "Accept: application/vnd.github+json" -o "$RELEASE_JSON" "$API_URL" 2>/dev/null; then
    API_OK=1
    found_tag=$(grep -o '"tag_name": *"[^"]*"' "$RELEASE_JSON" | sed 's/.*"\([^"]*\)"$/\1/' | sed -n 1p)
    if [ -n "$found_tag" ]; then say "    Release: $found_tag"; fi
  elif [ -n "$TAG" ] && [ -z "${CORTEX_API_URL:-}" ]; then
    # Tell a missing tag apart from GitHub being unreachable or rate limited.
    code=$(curl -sS -o /dev/null -w '%{http_code}' "$API_URL" 2>/dev/null || true)
    [ "$code" = "404" ] && fail "Cortex release $TAG does not exist. See https://github.com/$REPO/releases"
    warn "Could not read the release list from GitHub (HTTP ${code:-error}). Trying the direct download link."
  else
    warn "Could not read the release list from GitHub (it may be rate limiting this network). Trying the direct download link."
  fi
}

# asset_url SUFFIX FALLBACK_NAME: the download URL of the release asset whose name ends in SUFFIX.
# When the API could not be read, falls back to the stable release download link.
asset_url() {
  suffix=$1
  fallback=$2
  if [ "$API_OK" = 1 ]; then
    grep -o '"browser_download_url": *"[^"]*"' "$RELEASE_JSON" |
      sed 's/.*"\([^"]*\)"$/\1/' |
      grep -F -- "$suffix" |
      while IFS= read -r url; do
        case "$url" in
          *"$suffix") printf '%s\n' "$url" ;;
        esac
      done |
      sed -n 1p
  elif [ -n "$TAG" ]; then
    printf 'https://github.com/%s/releases/download/%s/%s\n' "$REPO" "$TAG" "$fallback"
  else
    printf 'https://github.com/%s/releases/latest/download/%s\n' "$REPO" "$fallback"
  fi
}

# download URL DEST: returns non-zero (without exiting) when the file is not there.
download() {
  step "Downloading $(basename "$1")"
  curl -fL --retry 3 --progress-bar -o "$2" "$1"
}

# ---------------------------------------------------------------- macOS

wait_for_exit() {
  i=0
  while pgrep -x "$PROCESS_NAME" >/dev/null 2>&1; do
    [ "$i" -ge "$1" ] && return 1
    sleep 1
    i=$((i + 1))
  done
  return 0
}

quit_running_mac() {
  pgrep -x "$PROCESS_NAME" >/dev/null 2>&1 || return 0
  step "Quitting the running Cortex"
  osascript -e "quit app \"$APP_NAME\"" >/dev/null 2>&1 &
  wait_for_exit 10 && return 0
  pkill -TERM -x "$PROCESS_NAME" >/dev/null 2>&1 || true
  wait_for_exit 5 && return 0
  pkill -KILL -x "$PROCESS_NAME" >/dev/null 2>&1 || true
  wait_for_exit 3 || fail "Cortex is still running. Quit it and run the installer again."
}

install_macos() {
  major=$(sw_vers -productVersion 2>/dev/null | sed 's/\..*//')
  case "$major" in
    '' | *[!0-9]*) ;;
    *) [ "$major" -ge 15 ] || fail "Cortex needs macOS 15 (Sequoia) or later. This Mac runs macOS $(sw_vers -productVersion)." ;;
  esac

  if [ -n "${INSTALL_DIR:-}" ]; then
    dest_dir=$INSTALL_DIR
    mkdir -p "$dest_dir" || fail "Cannot create $dest_dir."
    [ -w "$dest_dir" ] || fail "$dest_dir is not writable."
  elif [ -w /Applications ]; then
    dest_dir=/Applications
  else
    dest_dir="$HOME/Applications"
    mkdir -p "$dest_dir" || fail "Cannot create $dest_dir."
    say "    /Applications is not writable by this account, so Cortex goes into $dest_dir."
  fi

  fetch_release
  url=$(asset_url "-macOS.dmg" "Cortex-macOS.dmg")
  [ -n "$url" ] || fail "This release has no macOS installer. See https://github.com/$REPO/releases"
  dmg="$TMP_DIR/Cortex.dmg"
  download "$url" "$dmg" || fail "Download failed: $url"

  step "Opening the disk image"
  MOUNT_POINT="$TMP_DIR/mnt"
  mkdir -p "$MOUNT_POINT"
  hdiutil attach "$dmg" -nobrowse -readonly -noautoopen -mountpoint "$MOUNT_POINT" -quiet ||
    fail "Could not open the disk image."
  src="$MOUNT_POINT/$APP_NAME.app"
  [ -d "$src" ] || fail "The disk image has no $APP_NAME.app."

  quit_running_mac

  dest="$dest_dir/$APP_NAME.app"
  staging="$dest_dir/.$APP_NAME.app.installing"
  step "Installing $APP_NAME.app into $dest_dir"
  rm -rf "$staging"
  ditto "$src" "$staging" || {
    rm -rf "$staging"
    fail "Could not copy $APP_NAME.app into $dest_dir."
  }
  if [ -e "$dest" ]; then
    rm -rf "$dest" || {
      rm -rf "$staging"
      fail "Could not replace $dest. Delete it, or run with INSTALL_DIR=\$HOME/Applications."
    }
  fi
  mv "$staging" "$dest"

  hdiutil detach "$MOUNT_POINT" -quiet >/dev/null 2>&1 || true
  MOUNT_POINT=""

  # curl leaves no quarantine flag; clear it anyway in case something else added one.
  xattr -dr com.apple.quarantine "$dest" >/dev/null 2>&1 || true

  say ""
  say "Cortex is installed: $dest"
  if want_open; then
    step "Opening Cortex"
    open "$dest" || warn "Could not open Cortex. Open it from $dest_dir."
  fi
}

# ---------------------------------------------------------------- Linux

has_fuse2() {
  for ldc in ldconfig /sbin/ldconfig /usr/sbin/ldconfig; do
    if command -v "$ldc" >/dev/null 2>&1; then
      "$ldc" -p 2>/dev/null | grep -q 'libfuse\.so\.2' && return 0
      return 1
    fi
  done
  for d in /lib /usr/lib /lib64 /usr/lib64 /lib/x86_64-linux-gnu /usr/lib/x86_64-linux-gnu /lib/aarch64-linux-gnu /usr/lib/aarch64-linux-gnu; do
    [ -e "$d/libfuse.so.2" ] && return 0
  done
  return 1
}

# True on Arch and its derivatives (Omarchy, EndeavourOS, Manjaro, CachyOS...).
is_arch_family() {
  [ -r /etc/os-release ] || return 1
  osid=$(sed -n 's/^ID=//p' /etc/os-release | sed 's/"//g')
  oslike=$(sed -n 's/^ID_LIKE=//p' /etc/os-release | sed 's/"//g')
  case " $osid $oslike " in
    *" arch "*) return 0 ;;
  esac
  return 1
}

# Arch installs no GStreamer plugins or screen-sharing portal by default; Cortex needs both.
arch_hint() {
  is_arch_family || return 0
  say ""
  say "Arch-based system detected. If recording fails, install what Cortex uses:"
  say "    sudo pacman -S --needed fuse2 gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav gst-plugin-pipewire xdg-desktop-portal"
  say "  plus your desktop's portal backend (Hyprland: xdg-desktop-portal-hyprland,"
  say "  GNOME: xdg-desktop-portal-gnome, KDE: xdg-desktop-portal-kde)."
}

fuse_hint() {
  id=""
  version=""
  if [ -r /etc/os-release ]; then
    id=$(sed -n 's/^ID=//p' /etc/os-release | sed 's/"//g')
    version=$(sed -n 's/^VERSION_ID=//p' /etc/os-release | sed 's/"//g')
  fi
  warn "FUSE 2 (libfuse.so.2) is missing. AppImages need it to start. Install it with:"
  case "$id" in
    ubuntu | pop | linuxmint | elementary | zorin | debian)
      major=$(printf '%s' "$version" | sed 's/\..*//')
      if [ "$id" = ubuntu ] && [ -n "$major" ] && [ "$major" -ge 24 ] 2>/dev/null; then
        say "    sudo apt install libfuse2t64"
      elif [ "$id" = debian ] && [ -n "$major" ] && [ "$major" -ge 13 ] 2>/dev/null; then
        say "    sudo apt install libfuse2t64"
      else
        say "    sudo apt install libfuse2"
      fi
      ;;
    fedora | rhel | centos | rocky | almalinux) say "    sudo dnf install fuse-libs" ;;
    *)
      if is_arch_family; then
        say "    sudo pacman -S --needed fuse2"
        say "    Without it, start Cortex with: $1 --appimage-extract-and-run"
        return 0
      fi
      say "    Ubuntu 22.04 and older:  sudo apt install libfuse2"
      say "    Ubuntu 24.04 and newer:  sudo apt install libfuse2t64"
      say "    Fedora:                  sudo dnf install fuse-libs"
      ;;
  esac
  say "    Without it, start Cortex with: $1 --appimage-extract-and-run"
}

install_icon() {
  appimage=$1
  icon_dir="$HOME/.local/share/icons/hicolor/256x256/apps"
  extract_dir="$TMP_DIR/extract"
  mkdir -p "$extract_dir"
  # --appimage-extract runs the AppImage's own runtime, which needs no FUSE.
  if (cd "$extract_dir" && "$appimage" --appimage-extract >/dev/null 2>&1); then
    root="$extract_dir/squashfs-root"
    icon=""
    for candidate in \
      "$root"/usr/share/icons/hicolor/256x256@2/apps/*.png \
      "$root"/usr/share/icons/hicolor/256x256/apps/*.png \
      "$root"/usr/share/icons/hicolor/512x512/apps/*.png \
      "$root"/usr/share/icons/hicolor/128x128/apps/*.png \
      "$root"/*.png \
      "$root/.DirIcon"; do
      if [ -f "$candidate" ]; then
        icon=$candidate
        break
      fi
    done
    if [ -n "$icon" ]; then
      mkdir -p "$icon_dir"
      cp -L "$icon" "$icon_dir/cortex.png" && return 0
    fi
  fi
  warn "Could not take the icon out of the AppImage; the menu entry uses a generic icon."
  return 1
}

install_linux() {
  case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "Cortex for Linux is built for x86_64 and aarch64 (arm64). This machine is $(uname -m)." ;;
  esac
  asset="Cortex-Linux-$arch.AppImage"

  bin_dir=${INSTALL_DIR:-"$HOME/.local/bin"}
  mkdir -p "$bin_dir" || fail "Cannot create $bin_dir."
  [ -w "$bin_dir" ] || fail "$bin_dir is not writable."
  target="$bin_dir/cortex"

  fetch_release
  url=$(asset_url "-Linux-$arch.AppImage" "$asset")
  if [ -z "$url" ]; then
    fail "Linux build not published yet: this release has no $asset. Watch https://github.com/$REPO/releases"
  fi
  appimage="$TMP_DIR/$asset"
  if ! download "$url" "$appimage"; then
    if [ "$API_OK" = 1 ]; then
      fail "Download failed: $url"
    fi
    fail "Linux build not published yet, or GitHub is unreachable: could not download $asset."
  fi
  chmod +x "$appimage"

  if pgrep -x "$PROCESS_NAME" >/dev/null 2>&1; then
    step "Stopping the running Cortex"
    pkill -TERM -x "$PROCESS_NAME" >/dev/null 2>&1 || true
    i=0
    while pgrep -x "$PROCESS_NAME" >/dev/null 2>&1 && [ "$i" -lt 10 ]; do
      sleep 1
      i=$((i + 1))
    done
    pkill -KILL -x "$PROCESS_NAME" >/dev/null 2>&1 || true
  fi

  step "Installing Cortex as $target"
  # Copy next to the target and rename, so a running copy is never half written.
  cp "$appimage" "$target.installing"
  chmod 755 "$target.installing"
  mv -f "$target.installing" "$target"

  icon_name=application-x-executable
  install_icon "$target" && icon_name=cortex

  apps_dir="$HOME/.local/share/applications"
  mkdir -p "$apps_dir"
  cat >"$apps_dir/cortex.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Cortex
GenericName=Screen Recorder
Comment=Screen, camera and audio recorder
Exec="$target" %U
Icon=$icon_name
Terminal=false
Categories=AudioVideo;Video;Recorder;
StartupWMClass=cortex-desktop
EOF
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true

  say ""
  say "Cortex is installed: $target"
  say "Menu entry: $apps_dir/cortex.desktop"

  fuse_ok=1
  if ! has_fuse2; then
    fuse_ok=0
    fuse_hint "$target"
  fi
  arch_hint

  case ":${PATH}:" in
    *":$bin_dir:"*) ;;
    *) warn "$bin_dir is not on your PATH, so the 'cortex' command will not be found. Add it in your shell profile: export PATH=\"$bin_dir:\$PATH\"" ;;
  esac

  if want_open; then
    if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
      say "No graphical session found; start Cortex from your app menu."
    elif [ "$fuse_ok" = 1 ]; then
      step "Opening Cortex"
      nohup "$target" >/dev/null 2>&1 &
    else
      step "Opening Cortex (without FUSE)"
      nohup "$target" --appimage-extract-and-run >/dev/null 2>&1 &
    fi
  fi
}

case "$(uname -s)" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
  *) fail "This installer is for macOS and Linux. On Windows, run in PowerShell: irm https://cortex.roobi.dev/install.ps1 | iex" ;;
esac
