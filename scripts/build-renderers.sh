#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:-all}"

build_native() {
  cargo build \
    --manifest-path "$repo_root/Cargo.toml" \
    --package badpiggies_editor_renderer
}

build_web() {
  local crate="$repo_root/crates/renderer"
  local asset_root="$repo_root/app/assets/renderer"

  rm -rf "$asset_root/threaded"
  wasm-pack build "$crate" \
    --target web \
    --release \
    --out-dir "$asset_root/pkg" \
    --out-name badpiggies_editor_renderer
  rm -f "$asset_root/pkg/.gitignore"
}

case "$mode" in
  native)
    build_native
    ;;
  web)
    build_web
    ;;
  all)
    build_native
    build_web
    ;;
  *)
    echo "usage: $0 [native|web|all]" >&2
    exit 2
    ;;
esac
