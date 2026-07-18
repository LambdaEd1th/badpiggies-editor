# Bad Piggies Editor

A cross-platform level and save editor for **Bad Piggies**, written in Rust 2024.
The same Dioxus application runs in a native system WebView and in the browser.
Level rendering runs directly on a raw wgpu canvas surface with no GUI toolkit.

[![Rust](https://img.shields.io/badge/Rust-2024-orange)](https://www.rust-lang.org)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](LICENSE)

## Features

| Category | Details |
|---|---|
| Level editing | Open and export `.bytes`, `.yaml`, `.yml`, and `.toml`; edit the object tree, transforms, terrain, and hierarchy |
| Save editing | Decrypt, edit, and re-encrypt `Progress.dat`, `*.contraption`, and `Achievements.xml` |
| Rendering | Original six-pass wgpu renderer with textured terrain, sprites, compounds, backgrounds, dark masks, particles, selection, transform handles, pan, zoom, and fit-to-view |
| Web workers | A stable-WASM Processing Worker pool handles CPU work while a dedicated Render Worker owns the canvas and GPU |
| CLI | Convert level formats and encrypt or decrypt save files |
| Localization | Fluent locale files discovered from `app/assets/locales` |

## Workspace Architecture

```text
editor/
  app/                    Dioxus UI, editor state, platform adapters, CLI, assets
  crates/core/            UI-free domain, parser, crypto, Unity data, worker protocol
  crates/renderer/        Pure-wgpu renderer for HTMLCanvasElement or OffscreenCanvas
  crates/worker/          Persistent processing Worker bindings around core operations
  scripts/                Renderer/worker build and Dioxus development server scripts
  Dioxus.toml             Dioxus Web/Desktop configuration
  Cargo.toml              Cargo Workspace manifest
```

The dependency direction is intentionally one-way:

```text
badpiggies-editor-app ----> badpiggies_editor_core
badpiggies_editor_renderer -> badpiggies_editor_core
badpiggies_editor_worker -> badpiggies_editor_core
```

`core` does not depend on Dioxus, Web APIs, file dialogs, or localization. The
app loads runtime assets from the filesystem or HTTP and installs their bytes in
the core resource registry before parsing begins.

Native builds use Rayon for processing requests, resource database construction,
Unity bundle work, terrain generation, text search, and renderer preparation.
The WASM target does not link Rayon or `wasm-bindgen-rayon`.

On the Web, Dioxus and DOM input remain on the main thread. A dedicated Render
Worker owns the transferred OffscreenCanvas, wgpu device, resources, and frame
loop; scene revisions send full level data while selection and view controls use
smaller incremental messages. Browsers without Worker WebGPU/OffscreenCanvas
support automatically use the same renderer on the main-thread canvas. A pool
of independent Processing Workers routes requests by ID and handles level
parsing/serialization, save crypto/XML parsing, Unity TextAsset operations,
batch jobs, and large-text search. Each Worker runs an isolated single-threaded
WASM instance built with stable Rust, while independent requests can run on
different browser cores.

## Requirements

- Rust stable with the `wasm32-unknown-unknown` target
- Dioxus CLI 0.7.9
- wasm-pack 0.15.0 for the Web Worker and wgpu renderer
- Native WebView development libraries required by Dioxus Desktop

```bash
rustup toolchain install stable --profile minimal --component clippy,rustfmt --target wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.9 --locked
cargo install wasm-pack --version 0.15.0 --locked
```

Linux additionally requires GTK, WebKitGTK, xkbcommon, and Wayland/X11
development packages.

## Run

Native desktop:

```bash
cargo run --package badpiggies-editor-app --bin badpiggies-editor
```

Web development server:

```bash
./scripts/serve-web.sh --port 8080 --open false
```

The script builds stable single-thread WASM packages for the Processing and
Render Workers before starting Dioxus. SharedArrayBuffer and cross-origin
isolation headers are not required.

At runtime the Processing pool uses `navigator.hardwareConcurrency` to create
between one and four independent Workers. The selected backends and worker counts are exposed as
`data-processing-worker-backend`, `data-processing-worker-threads`,
`data-render-worker-backend`, and `data-render-worker-threads` attributes on
the document root.

Release Web build:

```bash
./scripts/build-web-worker.sh
./scripts/build-web-renderer.sh
dx build --platform web --package badpiggies-editor-app --release --debug-symbols=false
```

Output is written to:

```text
target/dx/badpiggies-editor/release/web/public
```

Production hosting only needs to serve JavaScript and WebAssembly files with
their correct MIME types. GitHub Pages can run the same Worker pool build.

## CLI

The native binary also exposes command-line operations:

```bash
badpiggies-editor convert level.bytes level.yaml
badpiggies-editor convert level.yaml level.toml
badpiggies-editor convert level.toml level.bytes

badpiggies-editor decrypt Progress.dat -o Progress.xml
badpiggies-editor encrypt Progress.xml Progress.dat
badpiggies-editor --help
```

From the Workspace, pass CLI arguments after `--`:

```bash
cargo run --package badpiggies-editor-app -- convert level.bytes level.yaml
```

## Runtime Assets

Runtime resources are stored under `app/assets/`:

```text
app/assets/
  data/       bundled Unity package
  locales/    Fluent translations
  shader/     retained game shader resources used by asset databases
  ui/         icons
  renderer/   Render Worker wrapper and generated pure-wgpu WASM package
  worker/     Processing Worker wrapper and generated WASM package
```

Both Worker folders contain a generated `pkg/` directory built with stable
Rust. The build scripts remove obsolete `threaded/` packages automatically.

Native lookup checks `app/assets`, executable-adjacent `assets`, macOS bundle
resources, and these environment overrides:

| Variable | Effect |
|---|---|
| `BP_EDITOR_RUNTIME_ASSETS_DIR` | Complete runtime asset directory |
| `BP_EDITOR_EXTERNAL_UNITYPACKAGE_PATH` | Replacement Unity `.unitypackage` file |

Web builds copy the asset folders through Dioxus `AssetOptions::folder()` and
load them under `assets/`. The generated worker and renderer packages must be
rebuilt with `scripts/build-web-worker.sh` and `scripts/build-web-renderer.sh`.

## Checks

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo check --workspace --target wasm32-unknown-unknown
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --target wasm32-unknown-unknown --all-targets -- -D warnings
./scripts/build-web-worker.sh
./scripts/build-web-renderer.sh
dx build --platform web --package badpiggies-editor-app --release --debug-symbols=false
```

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `Cmd/Ctrl+Z` | Undo |
| `Cmd/Ctrl+Shift+Z` | Redo |
| `Cmd/Ctrl+C/X/V` | Copy, cut, paste |
| `Cmd/Ctrl+D` | Duplicate selection |
| `Delete` / `Backspace` | Delete selection |
| `F` | Fit view |
| Scroll wheel | Zoom |
| Middle/right/Alt/Space + drag | Pan |

## License

Licensed under the GNU Affero General Public License v3.0. Game assets remain
the property of Rovio Entertainment and are not covered by this license.
