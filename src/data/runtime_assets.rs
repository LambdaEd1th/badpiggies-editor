use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(not(target_arch = "wasm32"))]
const RUNTIME_ASSETS_DIR_ENV: &str = "BP_EDITOR_RUNTIME_ASSETS_DIR";

const REQUIRED_RUNTIME_ASSETS: &[&str] = &[
    "ui/app-icon.png",
    "ui/tool-select.svg",
    "ui/tool-box-select.svg",
    "ui/tool-draw-terrain.svg",
    "ui/tool-pan.svg",
    "ui/tool-terrain-ellipse.svg",
    "ui/tool-terrain-rectangle.svg",
    "ui/tool-terrain-perfect-circle.svg",
    "ui/tool-terrain-square.svg",
    "ui/tool-terrain-equilateral-triangle.svg",
    "ui/tool-preview-build.svg",
    "ui/tool-preview-play.svg",
    "ui/tool-preview-pause.svg",
    "ui/drop-icon.svg",
    "fonts/NotoSansCJKsc-Regular.otf",
    "locales/zh-CN.ftl",
    "locales/en-US.ftl",
    "shader/e2d__curve.wgsl",
    "shader/_custom__unlit_color_geometry__terrain_fill.wgsl",
    "shader/unlit__transparent_cutout__sprite.wgsl",
    "shader/_custom__unlit_colortransparent_geometry__sprite.wgsl",
    "shader/_custom__unlit_monochrome.wgsl",
    "shader/_custom__unlit_color_geometry.wgsl",
    "shader/_custom__unlit_colortransparent_geometry.wgsl",
    "shader/_custom__unlit_alpha8bit_color.wgsl",
    "shader/unlit__transparent.wgsl",
    "shader/unlit__transparent_cutout.wgsl",
    "shader/depth_mask__unlit_transparent_cg__runtime.wgsl",
    "shader/depth_mask__maskoverlay__runtime.wgsl",
    "shader/depth_mask__maskoverlaynv__runtime.wgsl",
];

#[cfg(not(target_arch = "wasm32"))]
fn has_required_runtime_assets(root: &Path) -> bool {
    REQUIRED_RUNTIME_ASSETS
        .iter()
        .all(|relative| root.join(relative).is_file())
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime_asset_candidates(cwd: &Path, exe_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    candidates.push(cwd.join("assets"));

    for exe_path in exe_paths {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("assets"));

            #[cfg(target_os = "macos")]
            {
                if let Some(contents_dir) = exe_dir.parent() {
                    candidates.push(contents_dir.join("Resources/assets"));
                }
            }
        }
    }

    let mut deduped = Vec::new();
    for candidate in candidates {
        if !deduped.contains(&candidate) {
            deduped.push(candidate);
        }
    }

    deduped
}

#[cfg(not(target_arch = "wasm32"))]
fn executable_path_candidates(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(path) = env::current_exe() {
        paths.push(path);
    }

    if let Some(arg0) = env::args_os().next() {
        let arg0_path = PathBuf::from(arg0);
        let normalized = if arg0_path.is_absolute() {
            arg0_path
        } else {
            cwd.join(arg0_path)
        };
        if !paths.contains(&normalized) {
            paths.push(normalized);
        }
    }

    paths
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime_assets_root() -> &'static PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        if let Some(value) = env::var_os(RUNTIME_ASSETS_DIR_ENV) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                assert_required_runtime_assets_present(&path);
                return path;
            }
            panic!(
                "{} points to a non-directory path: {}",
                RUNTIME_ASSETS_DIR_ENV,
                path.display()
            );
        }

        let cwd = env::current_dir().expect("failed to read current working directory");
        let exe_paths = executable_path_candidates(&cwd);
        let candidates = runtime_asset_candidates(&cwd, &exe_paths);
        for candidate in &candidates {
            if candidate.is_dir() && has_required_runtime_assets(candidate) {
                return candidate.clone();
            }
        }

        let details = candidates
            .iter()
            .map(|path| format!("  - {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "Runtime assets directory not found. Tried:\n{}\nSet {} to a complete editor/assets directory.",
            details,
            RUNTIME_ASSETS_DIR_ENV,
        );
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn assert_required_runtime_assets_present(root: &Path) {
    let mut missing = Vec::new();
    for relative in REQUIRED_RUNTIME_ASSETS {
        let path = root.join(relative);
        if !path.is_file() {
            missing.push(relative.to_string());
        }
    }

    if missing.is_empty() {
        return;
    }

    missing.sort();
    let details = missing
        .into_iter()
        .map(|p| format!("  - {}", p))
        .collect::<Vec<_>>()
        .join("\n");

    panic!(
        "Missing runtime assets under {}:\n{}\nSet {} to a complete editor/assets directory.",
        root.display(),
        details,
        RUNTIME_ASSETS_DIR_ENV
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn read_runtime_asset_bytes_uncached(relative_path: &str) -> Vec<u8> {
    let path = runtime_assets_root().join(relative_path);
    fs::read(&path).unwrap_or_else(|error| {
        panic!("Failed to read runtime asset {}: {}", path.display(), error)
    })
}

pub fn read_runtime_asset_bytes(relative_path: &str) -> Vec<u8> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

        if let Some(bytes) = cache
            .lock()
            .expect("runtime asset cache lock poisoned")
            .get(relative_path)
            .cloned()
        {
            return bytes;
        }

        let bytes = read_runtime_asset_bytes_uncached(relative_path);
        cache
            .lock()
            .expect("runtime asset cache lock poisoned")
            .insert(relative_path.to_string(), bytes.clone());
        bytes
    }

    #[cfg(target_arch = "wasm32")]
    {
        let cache = wasm_runtime_assets_cache();
        if let Some(bytes) = cache
            .lock()
            .expect("runtime asset cache lock poisoned")
            .get(relative_path)
            .cloned()
        {
            return bytes;
        }

        panic!(
            "Runtime asset is not preloaded in wasm: {}. Call preload_required_runtime_assets_wasm() before app startup.",
            relative_path
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn wasm_runtime_assets_cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_arch = "wasm32")]
fn wasm_runtime_assets_base_url() -> String {
    let Some(window) = web_sys::window() else {
        return "assets".to_string();
    };
    let Some(document) = window.document() else {
        return "assets".to_string();
    };

    document
        .query_selector("meta[name='bp-runtime-assets-base']")
        .ok()
        .flatten()
        .and_then(|el| el.get_attribute("content"))
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "assets".to_string())
}

#[cfg(target_arch = "wasm32")]
pub async fn preload_required_runtime_assets_wasm() -> Result<(), String> {
    use wasm_bindgen_futures::JsFuture;

    let Some(window) = web_sys::window() else {
        return Err("无法获取 browser window".to_string());
    };

    let base = wasm_runtime_assets_base_url();
    let mut failures = Vec::new();

    for relative in REQUIRED_RUNTIME_ASSETS {
        let url = format!("{}/{}", base.trim_end_matches('/'), relative);
        let response = match JsFuture::from(window.fetch_with_str(&url)).await {
            Ok(value) => match value.dyn_into::<web_sys::Response>() {
                Ok(response) => response,
                Err(_) => {
                    failures.push(format!("{}: invalid fetch response", relative));
                    continue;
                }
            },
            Err(error) => {
                failures.push(format!("{}: fetch failed ({:?})", relative, error));
                continue;
            }
        };

        if !response.ok() {
            failures.push(format!(
                "{}: HTTP {} {}",
                relative,
                response.status(),
                response.status_text()
            ));
            continue;
        }

        let array_buffer_promise = match response.array_buffer() {
            Ok(promise) => promise,
            Err(error) => {
                failures.push(format!("{}: array_buffer() failed ({:?})", relative, error));
                continue;
            }
        };

        let array_buffer = match JsFuture::from(array_buffer_promise).await {
            Ok(buffer) => buffer,
            Err(error) => {
                failures.push(format!(
                    "{}: failed to read response body ({:?})",
                    relative, error
                ));
                continue;
            }
        };

        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        wasm_runtime_assets_cache()
            .lock()
            .expect("runtime asset cache lock poisoned")
            .insert((*relative).to_string(), bytes);
    }

    if failures.is_empty() {
        Ok(())
    } else {
        failures.sort();
        Err(format!(
            "WASM runtime assets missing or unreadable from base '{}':\n{}",
            base,
            failures
                .into_iter()
                .map(|item| format!("  - {}", item))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

pub fn read_runtime_asset_text(relative_path: &str) -> String {
    let bytes = read_runtime_asset_bytes(relative_path);
    String::from_utf8(bytes).unwrap_or_else(|error| {
        panic!(
            "Runtime asset is not valid UTF-8 ({}): {}",
            relative_path, error
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_runtime_asset_paths(
    root: &Path,
    current: &Path,
    out: &mut Vec<String>,
    prefix: &str,
    suffix: &str,
) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_runtime_asset_paths(root, &path, out, prefix, suffix);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if relative.starts_with(prefix) && relative.ends_with(suffix) {
            out.push(relative);
        }
    }
}

pub fn list_runtime_assets(prefix: &str, suffix: &str) -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let root = runtime_assets_root();
        let mut out = Vec::new();
        collect_runtime_asset_paths(root, root, &mut out, prefix, suffix);
        out.sort();
        out
    }

    #[cfg(target_arch = "wasm32")]
    {
        let cache = wasm_runtime_assets_cache();
        let cache = cache.lock().expect("runtime asset cache lock poisoned");
        let mut out: Vec<String> = cache
            .keys()
            .filter(|k| k.starts_with(prefix) && k.ends_with(suffix))
            .cloned()
            .collect();
        out.sort();
        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn missing_runtime_assets() -> Vec<String> {
    let root = runtime_assets_root();
    let mut missing = Vec::new();
    for relative in REQUIRED_RUNTIME_ASSETS {
        if !root.join(relative).is_file() {
            missing.push(relative.to_string());
        }
    }
    missing
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::runtime_asset_candidates;
    use std::path::{Path, PathBuf};

    #[test]
    fn runtime_asset_candidates_include_cwd_and_exe_paths() {
        let cwd = Path::new("/tmp/editor-workdir");
        let exe_paths = vec![PathBuf::from("/tmp/editor-workdir/target/release/badpiggies-editor")];
        let candidates = runtime_asset_candidates(cwd, &exe_paths);

        assert!(
            candidates.contains(&PathBuf::from("/tmp/editor-workdir/assets")),
            "expected cwd/assets candidate"
        );
        assert!(
            candidates.contains(&PathBuf::from("/tmp/editor-workdir/target/release/assets")),
            "expected executable sibling assets candidate"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn runtime_asset_candidates_include_app_resources_on_macos() {
        let cwd = Path::new("/tmp/editor-workdir");
        let exe_paths = vec![PathBuf::from(
            "/Applications/Bad Piggies Editor.app/Contents/MacOS/badpiggies-editor",
        )];
        let candidates = runtime_asset_candidates(cwd, &exe_paths);

        assert!(
            candidates.contains(&PathBuf::from(
                "/Applications/Bad Piggies Editor.app/Contents/Resources/assets"
            )),
            "expected .app Resources/assets candidate"
        );
    }
}
