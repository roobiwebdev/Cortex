#!/usr/bin/env bash
# Assembles a complete Cortex release in dist/release/<version>/assets/: the macOS build from
# this Mac, the Windows and Linux installers from the release.yml run, every updater file
# signed with the updater key, one multi-platform latest.json and SHA256SUMS. It uploads
# nothing; it prints the publish commands (scripts/release/publish.sh runs them).
#
#   scripts/release/assemble.sh <version> [options]
#
#   --from DIR          take the Windows/Linux files from DIR (searched recursively, e.g. a
#                       `gh run download` folder) instead of downloading them
#   --run-id ID         download from this release.yml run (default: the latest completed run
#                       titled "Release v<version>" in $CORTEX_SOURCE_REPO)
#   --notes TEXT | --notes-file FILE   release notes (feed and GitHub release)
#   --rebuild-macos     rebuild the macOS part even if dist/release/<version>/macos is current
#   --require-all       fail when the Linux arm64 files are missing (default: warn and leave
#                       them out)
#   --allow-commit-mismatch   accept CI artifacts built from another commit than HEAD
#   --allow-missing GROUP     leave out a whole platform group (windows-x64, linux-x64 or
#                       linux-arm64) whose files are absent; repeatable. The feed and sums then
#                       omit it; add it later by re-running and uploading with --clobber.
#
# Environment: CORTEX_SOURCE_REPO (default roobiwebdev/cortex), CORTEX_RELEASES_REPO (default
# roobiwebdev/cortex-releases), CORTEX_UPDATER_KEY (default ~/.config/cortex/updater.key),
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD (default: Keychain item cortex-updater-key). The macOS
# build takes the variables listed in scripts/desktop/release-macos.sh --help.
# See docs/engineering/releasing.md.
set -euo pipefail

die() {
  echo "assemble: $*" >&2
  exit 1
}
step() { echo "==> $*"; }
warn() { echo "assemble: WARNING: $*" >&2; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"

version="${1:-}"
if [[ "$version" == "-h" || "$version" == "--help" ]]; then sed -n '2,25p' "$0"; exit 0; fi
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]] ||
  die "usage: $0 <version> [options]   (see --help)"
shift

from=""
run_id=""
notes=""
notes_file=""
rebuild_macos=0
require_all=0
allow_missing=()
allow_mismatch=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --from) from="${2:?--from needs a folder}"; shift 2 ;;
    --run-id) run_id="${2:?--run-id needs an id}"; shift 2 ;;
    --notes) notes="${2:?--notes needs text}"; shift 2 ;;
    --notes-file) notes_file="${2:?--notes-file needs a path}"; shift 2 ;;
    --rebuild-macos) rebuild_macos=1; shift ;;
    --require-all) require_all=1; shift ;;
    --allow-missing) allow_missing+=("${2:?--allow-missing needs a group}"); shift 2 ;;
    --allow-commit-mismatch) allow_mismatch=1; shift ;;
    -h | --help) sed -n '2,25p' "$0"; exit 0 ;;
    *) die "unknown option $1 (see --help)" ;;
  esac
done

source_repo="${CORTEX_SOURCE_REPO:-roobiwebdev/cortex}"
releases_repo="${CORTEX_RELEASES_REPO:-roobiwebdev/cortex-releases}"
tag="v$version"
base_url="https://github.com/$releases_repo/releases/download/$tag"
manifest=(node scripts/release/updater-manifest.mjs)

conf_version="$(node -p 'require("./apps/desktop/src-tauri/tauri.conf.json").version')"
[[ "$conf_version" == "$version" ]] ||
  die "tauri.conf.json is $conf_version, not $version (check out the release commit, or run scripts/desktop/bump-version.sh)"

rel="dist/release/$version"
ci="$rel/ci"
assets="$rel/assets"
mkdir -p "$rel"

# --- Notes: one file for the feed and the GitHub release ---
if [[ -n "$notes_file" ]]; then
  cp "$notes_file" "$rel/notes.md"
elif [[ -n "$notes" ]]; then
  printf '%s\n' "$notes" >"$rel/notes.md"
elif [[ ! -s "$rel/notes.md" ]]; then
  printf 'Cortex %s\n' "$version" >"$rel/notes.md"
fi

# --- 1. Windows and Linux installers from CI ---
rm -rf "$ci"
mkdir -p "$ci"
if [[ -n "$from" ]]; then
  [[ -d "$from" ]] || die "--from $from is not a folder"
  step "Taking the CI files from $from"
  src="$from"
else
  command -v gh >/dev/null || die "gh (GitHub CLI) is needed to download the CI artifacts, or pass --from"
  if [[ -z "$run_id" ]]; then
    step "Finding the release.yml run for $tag in $source_repo"
    run_id="$(gh run list --repo "$source_repo" --workflow release.yml --limit 50 \
      --json databaseId,displayTitle,status \
      --jq "map(select(.displayTitle == \"Release $tag\" and .status == \"completed\")) | .[0].databaseId // empty")"
    [[ -n "$run_id" ]] || die "no completed release.yml run titled \"Release $tag\" in $source_repo (push the tag $tag or run the workflow; see releasing.md)"
  fi
  run_info="$(gh run view "$run_id" --repo "$source_repo" --json headSha,conclusion,url --jq '"\(.headSha) \(.conclusion) \(.url)"')"
  read -r run_sha run_conclusion run_url <<<"$run_info"
  echo "run $run_id ($run_conclusion): $run_url"
  head_sha="$(git rev-parse HEAD)"
  if [[ "$run_sha" != "$head_sha" ]]; then
    msg="the CI run built $run_sha, this checkout is $head_sha: the macOS build would come from another commit"
    if [[ $allow_mismatch == 1 ]]; then warn "$msg"; else die "$msg (check out $run_sha, or pass --allow-commit-mismatch)"; fi
  fi
  src="$rel/ci-download"
  rm -rf "$src"
  gh run download "$run_id" --repo "$source_repo" --dir "$src"
fi

# Copy each expected file; groups are all-or-nothing.
present_groups=()
for group in windows-x64 linux-x64 linux-arm64; do
  missing=()
  found=0
  while IFS= read -r file; do
    match="$(find "$src" -type f -name "$file" | head -2)"
    if [[ -z "$match" ]]; then
      missing+=("$file")
    elif [[ "$(wc -l <<<"$match")" -gt 1 ]]; then
      die "more than one $file under $src"
    else
      cp "$match" "$ci/$file"
      found=$((found + 1))
    fi
  done < <("${manifest[@]}" assets --group "$group" --only files)
  if [[ ${#missing[@]} == 0 ]]; then
    present_groups+=("$group")
  elif [[ $found -gt 0 ]]; then
    die "$group is incomplete: missing ${missing[*]}"
  elif [[ $group == linux-arm64 && $require_all == 0 ]]; then
    # The local fallback (scripts/release/build-linux-docker.sh) stages into the release folder.
    if [[ -d "$rel/linux-arm64" ]]; then
      for file in $("${manifest[@]}" assets --group linux-arm64 --only files); do
        [[ -f "$rel/linux-arm64/$file" ]] || die "$rel/linux-arm64 is incomplete: missing $file"
        cp "$rel/linux-arm64/$file" "$ci/$file"
      done
      echo "linux-arm64: taken from $rel/linux-arm64 (local Docker build)"
      present_groups+=("$group")
    else
      warn "no Linux arm64 files: this release leaves them out (the site's arm64 links will 404). Build them with scripts/release/build-linux-docker.sh, or pass --require-all to stop here."
    fi
  elif [[ " ${allow_missing[*]} " == *" $group "* ]]; then
    warn "no $group files: left out of this release (--allow-missing)."
  else
    die "$group files are missing: ${missing[*]}"
  fi
done
echo "CI groups: ${present_groups[*]}"

# --- 2. macOS ---
mac="$rel/macos"
if [[ $rebuild_macos == 1 || ! -f "$mac/VERSION" || "$(cat "$mac/VERSION" 2>/dev/null)" != "$version" ]]; then
  step "Building the macOS release"
  scripts/desktop/release-macos.sh --notes-file "$rel/notes.md"
else
  mac_commit="$(sed -n 's/^commit: *//p' "$mac/BUILD.txt")"
  step "Reusing the macOS build in $mac ($mac_commit)"
  [[ "$mac_commit" == "$(git rev-parse HEAD)" ]] ||
    warn "the macOS build is from $mac_commit, this checkout is $(git rev-parse HEAD); --rebuild-macos rebuilds it"
fi
for file in $("${manifest[@]}" assets --group macos); do
  [[ -f "$mac/$file" ]] || die "$mac/$file is missing; run with --rebuild-macos"
done
darwin_arches="$(sed -n 's/^arches: *//p' "$mac/BUILD.txt" | tr ' ' ',')"
[[ -n "$darwin_arches" ]] || die "$mac/BUILD.txt names no architectures; run with --rebuild-macos"

# --- 3. Collect under the release names ---
step "Collecting $assets"
rm -rf "$assets"
mkdir -p "$assets"
for file in $("${manifest[@]}" assets --group macos); do cp "$mac/$file" "$assets/"; done
for group in "${present_groups[@]}"; do
  for file in $("${manifest[@]}" assets --group "$group" --only files); do cp "$ci/$file" "$assets/"; done
done

# --- 4. Sign the Windows and Linux updater files (macOS was signed by its build) ---
step "Signing the updater files"
key_path="${CORTEX_UPDATER_KEY:-$HOME/.config/cortex/updater.key}"
[[ -r "$key_path" ]] || die "updater key not found at $key_path (see docs/engineering/desktop.md, Keys)"
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ]]; then
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s cortex-updater-key -w 2>/dev/null)" ||
    die "no Keychain item cortex-updater-key; set TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
unset TAURI_SIGNING_PRIVATE_KEY
for group in "${present_groups[@]}"; do
  for file in $("${manifest[@]}" assets --group "$group" --only updater); do
    rm -f "$assets/$file.sig"
    # The CLI prints the signature (public), never the key or password.
    pnpm --dir apps/desktop exec tauri signer sign --private-key-path "$key_path" \
      --app-version "$version" "$PWD/$assets/$file" >/dev/null ||
      die "signing $file failed"
    [[ -f "$assets/$file.sig" ]] || die "no signature written for $file"
    echo "  signed $file"
  done
done
unset TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# --- 5. latest.json: written from the files, then checked the way the updater reads it ---
step "Writing latest.json"
require="macos"
for group in windows-x64 linux-x64; do
  [[ " ${allow_missing[*]} " == *" $group "* ]] || require="$require,$group"
done
[[ $require_all == 1 ]] && require="$require,linux-arm64"
"${manifest[@]}" write --dir "$assets" --version "$version" --base-url "$base_url" \
  --notes-file "$rel/notes.md" --darwin-arches "$darwin_arches" --require "$require"
"${manifest[@]}" check --dir "$assets" --version "$version" --base-url "$base_url"

# --- 6. Checksums ---
(cd "$assets" && find . -maxdepth 1 -type f ! -name SHA256SUMS | sed 's#^\./##' | LC_ALL=C sort |
  xargs shasum -a 256 >SHA256SUMS)
cp "$mac/BUILD.txt" "$rel/BUILD-macos.txt"

step "Done. Nothing was uploaded."
ls -lh "$assets"
cat <<EOF

Publish (or run: scripts/release/publish.sh $version):

  # once, if $releases_repo does not exist yet:
  gh repo create $releases_repo --public --add-readme --description "Cortex desktop downloads"

  gh release create $tag --repo $releases_repo --title "Cortex $version" \\
    --notes-file $rel/notes.md --latest $assets/*

Installed apps read https://github.com/$releases_repo/releases/latest/download/latest.json,
and the site links to .../releases/latest/download/<asset>. Both follow the newest release that
is not a pre-release, so do not pass --prerelease (see docs/engineering/releasing.md).
EOF
