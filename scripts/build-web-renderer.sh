#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
crate="$repo_root/crates/renderer"
asset_root="$repo_root/app/assets/renderer"

rm -rf "$asset_root/threaded"

wasm-pack build "$crate" \
  --target web \
  --release \
  --out-dir "$asset_root/pkg" \
  --out-name badpiggies_editor_renderer

rm -f "$asset_root/pkg/.gitignore"
