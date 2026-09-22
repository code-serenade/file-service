#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

command -v docker >/dev/null 2>&1 || {
  echo "error: Docker is not installed" >&2
  exit 1
}

docker compose --env-file deploy.env down
