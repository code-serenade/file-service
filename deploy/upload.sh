#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ARCHIVE_PATH="$PROJECT_DIR/dist/file-service-linux-x86_64-docker.tar.gz"
CONFIG_FILE="$SCRIPT_DIR/upload.local.env"

if [ ! -f "$ARCHIVE_PATH" ]; then
  echo "error: offline bundle not found: $ARCHIVE_PATH" >&2
  echo "run make build-docker-release first" >&2
  exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
  echo "error: upload configuration not found: $CONFIG_FILE" >&2
  echo "copy deploy/upload.env.example to deploy/upload.local.env and set SCP_TARGET" >&2
  exit 1
fi

set -a
. "$CONFIG_FILE"
set +a

: "${SCP_TARGET:?SCP_TARGET is required in deploy/upload.local.env}"

echo "==> Uploading $ARCHIVE_PATH"
scp "$ARCHIVE_PATH" "$SCP_TARGET"

echo "==> Uploaded to $SCP_TARGET"
