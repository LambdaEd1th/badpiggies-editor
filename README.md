# Bad Piggies Editor

A native desktop (and WASM) level editor for the **Bad Piggies** mobile game, built with Rust and GPU-accelerated rendering.

[![Rust](https://img.shields.io/badge/Rust-2024-orange)](https://www.rust-lang.org)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](../LICENSE)

## Features

- **Level parsing** — Full binary `.bytes` level format parser, mirroring the original Unity `LevelLoader.cs`
- **GPU rendering** — Custom WGSL shaders via [wgpu](https://wgpu.rs) for all visual layers
- **Terrain** — Fill meshes + edge/curve meshes with splat-mapped textures
- **Backgrounds** — 10 parallax themes (Jungle, Forest, Plateau, Night, Halloween, MayaCave, …)
- **Sprites** — Atlas-based rendering with `RuntimeSpriteDatabase` and `CartoonFrameSprite` support
- **Compound objects** — Multi-part objects (fans, motors, hinges, boxes, …) with sub-sprite composition
- **Performance** — Frustum culling + GPU draw call batching (one render pass per shader type per frame)
- **Properties panel** — Editable object properties with override tree
- **Save viewer / editor** — Open `Progress.dat`, `*.contraption`, and `Achievements.xml`, edit raw XML or structured tables, export XML or re-encrypted save files, and preview contraptions on a grid
- **Undo / Redo** — Full snapshot-based undo/redo history (⌘Z / Ctrl+Z, ⇧⌘Z / Ctrl+Y)
- **Multi-format I/O** — Import/export levels as binary `.bytes`, YAML, or TOML
- **CLI** — Command-line format conversion between `.bytes` / `.yaml` / `.toml`
- **i18n** — Chinese and English UI, auto-detected from system locale
- **Cross-platform** — Native desktop (macOS / Windows / Linux) + WASM browser build

## Requirements

- Rust 1.85+ (edition 2024)
- For native: a GPU supporting Vulkan / Metal / DirectX 12 / OpenGL ES 3.1
- For WASM target checks/builds: `rustup target add wasm32-unknown-unknown`
- For WASM: [Trunk](https://trunkrs.dev) — `cargo install trunk`

## Getting Started

### Native

```bash
cargo run
```

Release build:

```bash
cargo build --release
```

The editor looks for extracted Unity content in a `unity_assets/` directory next to the executable when you want to keep a local extracted tree around. The repository no longer ships pre-extracted game assets under `unity_assets/`; editor-only icons and fonts still live under `editor_assets/`.

Builds embed Unity content into the editor crate. If `unity_assets/` exists locally, build.rs uses it. Otherwise, build.rs automatically downloads the pinned Bad Piggies 2.3.6 Windows Unity package from the BP-Innovation release, verifies its SHA-256, extracts only the runtime-needed files into a cache under `target/`, and embeds assets from there.

To regenerate a local `unity_assets/` tree from the local `../Bad-Piggies-2.3.6-Unity-Windows.unitypackage`, run:

```bash
python3 ../_extract_unitypackage_to_guid_layout.py
```

Use `python3 ../_extract_unitypackage_to_guid_layout.py --help` for alternate package/target paths or to keep a backup of the previous tree. The helper extracts only `asset`, `asset.meta`, and `pathname`, skips Unity preview images, and normalizes file modes so the rebuilt tree stays git-clean on macOS. The regenerated `unity_assets/` directory is ignored by git.

GitHub Actions forces the download/cache path with `BP_EDITOR_FETCH_UNITY_ASSETS=1`, but local builds no longer need that environment variable when `unity_assets/` is absent.

Advanced overrides:

```bash
# Use a pre-extracted asset tree instead of editor/unity_assets
BP_EDITOR_UNITY_ASSETS_DIR=/abs/path/to/unity_assets cargo check

# Force the fetch/cache path even if a local unity_assets/ tree exists
BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo check

# Exercise the fetch/cache path with a local unitypackage instead of downloading
BP_EDITOR_FETCH_UNITY_ASSETS=1 \
BP_EDITOR_UNITYPACKAGE_PATH=../Bad-Piggies-2.3.6-Unity-Windows.unitypackage \
cargo check
```

### WASM (browser)

```bash
trunk serve index.html
```

Then open `http://localhost:8080` in your browser.

## Development Checks

Native checks:

```bash
cargo test --message-format=short
cargo clippy --all-targets --message-format=short
```

WASM checks:

```bash
cargo check --target wasm32-unknown-unknown --message-format=short
cargo clippy --target wasm32-unknown-unknown --all-targets --message-format=short
```

For the remaining asset-migration blockers that are still intentionally kept as documented fallbacks, see [../ASSET_MIGRATION_BLOCKERS.md](../ASSET_MIGRATION_BLOCKERS.md).

## Usage

### GUI

1. **Open a level** — `File → Open Level File…` or drag-and-drop a `.bytes` / `.yaml` / `.toml` file onto the canvas
2. **Navigate** — Scroll wheel to zoom, drag on empty canvas to pan
3. **Select objects** — Click on any sprite or terrain in the canvas
4. **Edit properties** — Use the right-side properties panel to modify object fields and overrides
5. **Undo / Redo** — `Edit → Undo / Redo` or keyboard shortcuts
6. **Toggle panels** — `View → Object List / Properties / Physics Ground`
7. **Toggle background** — Press `B` or `View → Hide/Show Background`
8. **Export level** — `File → Export Level / Export as YAML / Export as TOML`
9. **Import text format** — `File → Import YAML/TOML…`
10. **Open a save file** — `File → Open Save File…` for `Progress.dat`, `*.contraption`, or `Achievements.xml`
11. **Edit save data** — Use the raw XML panel or the structured table view side-by-side
12. **Preview contraptions** — For `.contraption` saves, toggle `View → Contraption Preview`
13. **Export saves** — `File → Export Save` to re-encrypt, or `File → Export XML` for decrypted XML
14. **Import decrypted XML** — `File → Import XML…` and let the editor detect the save type from content
15. **Switch language** — `View → Switch to 中文 / English`

### CLI

The executable doubles as a command-line format converter:

```bash
# Convert binary to YAML
badpiggies-editor convert level.bytes level.yaml

# Convert YAML to TOML
badpiggies-editor convert level.yaml level.toml

# Convert TOML back to binary
badpiggies-editor convert level.toml level.bytes

# Decrypt a save file to XML
badpiggies-editor decrypt Progress.dat -o Progress.xml

# Encrypt edited XML back to a save file
badpiggies-editor encrypt Progress.xml Progress.dat

# Show help
badpiggies-editor --help
badpiggies-editor convert --help
badpiggies-editor decrypt --help
badpiggies-editor encrypt --help
```

CLI messages are localized based on system locale.

## Project Structure

```
editor/
├── src/
│   ├── main.rs              # Native + WASM entry point
│   ├── app/                 # egui app shell, menu/dialog/panel orchestration
│   │   ├── mod.rs
│   │   ├── app_loop.rs      # Main frame update loop and shortcuts
│   │   ├── canvas.rs        # Center canvas UI and renderer writeback
│   │   ├── dialogs/         # Tool/about/add-object/shortcut dialogs
│   │   ├── menu/            # File/Edit/View/Help menus
│   │   ├── properties/      # Object properties and override editors
│   │   ├── save_tables/     # Structured save-data table editors
│   │   ├── save_viewer/     # Save viewer UI + previews
│   │   └── tree/            # Object tree rendering and drag/drop
│   ├── data/                # Embedded asset/theme/sprite/icon lookup data
│   ├── diagnostics/         # Shared error/logging helpers
│   ├── domain/              # Level types, parser, terrain generation
│   │   ├── level/           # Level DB helpers
│   │   ├── parser.rs        # Binary .bytes parser / serializer
│   │   ├── types.rs         # Core level/object/terrain data structures
│   │   └── terrain_gen/     # Fill/stripe mesh generation utilities
│   ├── i18n/                # Fluent locale loading and language switching
│   ├── io/                  # Crypto and save-file parsing/export helpers
│   └── renderer/            # wgpu-backed scene rendering
│       ├── mod.rs           # Main render loop and renderer state
│       ├── background/      # Background cache, draw path, tests
│       ├── dark_overlay/    # Dark overlay interval/mesh parsing
│       ├── input/           # Camera, hit-test, interaction, terrain edit
│       ├── level_setup/     # Renderer cache rebuild when loading levels
│       ├── sprites/         # Sprite data extraction and drawing
│       ├── terrain.rs       # Terrain draw data construction
│       ├── fill_shader.rs   # Terrain fill shader
│       ├── edge_shader.rs   # Terrain edge shader
│       ├── sprite_shader.rs # Transparent sprite batching
│       ├── opaque_shader.rs # Opaque props batching
│       ├── bg_shader.rs     # Parallax background shader
│       ├── particles.rs     # Particle rendering helpers
│       └── grid.rs          # Editor grid overlay
├── editor_assets/       # Editor-only SVG icons and bundled fonts
│   ├── fonts/
│   └── ui/
├── unity_assets/        # Optional local GUID-keyed extracted Unity assets used by the runtime loaders
│   ├── <guid>/
│   │   ├── asset
│   │   ├── asset.meta
│   │   └── pathname
├── locales/             # Fluent translation files (embedded via include_str!)
│   ├── zh-CN.ftl        # Chinese (Simplified)
│   └── en-US.ftl        # English
├── index.html           # WASM host page
└── Cargo.toml
```

## Technical Details

### Rendering Pipeline

All rendering is done via `egui`'s `PaintCallback` mechanism, which lets custom wgpu render passes integrate seamlessly with the egui UI.

| Shader | Purpose |
|---|---|
| `fill_shader` | Terrain fill mesh (tiled texture, z = 0) |
| `edge_shader` | Terrain edge/curve mesh (alpha-blended, z = -0.01) |
| `sprite_shader` | Transparent sprites (batched, one pass per frame) |
| `opaque_shader` | Opaque Props sprites (pre-built atlas batch) |
| `bg_shader` | Parallax background layers |

### Level Binary Format

The `.bytes` format is a custom binary format used by the Unity game:

- Strings are length-prefixed UTF-8
- Positions are `f32` little-endian
- Colors are `u32` packed RGBA (not `float4`)
- Indices are `i16`
- Parent nodes store `childCount + name + position`
- Prefab instances store full `transform + data`

### Z-Ordering

- Pure Z-depth, no sorting layers
- Camera at Z = -15, orthographic projection
- Terrain fill at Z = 0, edge at Z = -0.01
- Smaller Z = closer to camera

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `B` | Toggle background display |
| `⌘Z` / `Ctrl+Z` | Undo |
| `⇧⌘Z` / `Ctrl+Y` | Redo |
| Scroll wheel | Zoom in / out |
| Drag (empty area) | Pan view |
| Click object | Select object |

## Acknowledgements

- [BP-Innovation/Bad-Piggies-Original](https://github.com/BP-Innovation/Bad-Piggies-Original) — Decompiled game source and asset references that made reverse-engineering the level format and rendering pipeline possible.

## License

This project is licensed under the **GNU Affero General Public License v3.0** — see the [LICENSE](../LICENSE) file for details.

> Game assets (textures, level data) are property of **Rovio Entertainment** and are not covered by this license. This project is for educational and reverse-engineering purposes only.
