#!/usr/bin/env bash
# Builds a Cortex release for macOS into dist/desktop/<version>/. It never uploads anything.
#
#   scripts/desktop/release-macos.sh [--notes TEXT | --notes-file FILE] [--skip-clean] [--local-test-feed]
#
# Steps: clean release build, code signing (hardened runtime), DMG, updater bundle
# (.app.tar.gz + minisign .sig), latest.json update manifest, SHA-256 sums. With Apple
# notarization credentials in the environment it also notarizes and staples the app (via
# the Tauri bundler) and the DMG. See docs/engineering/desktop.md, "Releasing".
#
# Environment (all optional):
#   CORTEX_SIGNING_IDENTITY   code-signing identity (default "Cortex Dev Signing"; a
#                             "Developer ID Application: ..." identity once the Apple
#                             membership exists)
#   CORTEX_UPDATE_BASE_URL    where the release folders will be served from; latest.json
#                             points at <base>/<version>/<bundle>. TODO: the real public URL.
#   CORTEX_UPDATER_KEY        updater private key (default ~/.config/cortex/updater.key)
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD   its password (default: Keychain item
#                             "cortex-updater-key")
#   CORTEX_DMGBUILD_VENV      where dmgbuild is installed on first use (default
#                             ~/.cache/cortex/dmgbuild-venv)
#   Notarization, either:  APPLE_ID + APPLE_PASSWORD (app-specific) + APPLE_TEAM_ID
#                     or:  APPLE_API_KEY + APPLE_API_ISSUER + APPLE_API_KEY_PATH
#
# --local-test-feed builds a copy that accepts a plain-HTTP update feed (for the local
# updater test with CORTEX_DEBUG_UPDATE_URL). Never publish it; it goes to
# dist/desktop/<version>-localtest/.
set -euo pipefail

die() {
  echo "release: $*" >&2
  exit 1
}
step() { echo "==> $*"; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
[[ "$(uname -s)" == Darwin ]] || die "macOS only"
[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"

notes=""
clean=1
local_feed=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes) notes="${2:?--notes needs text}"; shift 2 ;;
    --notes-file) notes="$(cat "${2:?--notes-file needs a path}")"; shift 2 ;;
    --skip-clean) clean=0; shift ;;
    --local-test-feed) local_feed=1; shift ;;
    -h | --help) sed -n '2,27p' "$0"; exit 0 ;;
    *) die "unknown option $1 (see --help)" ;;
  esac
done

# --- Version: one value in three files (scripts/desktop/bump-version.sh sets them) ---
version="$(node -p 'require("./apps/desktop/src-tauri/tauri.conf.json").version')"
pkg_version="$(node -p 'require("./apps/desktop/package.json").version')"
cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' apps/desktop/src-tauri/Cargo.toml | head -1)"
[[ "$version" == "$pkg_version" && "$version" == "$cargo_version" ]] ||
  die "versions differ (tauri.conf.json $version, package.json $pkg_version, Cargo.toml $cargo_version); run scripts/desktop/bump-version.sh"
[[ -n "$notes" ]] || notes="Cortex $version"

case "$(uname -m)" in
  arm64) arch=aarch64 ;;
  x86_64) arch=x86_64 ;;
  *) die "unsupported architecture $(uname -m)" ;;
esac
platform="darwin-$arch"

# --- Signing: code signing identity and updater key ---
identity="${CORTEX_SIGNING_IDENTITY:-Cortex Dev Signing}"
# (Outputs are captured before grep -q: with pipefail an early grep exit fails the pipe.)
identities="$(security find-identity -p codesigning)"
grep -qF "\"$identity\"" <<<"$identities" ||
  die "no code-signing identity \"$identity\" in the keychain"
key_path="${CORTEX_UPDATER_KEY:-$HOME/.config/cortex/updater.key}"
[[ -r "$key_path" ]] || die "updater key not found at $key_path (see docs/engineering/desktop.md, Releasing)"
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ]]; then
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s cortex-updater-key -w 2>/dev/null)" ||
    die "no Keychain item cortex-updater-key; set TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_path")"
export TAURI_SIGNING_PRIVATE_KEY

# --- Notarization: only with a complete set of credentials ---
apple_vars=(APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH)
notarize=0
if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]]; then
  notarize=1
  notary_auth=(--apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID")
elif [[ -n "${APPLE_API_KEY:-}" && -n "${APPLE_API_ISSUER:-}" && -n "${APPLE_API_KEY_PATH:-}" ]]; then
  notarize=1
  notary_auth=(--key "$APPLE_API_KEY_PATH" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER")
else
  for v in "${apple_vars[@]}"; do
    [[ -z "${!v:-}" ]] || die "$v is set but the notarization credentials are incomplete (see --help)"
  done
fi
if [[ $notarize == 1 ]]; then
  [[ "$identity" == "Developer ID Application:"* ]] ||
    die "notarization needs a Developer ID Application identity; set CORTEX_SIGNING_IDENTITY"
  [[ $local_feed == 0 ]] || die "--local-test-feed builds are never notarized"
fi

# TODO(hosting): replace the placeholder with the public URL the release folders are served from.
base_url="${CORTEX_UPDATE_BASE_URL:-https://cortex.example/updates}"
base_url="${base_url%/}"

out="dist/desktop/$version"
build_config='{"bundle":{"createUpdaterArtifacts":true}}'
if [[ $local_feed == 1 ]]; then
  out="dist/desktop/$version-localtest"
  build_config='{"bundle":{"createUpdaterArtifacts":true},"plugins":{"updater":{"dangerousInsecureTransportProtocol":true}}}'
  echo "release: --local-test-feed: this build accepts plain-HTTP update feeds. Never publish it."
fi

bundle_dir="target/release/bundle"
app="$bundle_dir/macos/Cortex.app"
lsregister=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

step "Cortex $version ($platform), signed by \"$identity\", notarize: $([[ $notarize == 1 ]] && echo yes || echo no)"

# --- 1. Clean release build ---
if [[ $clean == 1 ]]; then
  step "Cleaning the previous build"
  rm -rf apps/desktop/dist "$bundle_dir"
  cargo clean --release -p cortex-desktop
fi

step "Building, signing and bundling (app and updater bundle)"
(
  export APPLE_SIGNING_IDENTITY="$identity"
  # Without complete credentials the bundler must not try to notarize.
  if [[ $notarize == 0 ]]; then unset "${apple_vars[@]}"; fi
  pnpm --filter desktop tauri build --bundles app --config "$build_config"
)

tarball="$bundle_dir/macos/Cortex.app.tar.gz"
[[ -d "$app" && -f "$tarball" && -f "$tarball.sig" ]] || die "the build did not produce the app and updater bundle"

# --- 2. Check the signature ---
step "Checking the code signature"
codesign --verify --deep --strict "$app"
plist_version="$(defaults read "$PWD/$app/Contents/Info.plist" CFBundleShortVersionString)"
[[ "$plist_version" == "$version" ]] || die "Info.plist says $plist_version, expected $version"
requirement="$(codesign -d -r- "$app" 2>&1 | sed -n 's/^designated => //p')"
echo "designated requirement: $requirement"
[[ "$requirement" == *'identifier "dev.roobi.cortex"'* ]] || die "unexpected designated requirement"
if [[ "$identity" == "Cortex Dev Signing" ]]; then
  # Privacy grants follow this requirement: it must name this certificate, so updates keep them.
  leaf="$(security find-certificate -c "$identity" -Z | sed -n 's/^SHA-1 hash: //p' | head -1 | tr 'A-F' 'a-f')"
  [[ "$requirement" == *"certificate leaf = H\"$leaf\""* ]] || die "designated requirement does not name the $identity certificate"
fi
sig_info="$(codesign -dv "$app" 2>&1)"
grep -q 'flags=0x10000(runtime)' <<<"$sig_info" || die "hardened runtime is off"

# The updater refuses bundles whose signature does not name this version (requireSignedVersion).
sig_text="$(base64 -d <"$tarball.sig")"
grep -q $'\tversion:'"$version"'$' <<<"$sig_text" || die "updater signature does not carry version $version"
entries="$(tar -tzf "$tarball")"
grep -qv '^Cortex.app/' <<<"$entries" && die "updater bundle holds more than Cortex.app"

# --- 3. DMG: the app plus an Applications link, laid out by dmgbuild ---
# Tauri's own DMG step scripts Finder to lay out the window, which needs an Automation
# grant and a logged-in GUI. dmgbuild writes the layout (.DS_Store) directly.
step "Building the DMG"
venv="${CORTEX_DMGBUILD_VENV:-$HOME/.cache/cortex/dmgbuild-venv}"
if [[ ! -x "$venv/bin/dmgbuild" ]]; then
  echo "installing dmgbuild into $venv (one time)"
  python3 -m venv "$venv"
  "$venv/bin/pip" install --quiet "dmgbuild==1.6.7"
fi
mkdir -p "$bundle_dir/dmg"
dmg="$bundle_dir/dmg/Cortex_${version}_${arch}.dmg"
rm -f "$dmg"
dmg_dir="apps/desktop/src-tauri/dmg"
"$venv/bin/dmgbuild" -s "$dmg_dir/dmgbuild-settings.py" -D app="$app" -D background="$dmg_dir/background.png" \
  Cortex "$dmg" >/dev/null
# Signed like the app (a Developer ID signature is what notarization of the DMG needs).
if [[ $notarize == 1 ]]; then
  codesign --force --sign "$identity" --timestamp "$dmg"
else
  codesign --force --sign "$identity" "$dmg"
fi
codesign --verify "$dmg"

# --- Notarize and staple the DMG (the bundler already did the app) ---
if [[ $notarize == 1 ]]; then
  step "Notarizing and stapling the DMG"
  xcrun notarytool submit "$dmg" "${notary_auth[@]}" --wait
  xcrun stapler staple "$dmg"
  xcrun stapler validate "$app"
  spctl --assess --type open --context context:primary-signature --verbose "$dmg"
fi

# --- 4. Check the DMG: it mounts, holds the signed app and an Applications link ---
step "Checking the DMG"
hdiutil verify -quiet "$dmg"
mnt="$(mktemp -d -t cortex-dmg)"
copy="$(mktemp -d -t cortex-copy)"
cleanup() {
  hdiutil detach -quiet "$mnt" 2>/dev/null || true
  [[ -d "$copy/Cortex.app" ]] && "$lsregister" -u "$copy/Cortex.app" 2>/dev/null || true
  rm -rf "$copy"
  rmdir "$mnt" 2>/dev/null || true
}
trap cleanup EXIT
hdiutil attach -quiet -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$dmg"
[[ -L "$mnt/Applications" ]] || die "the DMG has no Applications link"
codesign --verify --deep --strict "$mnt/Cortex.app"
ditto "$mnt/Cortex.app" "$copy/Cortex.app"
codesign --verify --deep --strict "$copy/Cortex.app"
cleanup
trap - EXIT

# --- 5. Release folder: DMG, updater bundle, manifest, sums ---
step "Writing $out"
rm -rf "$out"
mkdir -p "$out"
bundle_name="Cortex_${version}_${arch}.app.tar.gz"
cp "$dmg" "$out/Cortex_${version}_${arch}.dmg"
cp "$tarball" "$out/$bundle_name"
cp "$tarball.sig" "$out/$bundle_name.sig"

VERSION="$version" NOTES="$notes" PLATFORM="$platform" URL="$base_url/$version/$bundle_name" \
  SIG_FILE="$out/$bundle_name.sig" node -e '
    const fs = require("node:fs");
    const e = process.env;
    const manifest = {
      version: e.VERSION,
      notes: e.NOTES,
      pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
      platforms: { [e.PLATFORM]: { signature: fs.readFileSync(e.SIG_FILE, "utf8").trim(), url: e.URL } },
    };
    fs.writeFileSync(process.argv[1], JSON.stringify(manifest, null, 2) + "\n");
  ' "$out/latest.json"

{
  echo "Cortex $version ($platform)"
  echo "commit:      $(git rev-parse HEAD)$([[ -n "$(git status --porcelain)" ]] && echo ' (uncommitted changes)')"
  echo "built:       $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "signed by:   $identity"
  echo "requirement: $requirement"
  echo "notarized:   $([[ $notarize == 1 ]] && echo yes || echo no)"
  echo "update feed: latest.json -> $base_url/$version/$bundle_name"
  [[ $local_feed == 1 ]] && echo "LOCAL TEST BUILD: accepts plain-HTTP feeds. Do not publish."
} >"$out/BUILD.txt"

(cd "$out" && shasum -a 256 ./*.dmg ./*.tar.gz ./*.sig latest.json | sed 's#  \./#  #' >SHA256SUMS)

# Launch Services must not pick the build folder's app over the installed one.
"$lsregister" -u "$PWD/$app" 2>/dev/null || true

step "Done. Nothing was uploaded."
ls -lh "$out"
if [[ $local_feed == 1 ]]; then
  echo "Local test build: serve it only from 127.0.0.1 (see docs/engineering/desktop.md). Never publish it."
  exit 0
fi
cat <<EOF

To publish (by hand, once hosting exists):
  - put $(basename "$out")/ at $base_url/$version/ (the DMG and the .app.tar.gz),
  - then replace $base_url/latest.json with $out/latest.json. Installed apps update from it.
EOF
