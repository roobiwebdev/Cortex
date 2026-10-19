#!/bin/sh
# Render og.html to static/og.jpg (1200x630) with headless Chrome. Needs macOS for `sips`.
# Run `npm install` first: the template loads its fonts from node_modules.
set -eu
cd "$(dirname "$0")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
PNG="$(mktemp -d)/og.png"
"$CHROME" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
  --window-size=1200,630 --virtual-time-budget=5000 --allow-file-access-from-files \
  --screenshot="$PNG" "file://$PWD/og.html"
sips -s format jpeg -s formatOptions 90 "$PNG" --out ../../static/og.jpg >/dev/null
rm -r "$(dirname "$PNG")"
sips -g pixelWidth -g pixelHeight ../../static/og.jpg
