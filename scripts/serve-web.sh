#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

./scripts/build-web-worker.sh
./scripts/build-web-renderer.sh
exec dx serve \
  --package badpiggies-editor-app \
  --platform web \
  "$@"
