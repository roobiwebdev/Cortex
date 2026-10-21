#!/bin/sh
# Render og.html to src/lib/assets/og.jpg (1200x630) with headless Chrome. Needs macOS for `sips`.
set -eu
cd "$(dirname "$0")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# Render at 2x and scale down so the text stays sharp.
"$CHROME" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
  --window-size=1200,630 --virtual-time-budget=5000 --allow-file-access-from-files \
  --screenshot="$TMP/og.png" "file://$PWD/og.html" 2>/dev/null
sips -z 630 1200 -s format jpeg -s formatOptions 85 "$TMP/og.png" --out ../../src/lib/assets/og.jpg >/dev/null
sips -g pixelWidth -g pixelHeight ../../src/lib/assets/og.jpg
