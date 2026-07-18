//! Unity prefab multi-sprite database.
//!
//! Parses prefab child Sprite and unmanaged atlas components from the decompiled
//! Unity project and exposes baked local quads for prefabs that render as one
//! or more visible child sprites.

mod layout;
mod math;
mod parse;
mod types;

use std::collections::HashMap;
use std::sync::OnceLock;

pub use types::{PrefabLocalBounds, PrefabSpriteLayer};

use types::RuntimeSpriteMeta;

const PREFAB_DIR_ASSET: &str = "Assets/Prefab/";

struct PrefabSpriteSource {
    asset_path: String,
    layers: OnceLock<Option<Vec<PrefabSpriteLayer>>>,
}

fn runtime_sprites() -> &'static HashMap<String, RuntimeSpriteMeta> {
    static INSTANCE: OnceLock<HashMap<String, RuntimeSpriteMeta>> = OnceLock::new();
    INSTANCE.get_or_init(parse::load_runtime_sprites)
}

fn prefab_sprite_sources() -> &'static HashMap<String, PrefabSpriteSource> {
    static SOURCES: OnceLock<HashMap<String, PrefabSpriteSource>> = OnceLock::new();

    SOURCES.get_or_init(|| {
        crate::data::assets::list_pathnames(PREFAB_DIR_ASSET, ".prefab")
            .into_iter()
            .filter_map(|asset_path| {
                let filename = asset_path.strip_prefix(PREFAB_DIR_ASSET)?;
                let name = filename.strip_suffix(".prefab")?.to_string();
                Some((
                    name,
                    PrefabSpriteSource {
                        asset_path,
                        layers: OnceLock::new(),
                    },
                ))
            })
            .collect()
    })
}

fn exact_multi_sprite_layers(name: &str) -> Option<&'static [PrefabSpriteLayer]> {
    let source = prefab_sprite_sources().get(name)?;
    source
        .layers
        .get_or_init(|| {
            let layers = layout::parse_prefab_layers(name, &source.asset_path, runtime_sprites())?;
            (layers.len() > 1
                || ((name.starts_with("GoalArea") || name == "StepRope") && !layers.is_empty()))
            .then_some(layers)
        })
        .as_deref()
}

pub fn get_multi_sprite_layers(name: &str) -> Option<&'static [PrefabSpriteLayer]> {
    exact_multi_sprite_layers(name).or_else(|| {
        name.split(" (")
            .next()
            .filter(|base| *base != name)
            .and_then(exact_multi_sprite_layers)
    })
}

pub fn get_prefab_local_bounds(name: &str) -> Option<PrefabLocalBounds> {
    let layers = get_multi_sprite_layers(name)?;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for layer in layers {
        for vertex in layer.vertices {
            min_x = min_x.min(vertex.x);
            max_x = max_x.max(vertex.x);
            min_y = min_y.min(vertex.y);
            max_y = max_y.max(vertex.y);
        }
    }

    (min_x.is_finite() && min_y.is_finite()).then_some(PrefabLocalBounds {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

#[cfg(test)]
mod tests {
    use super::get_multi_sprite_layers;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn embedded_icon_pig_prefab_has_multiple_layers() {
        let Some(layers) = get_multi_sprite_layers("Icon_Pig_01") else {
            panic!("expected embedded multi-sprite data for Icon_Pig_01");
        };
        assert!(
            layers.len() >= 3,
            "expected Icon_Pig_01 to keep multiple embedded sprite layers"
        );
    }

    #[test]
    fn slingshot_prefab_keeps_three_runtime_layers() {
        let Some(layers) = get_multi_sprite_layers("Slingshot") else {
            panic!("expected prefab layers for Slingshot");
        };
        assert_eq!(layers.len(), 3);
        let first_atlas = &layers[0].atlas;
        assert!(!first_atlas.is_empty());
        assert!(layers.iter().all(|layer| layer.atlas == *first_atlas));
    }

    #[test]
    fn pressure_button_blue_prefab_keeps_base_and_bump_layers() {
        let Some(layers) = get_multi_sprite_layers("PressureButtonBlue") else {
            panic!("expected prefab layers for PressureButtonBlue");
        };
        assert_eq!(layers.len(), 2);
        assert!(layers.iter().all(|layer| layer.atlas == "IngameAtlas2.png"));
    }

    #[test]
    fn activated_hinge_door_blue_ice_prefab_keeps_three_runtime_layers() {
        let Some(layers) = get_multi_sprite_layers("ActivatedHingeDoorBlue_Ice") else {
            panic!("expected prefab layers for ActivatedHingeDoorBlue_Ice");
        };
        assert_eq!(layers.len(), 3);
        assert!(layers.iter().all(|layer| layer.atlas == "IngameAtlas2.png"));
    }

    #[test]
    fn goal_area_mm_gold_prefab_keeps_runtime_icon_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_MM_Gold") else {
            panic!("expected prefab layers for GoalArea_MM_Gold");
        };
        let Some(icon_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "IngameAtlas2.png")
        else {
            panic!("expected GoalArea_MM_Gold icon layer");
        };
        assert_close(icon_layer.uv.x, 0.5419922);
        assert_close(icon_layer.uv.y, 0.7719727);
        assert_close(icon_layer.uv.w, 0.02929688);
        assert_close(icon_layer.uv.h, 0.06054688);
    }

    #[test]
    fn goal_area_01_prefab_keeps_achievement_icon_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_01") else {
            panic!("expected prefab layers for GoalArea_01");
        };
        let Some(icon_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "Props_Generic_Sheet_01.png")
        else {
            panic!("expected GoalArea_01 achievement icon layer");
        };
        assert_close(icon_layer.uv.x, 0.0);
        assert_close(icon_layer.uv.y, 0.0);
        assert_close(icon_layer.uv.w, 0.125);
        assert_close(icon_layer.uv.h, 0.125);
    }

    #[test]
    fn goal_area_star_level_prefab_keeps_hat_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_StarLevel") else {
            panic!("expected prefab layers for GoalArea_StarLevel");
        };
        let Some(hat_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "Props_Generic_Sheet_01.png")
        else {
            panic!("expected GoalArea_StarLevel hat layer");
        };
        assert_close(hat_layer.uv.x, 0.75);
        assert_close(hat_layer.uv.y, 0.25);
        assert_close(hat_layer.uv.w, 0.125);
        assert_close(hat_layer.uv.h, 0.125);
    }

    #[test]
    fn step_rope_prefab_keeps_runtime_layer() {
        let Some(layers) = get_multi_sprite_layers("StepRope") else {
            panic!("expected prefab layers for StepRope");
        };
        assert_eq!(layers.len(), 1);
        assert!(!layers[0].atlas.is_empty());
        assert!(
            layers[0]
                .vertices
                .iter()
                .all(|vertex| { vertex.x.is_finite() && vertex.y.is_finite() })
        );
    }

    #[test]
    fn box_challenge_prefab_does_not_reemit_glow_layer() {
        assert!(get_multi_sprite_layers("BoxChallenge").is_none());
        assert!(get_multi_sprite_layers("DynamicBoxChallenge").is_none());
    }

    #[test]
    fn star_box_prefab_does_not_reemit_glow_layer() {
        assert!(get_multi_sprite_layers("StarBox").is_none());
        assert!(get_multi_sprite_layers("DynamicStarBox").is_none());
    }
}
