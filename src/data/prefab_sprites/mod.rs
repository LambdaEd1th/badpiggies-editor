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

fn runtime_sprites() -> &'static HashMap<String, RuntimeSpriteMeta> {
    static INSTANCE: OnceLock<HashMap<String, RuntimeSpriteMeta>> = OnceLock::new();
    INSTANCE.get_or_init(parse::load_runtime_sprites)
}

fn multi_sprite_prefabs() -> &'static HashMap<String, Vec<PrefabSpriteLayer>> {
    static INSTANCE: OnceLock<HashMap<String, Vec<PrefabSpriteLayer>>> = OnceLock::new();
    INSTANCE.get_or_init(|| layout::load_multi_sprite_prefabs(runtime_sprites()))
}

pub fn get_multi_sprite_layers(name: &str) -> Option<&'static [PrefabSpriteLayer]> {
    let db = multi_sprite_prefabs();
    db.get(name)
        .or_else(|| name.split(" (").next().and_then(|base| db.get(base)))
        .map(Vec::as_slice)
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
