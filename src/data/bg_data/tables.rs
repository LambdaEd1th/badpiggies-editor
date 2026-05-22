use super::types::BgLayer;
use std::path::Path;
use std::sync::OnceLock;

pub(super) const BG_SPRITE_SCRIPT_GUID: &str = "b011dfa16a4475b746a1372ea41fdf05";
pub(super) const BG_TEXTURELOADER_ASSET: &str = "Assets/Resources/prefabs/textureloader.prefab";

pub(super) fn explicit_parallax_layer(tag: &str) -> Option<BgLayer> {
    match tag {
        "ParallaxLayerSky" => Some(BgLayer::Sky),
        "ParallaxLayerFixedFollowCamera" => Some(BgLayer::Camera),
        "ParallaxLayerFurther" => Some(BgLayer::Further),
        "ParallaxLayerFar" => Some(BgLayer::Far),
        "ParallaxLayerNear" => Some(BgLayer::Near),
        "ParallaxLayerForeground" => Some(BgLayer::Foreground),
        _ => None,
    }
}

pub(super) fn classify_group_layer(tag: &str) -> BgLayer {
    if let Some(layer) = explicit_parallax_layer(tag) {
        return layer;
    }

    match tag {
        "Ground" => BgLayer::Ground,
        _ => BgLayer::Ground,
    }
}

/// All known background atlas filenames.
pub fn bg_atlas_files() -> &'static [String] {
    static FILES: OnceLock<Vec<String>> = OnceLock::new();

    FILES.get_or_init(|| {
        texture2d_png_filenames()
            .into_iter()
            .filter(|name| name.starts_with("Background_") && name.contains("_Sheet_"))
            .collect()
    })
}

/// All known sky texture filenames.
pub fn sky_texture_files() -> &'static [String] {
    static FILES: OnceLock<Vec<String>> = OnceLock::new();

    FILES.get_or_init(|| {
        texture2d_png_filenames()
            .into_iter()
            .filter(|name| {
                let lower = name.to_ascii_lowercase();
                lower.ends_with("_sky_texture.png") || lower.ends_with("_sky.png")
            })
            .collect()
    })
}

fn texture2d_png_filenames() -> Vec<String> {
    let mut filenames: Vec<String> =
        crate::data::assets::list_pathnames("Assets/Texture2D/", ".png")
            .into_iter()
            .filter_map(|path| {
                Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .collect();
    filenames.sort();
    filenames.dedup();
    filenames
}
