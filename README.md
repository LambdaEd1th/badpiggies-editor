# Bad Piggies Editor

A native desktop (and WASM) level editor for the **Bad Piggies** mobile game, built with Rust and GPU-accelerated rendering.

[![Rust](https://img.shields.io/badge/Rust-2024-orange)](https://www.rust-lang.org)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](../LICENSE)

## Features

- **Level parsing** — Full binary `.bytes` level format parser, mirroring the original Unity `LevelLoader.cs`
- **GPU rendering** — Custom WGSL shaders via [wgpu](https://wgpu.rs) for all visual layers
- **Terrain** — Fill meshes + edge/curve meshes with splat-mapped textures
- **Backgrounds** — 10 parallax themes (Jungle, Forest, Plateau, Night, Halloween, MayaCave, …)
- **Sprites** — Atlas-based rendering with `RuntimeSpriteDatabase` and `CartoonFrameSprite` support
- **Compound objects** — Multi-part objects (fans, motors, hinges, boxes, …) with sub-sprite composition
- **Performance** — Frustum culling + GPU draw call batching (one render pass per shader type per frame)
- **Properties panel** — Editable object properties with override tree
- **Cross-platform** — Native desktop (macOS / Windows / Linux) + WASM browser build

## Requirements

- Rust 1.85+ (edition 2024)
- For native: a GPU supporting Vulkan / Metal / DirectX 12 / OpenGL ES 3.1
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

The editor looks for game asset textures in an `assets/` directory next to the executable. The repository ships with pre-extracted textures under `assets/`.

### WASM (browser)

```bash
trunk serve
```

Then open `http://localhost:8080` in your browser.

## Usage

1. **Open a level** — `File → Open Level File…` or drag-and-drop a `.bytes` file onto the canvas
2. **Navigate** — Scroll wheel to zoom, drag on empty canvas to pan
3. **Select objects** — Click on any sprite or terrain in the canvas
4. **Edit properties** — Use the right-side properties panel to modify object fields and overrides
5. **Toggle panels** — `View → Object List / Properties / Physics Ground`
6. **Toggle background** — Press `B` or `View → Hide/Show Background`
7. **Export level** — `File → Export Level`
8. **Switch language** — `View → Switch to 中文 / English`

## Project Structure

```
editor/
├── src/
│   ├── main.rs          # Entry point (native + WASM)
│   ├── app.rs           # egui application, menus, panels, dialogs
│   ├── locale.rs        # i18n via fluent-bundle (zh-CN / en-US)
│   ├── parser.rs        # Binary .bytes level file parser / serializer
│   ├── types.rs         # Data structures (LevelData, TerrainData, PrefabInstance, …)
│   ├── assets.rs        # Embedded asset loader (rust-embed)
│   ├── sprite_db.rs     # Runtime sprite database
│   ├── bg_data.rs       # Background theme data
│   ├── level_refs.rs    # Level object reference tables
│   └── renderer/
│       ├── mod.rs           # Main render loop, frustum culling, batch collection
│       ├── terrain.rs       # Terrain mesh generation
│       ├── fill_shader.rs   # Terrain fill wgpu shader + texture cache
│       ├── edge_shader.rs   # Terrain edge/curve wgpu shader
│       ├── sprite_shader.rs # Transparent sprite batch shader
│       ├── opaque_shader.rs # Opaque (Props) sprite batch shader
│       ├── bg_shader.rs     # Parallax background wgpu shader
│       ├── background.rs    # Background layer rendering logic
│       ├── compounds.rs     # Compound object sub-sprite definitions
│       ├── sprites.rs       # Sprite atlas GPU resource management
│       └── grid.rs          # Editor grid overlay
├── assets/              # Game assets — all embedded at compile time (rust-embed)
│   ├── bg/              # Background layer textures
│   ├── ground/          # Terrain fill textures
│   ├── props/           # Props atlas textures
│   ├── sky/             # Sky textures
│   ├── sprites/         # Sprite atlas textures
│   ├── particles/       # Particle textures
│   ├── bg-data.json     # Background theme metadata
│   ├── sprite-data.json # Sprite atlas UV / sizing data
│   └── level-refs.json  # Level object reference tables
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
| Scroll wheel | Zoom in / out |
| Drag (empty area) | Pan view |
| Click object | Select object |

## Acknowledgements

- [BP-Innovation/Bad-Piggies-Original](https://github.com/BP-Innovation/Bad-Piggies-Original) — Decompiled game source and asset references that made reverse-engineering the level format and rendering pipeline possible.

## License

This project is licensed under the **GNU General Public License v3.0** — see the [LICENSE](../LICENSE) file for details.

> Game assets (textures, level data) are property of **Rovio Entertainment** and are not covered by this license. This project is for educational and reverse-engineering purposes only.
