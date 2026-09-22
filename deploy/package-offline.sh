#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

IMAGE="file-service:local"
OUTPUT_ROOT="$PROJECT_DIR/dist"
BUNDLE_NAME="file-service-linux-x86_64-docker"
BUNDLE_DIR="$OUTPUT_ROOT/$BUNDLE_NAME"
ARCHIVE_PATH="$OUTPUT_ROOT/$BUNDLE_NAME.tar.gz"
IMAGE_TAR="file-service-images.tar"

random_hex() {
  od -An -N 32 -tx1 /dev/urandom | tr -d ' \n'
}

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "error: missing Docker image: $IMAGE" >&2
  echo "run deploy/build-release.sh first" >&2
  exit 1
fi

architecture="$(docker image inspect --format '{{.Architecture}}' "$IMAGE")"
if [ "$architecture" != "amd64" ]; then
  echo "error: $IMAGE has architecture $architecture, expected amd64" >&2
  exit 1
fi

mkdir -p "$OUTPUT_ROOT"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"

echo "==> Saving Docker image"
docker save --platform linux/amd64 "$IMAGE" -o "$BUNDLE_DIR/$IMAGE_TAR"

cp "$SCRIPT_DIR/runtime/deploy.env" "$BUNDLE_DIR/deploy.env"
cp "$SCRIPT_DIR/runtime/docker-compose.yml" "$BUNDLE_DIR/docker-compose.yml"
cp "$SCRIPT_DIR/runtime/install.sh" "$BUNDLE_DIR/install.sh"
cp "$SCRIPT_DIR/runtime/stop.sh" "$BUNDLE_DIR/stop.sh"
cp "$SCRIPT_DIR/runtime/start.sh" "$BUNDLE_DIR/start.sh"
cp "$SCRIPT_DIR/runtime/restart.sh" "$BUNDLE_DIR/restart.sh"
cp "$SCRIPT_DIR/runtime/README.md" "$BUNDLE_DIR/README.md"
chmod +x "$BUNDLE_DIR/install.sh"
chmod +x "$BUNDLE_DIR/stop.sh"
chmod +x "$BUNDLE_DIR/start.sh"
chmod +x "$BUNDLE_DIR/restart.sh"

echo "==> Generating deployment-specific user key salt"
user_key_salt="$(random_hex)"
sed "s/replace_with_random_user_key_salt/$user_key_salt/" \
  "$BUNDLE_DIR/deploy.env" > "$BUNDLE_DIR/deploy.env.tmp"
mv "$BUNDLE_DIR/deploy.env.tmp" "$BUNDLE_DIR/deploy.env"
chmod 600 "$BUNDLE_DIR/deploy.env"

tar_args=()
if tar --help 2>/dev/null | grep -q -- '--no-xattrs'; then
  tar_args+=(--no-xattrs)
fi
if tar --help 2>/dev/null | grep -q -- '--no-mac-metadata'; then
  tar_args+=(--no-mac-metadata)
fi

echo "==> Creating $ARCHIVE_PATH"
if [ "${#tar_args[@]}" -gt 0 ]; then
  COPYFILE_DISABLE=1 tar "${tar_args[@]}" -czf "$ARCHIVE_PATH" -C "$OUTPUT_ROOT" "$BUNDLE_NAME"
else
  COPYFILE_DISABLE=1 tar -czf "$ARCHIVE_PATH" -C "$OUTPUT_ROOT" "$BUNDLE_NAME"
fi

echo "==> Done"
echo "Offline bundle: $ARCHIVE_PATH"
