use badpiggies_editor_core::data::runtime_assets::install_runtime_assets;

pub const REQUIRED_RUNTIME_ASSETS: &[&str] = &[
    "data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage",
    "ui/app-icon.png",
    "locales/ar.ftl",
    "locales/zh-CN.ftl",
    "locales/en-US.ftl",
    "locales/es-ES.ftl",
    "locales/fr-FR.ftl",
    "locales/ru-RU.ftl",
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
const RUNTIME_ASSETS_DIR_ENV: &str = "BP_EDITOR_RUNTIME_ASSETS_DIR";
#[cfg(not(target_arch = "wasm32"))]
const EXTERNAL_UNITYPACKAGE_PATH_ENV: &str = "BP_EDITOR_EXTERNAL_UNITYPACKAGE_PATH";

#[cfg(not(target_arch = "wasm32"))]
fn runtime_asset_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Some(configured) = std::env::var_os(RUNTIME_ASSETS_DIR_ENV) {
        candidates.push(std::path::PathBuf::from(configured));
    }
    candidates.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("app/assets"));
        candidates.push(cwd.join("assets"));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join("assets"));
        #[cfg(target_os = "macos")]
        if let Some(contents) = directory.parent() {
            candidates.push(contents.join("Resources/assets"));
        }
    }
    candidates.dedup();
    candidates
}

#[cfg(not(target_arch = "wasm32"))]
fn package_override(relative: &str) -> Option<std::path::PathBuf> {
    (relative == "data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage")
        .then(|| std::env::var_os(EXTERNAL_UNITYPACKAGE_PATH_ENV).map(std::path::PathBuf::from))
        .flatten()
}

#[cfg(not(target_arch = "wasm32"))]
fn has_required_assets(root: &std::path::Path) -> bool {
    REQUIRED_RUNTIME_ASSETS.iter().all(|relative| {
        package_override(relative)
            .map_or_else(|| root.join(relative).is_file(), |path| path.is_file())
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn runtime_assets_root() -> Result<std::path::PathBuf, String> {
    let candidates = runtime_asset_candidates();
    candidates
        .iter()
        .find(|candidate| candidate.is_dir() && has_required_assets(candidate))
        .cloned()
        .ok_or_else(|| {
            format!(
                "runtime assets directory not found; tried:\n{}",
                candidates
                    .iter()
                    .map(|path| format!("  - {}", path.display()))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn runtime_asset_path(root: &std::path::Path, relative: &str) -> std::path::PathBuf {
    package_override(relative).unwrap_or_else(|| root.join(relative))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn preload_required_runtime_assets() -> Result<(), String> {
    let root = runtime_assets_root()?;

    let assets = REQUIRED_RUNTIME_ASSETS
        .iter()
        .map(|relative| {
            let path = runtime_asset_path(&root, relative);
            std::fs::read(&path)
                .map(|bytes| ((*relative).to_string(), bytes))
                .map_err(|error| format!("failed to read {}: {error}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    install_runtime_assets(assets);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub async fn preload_required_runtime_assets_with_progress<F>(
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(usize, usize, &str),
{
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or_else(|| "browser window is unavailable".to_string())?;
    let asset_root = crate::app_view::APP_ASSETS.to_string();
    let asset_root = asset_root.trim_end_matches('/');
    let requests = REQUIRED_RUNTIME_ASSETS
        .iter()
        .map(|relative| {
            let url = format!("{asset_root}/{relative}");
            let request = window.fetch_with_str(&url);
            (*relative, request)
        })
        .collect::<Vec<_>>();
    let mut assets = Vec::with_capacity(requests.len());
    let mut failures = Vec::new();
    let total = requests.len();
    let mut loaded = 0usize;

    on_progress(loaded, total, "");

    for (relative, request) in requests {
        on_progress(loaded, total, relative);
        let result = async {
            let response = JsFuture::from(request)
                .await
                .map_err(|error| format!("fetch failed: {error:?}"))?
                .dyn_into::<web_sys::Response>()
                .map_err(|_| "invalid fetch response".to_string())?;
            if !response.ok() {
                return Err(format!(
                    "HTTP {} {}",
                    response.status(),
                    response.status_text()
                ));
            }
            let buffer = JsFuture::from(response.array_buffer().map_err(|e| format!("{e:?}"))?)
                .await
                .map_err(|error| format!("body read failed: {error:?}"))?;
            Ok(js_sys::Uint8Array::new(&buffer).to_vec())
        }
        .await;

        match result {
            Ok(bytes) => {
                assets.push((relative.to_string(), bytes));
                loaded += 1;
                on_progress(loaded, total, "");
            }
            Err(error) => failures.push(format!("{relative}: {error}")),
        }
    }

    if !failures.is_empty() {
        return Err(format!(
            "runtime asset preload failed:\n{}",
            failures.join("\n")
        ));
    }
    install_runtime_assets(assets);
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::REQUIRED_RUNTIME_ASSETS;

    #[test]
    fn manifest_asset_directory_is_complete() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let missing = REQUIRED_RUNTIME_ASSETS
            .iter()
            .filter(|relative| !root.join(relative).is_file())
            .collect::<Vec<_>>();
        assert!(missing.is_empty(), "missing runtime assets: {missing:?}");
    }
}
