# Bad Piggies Editor

A Rust-based level and save editor for **Bad Piggies**, built with `egui`, `eframe`, and `wgpu`.

It runs as a native desktop application on macOS, Windows, and Linux, and it also has a WASM build for browser-based testing and preview.

[![Rust](https://img.shields.io/badge/Rust-2024-orange)](https://www.rust-lang.org)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](../LICENSE)

## Highlights

- Visual editing for Bad Piggies level files, including object transforms, hierarchy, terrain, and overrides
- GPU-backed rendering for terrain fills, terrain edges, sprites, opaque props, parallax backgrounds, and dark overlay effects
- Import and export for binary `.bytes`, YAML, and TOML level formats
- Save-file viewer and editor for `Progress.dat`, `*.contraption`, and `Achievements.xml`
- Contraption preview support inside the save viewer
- English and Simplified Chinese UI with system-locale auto-detection
- Native desktop UI plus a WASM/browser target
- Built-in CLI for conversion, decryption, and re-encryption tasks

## Requirements

- Rust 1.85 or newer
- For native builds: a GPU/API stack that supports Vulkan, Metal, DirectX 12, or OpenGL ES 3.1 via `wgpu`
- For Linux native builds: the system libraries required by the `eframe`/`winit` stack, similar to the packages installed in CI
- For WASM builds: `rustup target add wasm32-unknown-unknown`
- For browser serving: [Trunk](https://trunkrs.dev), installed with `cargo install trunk`

## Quick Start

### Native desktop

```bash
cargo run
```

Release build:

```bash
cargo build --release
```

### WASM / browser

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve index.html
```

Then open `http://localhost:8080` in your browser.

## Assets And Build Inputs

The editor uses two different asset sources:

- `assets/` contains bundled editor resources such as icons, fonts, locales, and WGSL shader files
- `unity_assets/` contains extracted runtime data from the original game, keyed by Unity GUID

Bundled editor resources under `assets/` are compiled into the crate directly with `include_bytes!` and `include_str!`, so native and WASM builds do not require an external `assets/` directory at runtime.

During builds, the crate first looks for a local `unity_assets/` directory next to this crate's `Cargo.toml`, which means `editor/unity_assets/`. If that directory is present, `build.rs` embeds assets from it.

If `unity_assets/` is missing, `build.rs` automatically:

1. Downloads the pinned Bad Piggies 2.3.6 Windows Unity package
2. Verifies its SHA-256
3. Extracts only the runtime-needed files into `target/unity_asset_cache/`
4. Embeds those extracted files into the build

GitHub Actions forces that fetch path with `BP_EDITOR_FETCH_UNITY_ASSETS=1` and caches only the extracted Unity asset cache, not the full Rust build output.

To regenerate a local `unity_assets/` tree from a local Unity package:

```bash
python3 ../_extract_unitypackage_to_guid_layout.py
```

Use `python3 ../_extract_unitypackage_to_guid_layout.py --help` for alternate package and target paths.

`unity_assets/` is not ignored by git. If you want to keep a private extracted tree outside the repo, point the build to it with `BP_EDITOR_UNITY_ASSETS_DIR`.

### Asset-related environment variables

| Variable | Purpose |
|---|---|
| `BP_EDITOR_UNITY_ASSETS_DIR` | Use a pre-extracted asset tree outside `editor/unity_assets/` |
| `BP_EDITOR_FETCH_UNITY_ASSETS` | Force the download/extract/cache path even if `unity_assets/` exists |
| `BP_EDITOR_UNITYPACKAGE_PATH` | Use a local `.unitypackage` file instead of downloading |
| `BP_EDITOR_UNITYPACKAGE_URL` | Override the download URL |
| `BP_EDITOR_UNITYPACKAGE_SHA256` | Override the expected SHA-256 for the package |
| `BP_EDITOR_UNITY_ASSET_CACHE_DIR` | Override the base cache directory used by `build.rs` |

Example overrides:

```bash
BP_EDITOR_UNITY_ASSETS_DIR=/abs/path/to/unity_assets cargo check

BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo check

BP_EDITOR_FETCH_UNITY_ASSETS=1 \
BP_EDITOR_UNITYPACKAGE_PATH=../Bad-Piggies-2.3.6-Unity-Windows.unitypackage \
cargo check
```

## What The Editor Can Do

### Levels

- Open `.bytes`, `.yaml`, `.yml`, and `.toml` level files
- Pan, zoom, and inspect the scene visually
- Select objects and edit their properties and overrides
- Edit terrain data and terrain-generated geometry
- Export levels back to binary, YAML, or TOML

### Saves

- Open `Progress.dat`, `*.contraption`, and `Achievements.xml`
- View and edit save content as raw XML or structured tables
- Re-export save data as encrypted game files or plain XML
- Preview contraptions from save files directly inside the editor

### Interface

- Toggle editor panels from the View menu
- Switch UI language between English and Simplified Chinese
- Toggle background rendering with the `B` shortcut
- Use snapshot-based undo/redo across editing operations

## CLI

The binary also works as a command-line tool. The installed executable name is `badpiggies-editor`; from the repo you can use the same commands via `cargo run -- ...`.

Examples:

```bash
# Convert a level from binary to YAML
badpiggies-editor convert level.bytes level.yaml

# Convert a level from YAML to TOML
badpiggies-editor convert level.yaml level.toml

# Convert a level from TOML back to binary
badpiggies-editor convert level.toml level.bytes

# Decrypt a save file to XML
badpiggies-editor decrypt Progress.dat -o Progress.xml

# Encrypt edited XML back to a game save
badpiggies-editor encrypt Progress.xml Progress.dat

# Show help
badpiggies-editor --help
badpiggies-editor convert --help
badpiggies-editor decrypt --help
badpiggies-editor encrypt --help
```

CLI messages are localized from the system locale in the same way as the GUI.

## Development And Validation

Quick local checks:

```bash
cargo test --message-format=short
cargo check --all-targets --message-format=short
cargo clippy --all-targets --message-format=short -- -D warnings
cargo check --target wasm32-unknown-unknown --message-format=short
cargo clippy --target wasm32-unknown-unknown --all-targets --message-format=short -- -D warnings
```

CI-equivalent validation:

```bash
BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo check --all-targets
BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo clippy --all-targets -- -D warnings
BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo check --target wasm32-unknown-unknown
BP_EDITOR_FETCH_UNITY_ASSETS=1 cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings
```

Optional fixture-backed parser roundtrip test:

```bash
cargo test test_level_roundtrip
```

That test looks for `../test_levels/assetbundles/episode_1_levels.unity3d/Level_05_data.bytes`. If the fixture is absent, the test skips itself instead of breaking CI compilation.

For the remaining asset-migration blockers that are intentionally still documented, see [../ASSET_MIGRATION_BLOCKERS.md](../ASSET_MIGRATION_BLOCKERS.md).

## Repository Layout

```text
editor/
├── assets/              # Bundled editor-only resources: fonts, locales, WGSL shaders, UI icons
├── src/
│   ├── app/             # egui application shell, menus, dialogs, panels, save viewer
│   ├── data/            # Embedded databases and asset lookup data
│   ├── diagnostics/     # Error handling and logging helpers
│   ├── domain/          # Level types, parser/serializer, terrain generation
│   ├── i18n/            # Fluent localization
│   ├── io/              # Save crypto and import/export helpers
│   ├── renderer/        # wgpu-backed scene rendering
│   ├── unity_runtime/   # Unity runtime data adapters
│   └── main.rs          # Native entry point, WASM entry point, CLI wiring
├── unity_assets/        # Optional local extracted runtime assets
├── build.rs             # Unity asset resolution, fetch, extraction, embed generation
├── index.html           # Trunk host page for the WASM build
└── Cargo.toml
```

## Rendering Notes

Rendering is integrated into `egui` via `PaintCallback`, so editor UI and custom `wgpu` passes share the same frame.

Core runtime shader groups:

| Shader module | Purpose |
|---|---|
| `fill_shader` | Terrain fill meshes with tiled textures |
| `edge_shader` | Terrain edge and curve rendering |
| `sprite_shader` | Transparent sprite batching |
| `opaque_shader` | Opaque prop batching |
| `bg_shader` | Parallax background layers |
| `dark_mask_shader` | Dark overlay and lit-area masking |

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `B` | Toggle background display |
| `⌘Z` / `Ctrl+Z` | Undo |
| `⇧⌘Z` / `Ctrl+Y` | Redo |
| Scroll wheel | Zoom in or out |
| Drag on empty canvas | Pan the camera |
| Click object | Select an object |

## Acknowledgements

- [BP-Innovation/Bad-Piggies-Original](https://github.com/BP-Innovation/Bad-Piggies-Original) for the decompiled project and asset references that made the level and rendering work possible

## License

This project is licensed under the **GNU Affero General Public License v3.0**. See [../LICENSE](../LICENSE) for details.

Game assets, textures, and level data remain the property of **Rovio Entertainment** and are not covered by this repository's license.
