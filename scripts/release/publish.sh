#!/usr/bin/env bash
# Publishes a release that scripts/release/assemble.sh put together: a GitHub release v<version>
# in the public downloads repository with every file in dist/release/<version>/assets/.
# Run by a person, after a last look at the files. It asks before changing anything.
#
#   scripts/release/publish.sh <version> [--dry-run]
#
#   --dry-run   check the files and print the gh commands without running them
#
# Environment: CORTEX_RELEASES_REPO (default roobiwebdev/cortex-releases).
# See docs/engineering/releasing.md.
set -euo pipefail

die() {
  echo "publish: $*" >&2
  exit 1
}

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

version="${1:-}"
if [[ "$version" == "-h" || "$version" == "--help" ]]; then sed -n '2,11p' "$0"; exit 0; fi
[[ "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]] ||
  die "usage: $0 <version> [--dry-run]"
shift
dry_run=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) dry_run=1; shift ;;
    *) die "unknown option $1" ;;
  esac
done

repo="${CORTEX_RELEASES_REPO:-roobiwebdev/cortex-releases}"
tag="v$version"
rel="dist/release/$version"
assets="$rel/assets"
notes="$rel/notes.md"
base_url="https://github.com/$repo/releases/download/$tag"

# --- Check what is about to go out ---
[[ -d "$assets" && -f "$notes" ]] || die "no assembled release in $rel; run scripts/release/assemble.sh $version"
(cd "$assets" && shasum -a 256 -c --quiet SHA256SUMS) || die "SHA256SUMS does not match the files in $assets"
node scripts/release/updater-manifest.mjs check --dir "$assets" --version "$version" --base-url "$base_url" >/dev/null ||
  die "latest.json does not check out"
# (macOS ships bash 3.2: no mapfile.)
files=()
while IFS= read -r f; do files+=("$f"); done < <(find "$assets" -maxdepth 1 -type f | LC_ALL=C sort)
[[ ${#files[@]} -gt 0 ]] || die "$assets is empty"
listed="$(sed 's/^[0-9a-f]*  //' "$assets/SHA256SUMS" | LC_ALL=C sort)"
on_disk="$(for f in "${files[@]}"; do basename "$f"; done | grep -vx SHA256SUMS | LC_ALL=C sort)"
[[ "$listed" == "$on_disk" ]] || die "$assets holds files that SHA256SUMS does not list (or the other way round)"

create_repo=(gh repo create "$repo" --public --add-readme --description "Cortex desktop downloads")
# --latest: installed apps and the site follow releases/latest/download/..., which skips
# pre-releases, so a release must not be marked as one.
create_release=(gh release create "$tag" --repo "$repo" --title "Cortex $version" --notes-file "$notes" --latest "${files[@]}")

echo "Cortex $version -> https://github.com/$repo/releases/tag/$tag (${#files[@]} files):"
for f in "${files[@]}"; do printf '  %-40s %s\n' "$(basename "$f")" "$(du -h "$f" | cut -f1)"; done

if [[ $dry_run == 1 ]]; then
  echo
  echo "Dry run. These commands would run:"
  echo "  # only when $repo does not exist yet:"
  printf '  %q' "${create_repo[@]}"
  echo
  printf '  %q' "${create_release[@]}"
  echo
  exit 0
fi

command -v gh >/dev/null || die "gh (GitHub CLI) is not installed"
gh auth status >/dev/null 2>&1 || die "gh is not signed in (gh auth login)"
need_repo=0
gh repo view "$repo" >/dev/null 2>&1 || need_repo=1
gh release view "$tag" --repo "$repo" >/dev/null 2>&1 && die "$repo already has a release $tag"

echo
[[ $need_repo == 1 ]] && echo "$repo does not exist yet: it will be created, public, with a README."
echo "This makes Cortex $version public and offers it to every installed copy as an update."
read -r -p "Type $version to publish: " answer
[[ "$answer" == "$version" ]] || die "not published"

if [[ $need_repo == 1 ]]; then
  "${create_repo[@]}"
fi
"${create_release[@]}"
echo "Published. Feed: https://github.com/$repo/releases/latest/download/latest.json"
