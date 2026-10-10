#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PLATFORM="linux/amd64"
BUILDER_IMAGE="rust:1.94-alpine3.22"
IMAGE="file-service:local"
ARTIFACT_DIR="$SCRIPT_DIR/artifacts"

fail() {
  echo "error: $*" >&2
  exit 1
}

for command_name in cargo docker; do
  command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done

docker info >/dev/null 2>&1 || fail "Docker daemon is unavailable"

if ! docker image inspect "$BUILDER_IMAGE" >/dev/null 2>&1; then
  echo "error: required local image is missing: $BUILDER_IMAGE" >&2
  echo "prepare it yourself before running this script:" >&2
  echo "  docker pull --platform linux/amd64 $BUILDER_IMAGE" >&2
  exit 1
fi

if ! docker run --rm --pull=never --platform "$PLATFORM" \
  --entrypoint /bin/sh "$BUILDER_IMAGE" -c 'rustc --version' >/dev/null; then
  fail "local linux/amd64 variant is unavailable for $BUILDER_IMAGE"
fi

cargo_home_dir="$(cd "$(dirname "$(command -v cargo)")/.." && pwd)"
registry_dir="$cargo_home_dir/registry"
[ -d "$registry_dir" ] || fail "missing local Cargo registry: $registry_dir"

cleanup() {
  rm -rf "$ARTIFACT_DIR"
}
trap cleanup EXIT

mkdir -p "$ARTIFACT_DIR"

echo "==> Building file-service for linux/amd64 without network access"
docker run --rm --pull=never --platform "$PLATFORM" --network none \
  -e CARGO_HOME=/cargo-home \
  -v "$PROJECT_DIR:/workspace" \
  -v "$registry_dir:/cargo-home/registry" \
  -w /workspace \
  "$BUILDER_IMAGE" \
  cargo build --offline --locked --release

binary_path="$PROJECT_DIR/target/release/file-service"
[ -x "$binary_path" ] || fail "missing compiled binary: $binary_path"
cp "$binary_path" "$ARTIFACT_DIR/file-service"

container_id="$(docker create --pull=never --platform "$PLATFORM" "$BUILDER_IMAGE")"
cleanup_container() {
  docker rm -f "$container_id" >/dev/null 2>&1 || true
}
trap 'cleanup_container; cleanup' EXIT
docker cp "$container_id:/etc/ssl/certs/ca-certificates.crt" "$ARTIFACT_DIR/ca-certificates.crt"
cleanup_container
trap cleanup EXIT

echo "==> Building minimal linux/amd64 runtime image without network access"
docker build --platform "$PLATFORM" --pull=false --network=none \
  --tag "$IMAGE" \
  --file deploy/Dockerfile \
  "$PROJECT_DIR"

architecture="$(docker image inspect --format '{{.Architecture}}' "$IMAGE")"
[ "$architecture" = "amd64" ] || fail "$IMAGE has architecture $architecture, expected amd64"

"$SCRIPT_DIR/package-offline.sh"
