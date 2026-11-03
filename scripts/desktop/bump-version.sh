#!/usr/bin/env bash
# Sets the Cortex desktop version everywhere it is written:
#   apps/desktop/src-tauri/tauri.conf.json   (what the app and the updater report)
#   apps/desktop/package.json
#   apps/desktop/src-tauri/Cargo.toml        (and the cortex-desktop entry in Cargo.lock)
# The UI reads tauri.conf.json at build time; `cargo test -p cortex-desktop` fails if the
# three disagree.
#
# Usage: scripts/desktop/bump-version.sh 0.1.1
set -euo pipefail

version="${1:-}"
# Plain semver: MAJOR.MINOR.PATCH with an optional -prerelease.
if ! [[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "usage: $0 <version>   e.g. $0 0.1.1" >&2
  exit 2
fi

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

node - "$version" <<'EOF'
const fs = require("node:fs");
const version = process.argv[2];

// Replace one exact occurrence, or fail loudly instead of writing a half-bumped tree.
function replaceOnce(path, pattern, replacement) {
  const text = fs.readFileSync(path, "utf8");
  const matches = text.match(new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : pattern.flags + "g"));
  if (!matches || matches.length !== 1) {
    console.error(`${path}: expected exactly one version field, found ${matches ? matches.length : 0}`);
    process.exit(1);
  }
  fs.writeFileSync(path, text.replace(pattern, replacement));
}

// Only the top-level "version" key (two-space indent) in the JSON files.
replaceOnce("apps/desktop/src-tauri/tauri.conf.json", /^  "version": "[^"]*"/m, `  "version": "${version}"`);
replaceOnce("apps/desktop/package.json", /^  "version": "[^"]*"/m, `  "version": "${version}"`);
// The [package] version line (the first `version =` at the start of a line).
replaceOnce("apps/desktop/src-tauri/Cargo.toml", /^version = "[^"]*"/m, `version = "${version}"`);
replaceOnce("Cargo.lock", /(name = "cortex-desktop"\nversion = )"[^"]*"/, `$1"${version}"`);
EOF

echo "Cortex desktop version is now $version:"
grep -H '^  "version"' apps/desktop/src-tauri/tauri.conf.json apps/desktop/package.json
grep -H '^version' apps/desktop/src-tauri/Cargo.toml
