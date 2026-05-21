//! Runtime sprite atlas lookup from embedded prefab/material data, plus
//! representative UI child-name heuristics for preview sprite selection.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub(super) const RUNTIME_ATLAS_FILENAMES: &[&str] = &[
    "IngameAtlas.png",
    "IngameAtlas2.png",
    "IngameAtlas3.png",
    "Ingame_Characters_Sheet_01.png",
    "Ingame_Sheet_04.png",
    "MenuAtlas.png",
    "MenuAtlas2.png",
];

pub(super) fn is_runtime_atlas_filename(texture_name: &str) -> bool {
    RUNTIME_ATLAS_FILENAMES.contains(&texture_name)
}

pub(super) fn runtime_unique_atlas_for_material_id(material_id: &str) -> Option<&'static str> {
    runtime_unique_atlas_by_material_id()
        .get(material_id)
        .map(String::as_str)
}

fn runtime_unique_atlas_by_material_id() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

    MAP.get_or_init(build_runtime_unique_atlas_by_material_id)
}

fn build_runtime_unique_atlas_by_material_id() -> HashMap<String, String> {
    let runtime_sprites = super::runtime::load_runtime_sprites();
    let mut atlases_by_material_id: HashMap<String, HashSet<String>> = HashMap::new();

    for prefix in ["Assets/Prefab/", "Assets/Resources/ui/"] {
        for asset_path in crate::data::assets::list_pathnames(prefix, ".prefab") {
            let Some(text) = super::read_embedded_text(&asset_path) else {
                continue;
            };
            let parsed = super::parse::parse_prefab(&text);

            for component in &parsed.runtime_sprites {
                let Some(meta) = runtime_sprites.get(&component.sprite_id) else {
                    continue;
                };
                let Some(game_object) = parsed.game_objects.get(&component.game_object_id) else {
                    continue;
                };
                let Some(renderer) = parsed.renderers.get(&component.game_object_id) else {
                    continue;
                };
                if !game_object.active || !renderer.enabled {
                    continue;
                }
                let Some(texture_name) = crate::data::assets::effect_texture_name_for_material_guid(
                    &renderer.material_guid,
                ) else {
                    continue;
                };
                if !is_runtime_atlas_filename(texture_name) {
                    continue;
                }

                atlases_by_material_id
                    .entry(meta.material_id.clone())
                    .or_default()
                    .insert(texture_name.to_string());
            }
        }
    }

    atlases_by_material_id
        .into_iter()
        .filter_map(|(material_id, atlases)| {
            if atlases.len() != 1 {
                return None;
            }
            let atlas = atlases.into_iter().next()?;
            Some((material_id, atlas))
        })
        .collect()
}

pub(super) fn is_representative_runtime_sprite_name(game_object_name: &str) -> bool {
    // UI prefabs often serialize large background ribbons/panels before the
    // semantically representative button/icon/loading child the editor should
    // preview. Prefer those asset-authored child names instead of a per-prefab
    // sprite-id table.
    match normalize_runtime_sprite_name(game_object_name).as_str() {
        "closebutton"
        | "backbutton"
        | "loading"
        | "buybutton"
        | "claimbutton"
        | "watchad"
        | "openpopupbutton"
        | "trackicon"
        | "levelicon"
        | "softcurrencyicon"
        | "okbutton"
        | "infobutton" => true,
        _ => false,
    }
}

fn normalize_runtime_sprite_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

pub(super) fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    crate::data::assets::atlas_for_material_guid(material_guid)
}

#[cfg(test)]
mod tests {
    use super::runtime_unique_atlas_for_material_id;

    #[test]
    fn dynamic_runtime_atlas_map_recovers_unique_material_ids() {
        assert_eq!(
            runtime_unique_atlas_for_material_id("20936c462fac24dbb967c450f9cb0cb4"),
            Some("IngameAtlas2.png")
        );
        assert_eq!(
            runtime_unique_atlas_for_material_id("1dc9819db44b840edb8cdec9ef7b80c2"),
            Some("MenuAtlas2.png")
        );
        assert_eq!(
            runtime_unique_atlas_for_material_id("32e759dd981a043fa8fbcfd4997143ea"),
            Some("MenuAtlas2.png")
        );
    }

    #[test]
    fn conflicting_runtime_material_ids_stay_out_of_dynamic_map() {
        assert_eq!(
            runtime_unique_atlas_for_material_id("a286b652d38de4df384036482abc0571"),
            None
        );
        assert_eq!(
            runtime_unique_atlas_for_material_id("f300c561f75e74380a11f80d4d2647f3"),
            None
        );
    }
}
