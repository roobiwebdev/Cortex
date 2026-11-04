#!/usr/bin/env bash
# Builds the Linux installers (deb, rpm, AppImage) for the Docker host's architecture in the
# cortex-linux-dev container and stages them under the release names in
# dist/release/<version>/linux-<arch>/. On an Apple Silicon Mac that is linux-arm64: the
# fallback when GitHub's arm64 runners are not available to the repository.
#
#   scripts/release/build-linux-docker.sh [--check] [--remove-volumes]
#
#   --check            run fmt, clippy and the Linux tests first (as CI does)
#   --remove-volumes   delete the build volumes afterwards (they cache the target and the
#                      cargo registry between runs; several GB)
#
# Needs Docker and the image (scripts/desktop/linux-dev.Dockerfile):
#   docker build -t cortex-linux-dev -f scripts/desktop/linux-dev.Dockerfile scripts/desktop
# The files are not signed; scripts/release/assemble.sh signs them.
set -euo pipefail

die() {
  echo "build-linux-docker: $*" >&2
  exit 1
}
step() { echo "==> $*"; }

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

check=0
remove=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check=1; shift ;;
    --remove-volumes) remove=1; shift ;;
    -h | --help) sed -n '2,16p' "$0"; exit 0 ;;
    *) die "unknown option $1 (see --help)" ;;
  esac
done

image="${CORTEX_LINUX_IMAGE:-cortex-linux-dev}"
target_volume=cortex-release-linux-target
cargo_volume=cortex-release-linux-cargo
docker image inspect "$image" >/dev/null 2>&1 ||
  die "no Docker image $image; docker build -t $image -f scripts/desktop/linux-dev.Dockerfile scripts/desktop"

version="$(node -p 'require("./apps/desktop/src-tauri/tauri.conf.json").version')"
case "$(docker run --rm "$image" uname -m)" in
  aarch64) group=linux-arm64 ;;
  x86_64) group=linux-x64 ;;
  *) die "unsupported container architecture" ;;
esac
out="dist/release/$version/$group"

# The image has no Node: the web assets are platform-neutral, so they are built here.
step "Building the web assets"
pnpm --filter desktop build >/dev/null

step "Cortex $version, $group, in $image"
docker volume create "$target_volume" >/dev/null
docker volume create "$cargo_volume" >/dev/null
rm -rf "$out"
docker run --rm \
  -v "$root":/src -v "$target_volume":/target -v "$cargo_volume":/usr/local/cargo/registry \
  -e CHECK="$check" -e GROUP="$group" -e VERSION="$version" -e OUT="$out" \
  -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 \
  -w /src "$image" bash -euo pipefail -c '
    crates=(-p rec-core -p rec-engine-linux -p cortex-desktop)
    if [[ $CHECK == 1 ]]; then
      cargo fmt --check "${crates[@]}"
      cargo clippy --locked --no-deps "${crates[@]}" --all-targets -- -D warnings
      cargo test --locked "${crates[@]}" -- --test-threads=2
    fi
    rm -rf /target/release/bundle
    cd apps/desktop/src-tauri
    cargo tauri build --bundles deb,rpm,appimage --config "{\"build\":{\"beforeBuildCommand\":\"\"}}"
    cd /src
    bash scripts/release/stage.sh "$GROUP" /target/release/bundle "$OUT" "$VERSION"
  '

if [[ $remove == 1 ]]; then
  step "Removing the build volumes"
  docker volume rm "$target_volume" "$cargo_volume" >/dev/null
fi

step "Done: $out"
ls -lh "$out"
