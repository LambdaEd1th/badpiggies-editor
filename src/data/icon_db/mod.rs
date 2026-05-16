//! Icon layer database — rebuilds multi-layer part icons from embedded Unity raw assets.
//!
//! Each part type/custom index maps to one or more sprite layers, baked from
//! Part_*_NN_SET prefab hierarchies plus Sprites.bytes and spritemapping.bytes.

mod layout;
mod math;
mod parse;
mod runtime;
mod types;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;

pub use types::{IconLayer, PartInfo};

const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_DIR_ASSET: &str = "unity/prefabs";

static ICON_DB: OnceLock<HashMap<String, PartInfo>> = OnceLock::new();

/// Get the part info (z_offset + layers) for a given part type and custom part index.
/// Falls back to customPartIndex=0 if the exact variant is not found.
pub fn get_part_info(part_type: i32, custom_part_index: i32) -> Option<&'static PartInfo> {
    let db = ICON_DB.get_or_init(load);
    // Try exact match first
    let key = format!("{part_type}.{custom_part_index}");
    if let Some(info) = db.get(&key) {
        return Some(info);
    }
    // Fall back to default variant (customPartIndex=0)
    let key = format!("{part_type}.0");
    db.get(&key)
}

fn load() -> HashMap<String, PartInfo> {
    let runtime_sprites = runtime::load_runtime_sprites();
    if runtime_sprites.is_empty() {
        log::error!("Failed to build runtime sprite metadata for icon layer database");
        return HashMap::new();
    }

    let Some(manifest) = read_embedded_text(PREFAB_MANIFEST_ASSET) else {
        log::error!(
            "Failed to read embedded prefab manifest for icon layer database: {}",
            PREFAB_MANIFEST_ASSET
        );
        return HashMap::new();
    };

    let mut map = HashMap::new();
    for filename in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !filename.starts_with("Part_") || !filename.ends_with("_SET.prefab") {
            continue;
        }
        let Some(default_custom_part_index) = default_custom_part_index(filename) else {
            continue;
        };

        let asset_path = format!("{}/{}", PREFAB_DIR_ASSET, filename);
        let Some(text) = read_embedded_text(&asset_path) else {
            log::warn!(
                "Missing embedded part prefab for icon layers: {}",
                asset_path
            );
            continue;
        };

        let parsed = parse::parse_prefab(&text);
        let Some(part_type) = parsed.part_type else {
            continue;
        };
        let custom_part_index = parsed
            .custom_part_index
            .unwrap_or(default_custom_part_index);
        let layers = layout::build_part_layers(&parsed, &runtime_sprites);
        if layers.is_empty() {
            continue;
        }

        map.insert(
            format!("{part_type}.{custom_part_index}"),
            PartInfo {
                z_offset: parsed.z_offset,
                layers,
            },
        );
    }

    map
}

pub(super) fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn default_custom_part_index(filename: &str) -> Option<i32> {
    let stem = filename.strip_suffix(".prefab")?;
    let stem = stem.strip_suffix("_SET")?;
    let (_, suffix) = stem.rsplit_once('_')?;
    let variant: i32 = suffix.parse().ok()?;
    Some(variant - 1)
}

#[cfg(test)]
mod tests {
    use super::get_part_info;

    #[test]
    fn balloon_variant_keeps_embedded_face_layer() {
        let Some(info) = get_part_info(1, 7) else {
            panic!("expected embedded icon layers for Balloon custom part 7");
        };

        assert_eq!(info.layers.len(), 2);
        let face = info
            .layers
            .iter()
            .find(|layer| layer.go_name == "Face1")
            .expect("expected Face1 overlay layer");

        assert_eq!(face.atlas, "IngameAtlas3.png");
        assert!(
            (face.z_local - (-0.05)).abs() < 0.000_01,
            "unexpected z_local: {}",
            face.z_local
        );
        assert!(
            (face.v0_x - (-0.299479)).abs() < 0.000_01,
            "unexpected v0_x: {}",
            face.v0_x
        );
        assert!(
            (face.v2_y - 0.295312).abs() < 0.000_01,
            "unexpected v2_y: {}",
            face.v2_y
        );
    }
}
