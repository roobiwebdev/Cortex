#!/bin/sh
# Render every Cortex logo raster from the master SVGs in this folder:
#   icon.svg            -> icon.png (1024, macOS shape) -> src-tauri/icons/icon.icns
#   icon-full-bleed.svg -> icon-full-bleed.png (1024)   -> src-tauri/icons/*.png, icon.ico, Square*/StoreLogo
#   tray-template.svg   -> src-tauri/icons/tray-template.png (44 px, the menu-bar template at 2x)
#   mark.svg            -> src-tauri/icons/tray-color.png (64 px, the Windows and Linux tray icon in colour)
#   site: favicon.png, apple-touch-icon.png, src/lib/assets/icon.webp, scripts/og/icon.webp
# Needs macOS (`sips`), Google Chrome, `cwebp` (brew install webp) and the desktop app's node_modules.
# Run from anywhere: sh apps/desktop/branding/render.sh. Then run `pnpm --filter site og` for the share image.
set -eu
cd "$(dirname "$0")"
BRANDING="$PWD"
DESKTOP="$(cd .. && pwd)"
ICONS="$DESKTOP/src-tauri/icons"
SITE="$(cd ../../site && pwd)"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# render <svg> <out.png>: a 1024x1024 PNG with a transparent background.
render() {
  "$CHROME" --headless --disable-gpu --hide-scrollbars --default-background-color=00000000 \
    --force-device-scale-factor=1 --window-size=1024,1024 --screenshot="$2" "file://$BRANDING/$1" 2>/dev/null
}
# resize <in.png> <size> <out.png>
resize() { sips -z "$2" "$2" "$1" --out "$3" >/dev/null; }

render icon.svg icon.png
render icon-full-bleed.svg icon-full-bleed.png
render tray-template.svg "$TMP/tray.png"
# Windows and Linux do not tint template images, so a black icon vanishes on a dark taskbar.
render mark.svg "$TMP/tray-color.png"

# Windows and Linux files come from the full-bleed tile; the macOS .icns keeps Apple's margin.
(cd "$DESKTOP" && pnpm exec tauri icon "$BRANDING/icon-full-bleed.png" >/dev/null 2>&1)
(cd "$DESKTOP" && pnpm exec tauri icon "$BRANDING/icon.png" -o "$TMP/mac" >/dev/null 2>&1)
cp "$TMP/mac/icon.icns" "$ICONS/icon.icns"
# No mobile targets: drop the Android and iOS sets the CLI also writes.
rm -rf "$ICONS/android" "$ICONS/ios"

resize "$TMP/tray.png" 44 "$ICONS/tray-template.png"
resize "$TMP/tray-color.png" 64 "$ICONS/tray-color.png"

resize icon-full-bleed.png 64 "$SITE/static/favicon.png"
resize icon-full-bleed.png 180 "$SITE/static/apple-touch-icon.png"
resize icon.png 224 "$TMP/icon-224.png"
resize icon.png 336 "$TMP/icon-336.png"
cwebp -quiet -q 90 -alpha_q 100 "$TMP/icon-224.png" -o "$SITE/src/lib/assets/icon.webp"
cwebp -quiet -q 90 -alpha_q 100 "$TMP/icon-336.png" -o "$SITE/scripts/og/icon.webp"

echo "Rendered app icons into $ICONS and site icons into $SITE"
