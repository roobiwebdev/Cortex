#!/usr/bin/env bash
# Copies the installers Tauri built into OUT_DIR under the stable release names, the
# contract with the site's download links and the updater feed (the full list is in
# scripts/release/updater-manifest.mjs, GROUPS).
#
#   scripts/release/stage.sh windows-x64|linux-x64|linux-arm64 BUNDLE_DIR OUT_DIR [VERSION]
#
# BUNDLE_DIR is Tauri's bundle folder (target/release/bundle). With VERSION, every file must
# carry it in its Tauri name, so a stale bundle from an older build is never staged.
# Used by .github/workflows/desktop-{windows,linux}.yml and scripts/release/build-linux-docker.sh.
set -euo pipefail

die() {
  echo "stage: $*" >&2
  exit 1
}

group="${1:-}"
bundle_dir="${2:-}"
out="${3:-}"
version="${4:-*}"
[[ -n "$group" && -d "$bundle_dir" && -n "$out" ]] || die "usage: $0 windows-x64|linux-x64|linux-arm64 BUNDLE_DIR OUT_DIR [VERSION]"
mkdir -p "$out"

# Exactly one file must match each pattern.
take() {
  local pattern="$1" name="$2"
  local matches=()
  shopt -s nullglob
  # shellcheck disable=SC2206 # the pattern is a glob on purpose
  matches=($bundle_dir/$pattern)
  shopt -u nullglob
  [[ ${#matches[@]} == 1 ]] || die "expected one $bundle_dir/$pattern, found ${#matches[@]}"
  cp "${matches[0]}" "$out/$name"
  echo "  $(basename "${matches[0]}") -> $name"
}

# Tauri's names: Cortex_<v>_x64-setup.exe, Cortex_<v>_x64_en-US.msi, Cortex_<v>_amd64.AppImage
# (aarch64 on arm), Cortex_<v>_amd64.deb (arm64), Cortex-<v>-1.x86_64.rpm (aarch64). Each
# bundle folder holds one architecture, so the patterns only pin the version.
case "$group" in
  windows-x64)
    take "nsis/Cortex_${version}_*-setup.exe" Cortex-Windows-x64-Setup.exe
    take "msi/Cortex_${version}_*.msi" Cortex-Windows-x64.msi
    ;;
  linux-x64 | linux-arm64)
    if [[ $group == linux-x64 ]]; then arch=x86_64 deb_arch=amd64; else arch=aarch64 deb_arch=arm64; fi
    take "appimage/Cortex_${version}_*.AppImage" "Cortex-Linux-$arch.AppImage"
    take "deb/Cortex_${version}_*.deb" "cortex-linux-$deb_arch.deb"
    take "rpm/Cortex-${version}-*.rpm" "cortex-linux-$arch.rpm"
    ;;
  *) die "unknown group $group" ;;
esac
