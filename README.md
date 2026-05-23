# Bad Piggies Editor

> A level and save-file editor for **Bad Piggies**, written in Rust.

Built with [`egui`](https://github.com/emilk/egui), [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe), and [`wgpu`](https://github.com/gfx-rs/wgpu). Runs as a native desktop app on macOS, Windows, and Linux, and compiles to WASM for in-browser use.

[![Rust](https://img.shields.io/badge/Rust-2024-orange)](https://www.rust-lang.org)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](../LICENSE)

---

## Features

| Category | Details |
|---|---|
| **Level editing** | Open/save `.bytes`, `.yaml`, `.yml`, `.toml` level files; visually inspect and edit objects, terrain, and overrides |
| **Save editing** | Open `Progress.dat`, `*.contraption`, `Achievements.xml`; edit as raw XML or structured tables; re-export as encrypted saves |
| **Rendering** | GPU-backed rendering of terrain fills, edges, sprites, opaque props, parallax backgrounds, and dark-mask overlays |
| **CLI** | Convert formats, decrypt/encrypt save files from the command line |
| **i18n** | English and Simplified Chinese UI, auto-detected from the system locale |
| **Targets** | Native desktop (Vulkan / Metal / DX12 / OpenGL ES 3.1) + WASM/browser |

---

## Getting Started

### Prerequisites

- **Rust 1.85+**
- **Native:** a GPU that supports Vulkan, Metal, DirectX 12, or OpenGL ES 3.1
- **Linux native:** system libraries needed by `eframe`/`winit` (X11/Wayland headers, etc.)
- **WASM:** `rustup target add wasm32-unknown-unknown` and [`Trunk`](https://trunkrs.dev)

### Run natively

```bash
cargo run                  # debug
cargo build --release      # optimized binary
```

### Run in the browser

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve index.html     # opens at http://localhost:8080
```

---

## CLI Usage

The compiled binary doubles as a CLI tool. From the repo you can invoke it with `cargo run -- <args>`.

```bash
# Level format conversion
badpiggies-editor convert level.bytes level.yaml
badpiggies-editor convert level.yaml  level.toml
badpiggies-editor convert level.toml  level.bytes

# Save file crypto
badpiggies-editor decrypt Progress.dat -o Progress.xml
badpiggies-editor encrypt Progress.xml Progress.dat

# Help
badpiggies-editor --help
badpiggies-editor convert --help
```

CLI output is localized the same way as the GUI.

---

## Assets and Build System

### How assets are resolved

`build.rs` drives Unity asset resolution at compile time:

1. Uses the bundled package at `assets/data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage` by default.
2. Computes the package SHA-256 as a cache key and extracts runtime files to `target/unity_asset_cache/`.
3. Embeds extracted files into the build binary.

All bundled resources under `assets/` (icons, fonts, locales, shaders) are compiled in via `include_bytes!`/`include_str!`, so neither native nor WASM builds need an external `assets/` directory at runtime.

### Environment variables

| Variable | Effect |
|---|---|
| `BP_EDITOR_UNITY_ASSETS_DIR` | Point to a pre-extracted asset tree instead of extracting at build time |
| `BP_EDITOR_UNITYPACKAGE_PATH` | Use a custom `.unitypackage` file instead of the bundled one |
| `BP_EDITOR_UNITY_ASSET_CACHE_DIR` | Override the cache directory used by `build.rs` |

```bash
# Use an existing extracted tree
BP_EDITOR_UNITY_ASSETS_DIR=/path/to/unity_assets cargo build

# Use a custom package
BP_EDITOR_UNITYPACKAGE_PATH=../Bad-Piggies-2.3.6-Unity-Windows.unitypackage cargo build
```

To re-extract assets from the bundled package to a local `unity_assets/` tree:

```bash
python3 ../_extract_unitypackage_to_guid_layout.py
python3 ../_extract_unitypackage_to_guid_layout.py --help   # see options
```

---

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `B` | Toggle background rendering |
| `⌘Z` / `Ctrl+Z` | Undo |
| `⇧⌘Z` / `Ctrl+Y` | Redo |
| Scroll wheel | Zoom in / out |
| Drag on empty canvas | Pan |
| Click object | Select |

---

## Repository Layout

```
badpiggies-editor/
├── assets/              # Bundled resources (icons, fonts, shaders, locales, Unity package)
├── src/
│   ├── app/             # egui shell, menus, dialogs, panels, save viewer
│   ├── data/            # Embedded asset databases and lookup tables
│   ├── diagnostics/     # Error handling and logging
│   ├── domain/          # Level types, parser/serializer, terrain generation
│   ├── i18n/            # Fluent localization files
│   ├── io/              # Save-file crypto and import/export helpers
│   ├── renderer/        # wgpu scene renderer
│   ├── unity_runtime/   # Unity runtime data adapters
│   └── main.rs          # Native + WASM entry points, CLI wiring
├── build.rs             # Unity asset extraction and embedding
├── index.html           # Trunk host page for the WASM build
└── Cargo.toml
```

### Rendering pipeline

Rendering hooks into `egui` via `PaintCallback`, sharing the same frame with the UI.

| Shader | Purpose |
|---|---|
| `fill_shader` | Terrain fill with tiled textures |
| `edge_shader` | Terrain edges and curves |
| `sprite_shader` | Transparent sprite batching |
| `opaque_shader` | Opaque prop batching |
| `bg_shader` | Parallax background layers |
| `dark_mask_shader` | Dark overlay and lit-area masking |

---

## Development

```bash
# Quick checks
cargo test --message-format=short
cargo check --all-targets --message-format=short
cargo clippy --all-targets --message-format=short -- -D warnings
cargo check --target wasm32-unknown-unknown --message-format=short

# Optional: parser round-trip test (requires fixture file)
cargo test test_level_roundtrip
# fixture: ../test_levels/assetbundles/episode_1_levels.unity3d/Level_05_data.bytes
# skipped automatically if the fixture is absent
```

For known asset-migration blockers, see [`../ASSET_MIGRATION_BLOCKERS.md`](../ASSET_MIGRATION_BLOCKERS.md).

---

## Acknowledgements

- [BP-Innovation/Bad-Piggies-Original](https://github.com/BP-Innovation/Bad-Piggies-Original) — decompiled project and asset references that made level parsing and rendering possible.

---

## License

Licensed under the **GNU Affero General Public License v3.0**. See [`../LICENSE`](../LICENSE).

Game assets, textures, and level data are the property of **Rovio Entertainment** and are not covered by this license.
