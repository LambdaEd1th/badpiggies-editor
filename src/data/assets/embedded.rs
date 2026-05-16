//! Project asset loading via rust-embed.

use std::borrow::Cow;

/// Project assets exposed through rust-embed on all targets.
#[derive(rust_embed::RustEmbed)]
#[folder = "unity_assets/"]
pub struct ProjectAssets;

/// Read asset bytes by relative path (e.g. "sprites/IngameAtlas.png").
pub fn read_asset(key: &str) -> Option<Cow<'static, [u8]>> {
    if let Some(text) = generated_asset_text(key) {
        return Some(Cow::Owned(text.into_bytes()));
    }

    let mapped = map_asset_key(key)?;
    read_project_asset(mapped.as_ref())
}

pub fn read_asset_text(key: &str) -> Option<String> {
    read_asset(key).map(|bytes| String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

pub fn list_asset_paths(prefix: &str, suffix: &str) -> Vec<String> {
    let mut files: Vec<String> = ProjectAssets::iter()
        .filter_map(|path| {
            let path = path.as_ref();
            path.strip_prefix(prefix)
                .filter(|path| path.ends_with(suffix))
                .map(str::to_string)
        })
        .collect();
    files.sort();
    files
}

fn generated_asset_text(key: &str) -> Option<String> {
    match key {
        "unity/prefabs/manifest.txt" => Some(prefab_manifest_text()),
        "unity/levels/loader-manifest.txt" => Some(loader_manifest_text()),
        _ => None,
    }
}

fn map_asset_key(key: &str) -> Option<Cow<'static, str>> {
    if key.starts_with("Texture2D/")
        || key.starts_with("Resources/")
        || key.starts_with("Prefab/")
        || key.starts_with("AnimationClip/")
    {
        return Some(Cow::Owned(key.to_string()));
    }

    if let Some(filename) = key
        .strip_prefix("sprites/")
        .or_else(|| key.strip_prefix("props/"))
        .or_else(|| key.strip_prefix("bg/"))
        .or_else(|| key.strip_prefix("sky/"))
        .or_else(|| key.strip_prefix("ground/"))
        .or_else(|| key.strip_prefix("particles/"))
    {
        return Some(Cow::Owned(format!("Texture2D/{filename}")));
    }

    if let Some(filename) = key.strip_prefix("unity/animation/") {
        return Some(Cow::Owned(format!("AnimationClip/{filename}")));
    }

    if let Some(path) = key.strip_prefix("unity/background/") {
        return Some(Cow::Owned(format!(
            "Resources/environment/background/{path}"
        )));
    }

    if let Some(path) = key.strip_prefix("unity/levels/") {
        return Some(Cow::Owned(format!("Resources/levels/{path}")));
    }

    if let Some(path) = key.strip_prefix("unity/prefabs/") {
        return Some(Cow::Owned(format!("Prefab/{path}")));
    }

    match key {
        "unity/resources/textureloader.prefab" => {
            Some(Cow::Borrowed("Resources/prefabs/textureloader.prefab"))
        }
        "unity/resources/guisystem/Sprites.bytes" => {
            Some(Cow::Borrowed("Resources/guisystem/sprites.bytes"))
        }
        _ => key
            .strip_prefix("unity/resources/")
            .map(|path| Cow::Owned(format!("Resources/{path}"))),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_project_asset(path: &str) -> Option<Cow<'static, [u8]>> {
    ProjectAssets::get(path).map(|f| f.data)
}

fn prefab_manifest_text() -> String {
    list_asset_paths("Prefab/", ".prefab").join("\n")
}

fn loader_manifest_text() -> String {
    list_asset_paths("Resources/levels/", "_loader.prefab").join("\n")
}
