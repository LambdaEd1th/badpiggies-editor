#![cfg(test)]
//! Background unit tests.

use crate::data::bg_data;

use super::cache::{bg_sprite_x_animation_offset, build_bg_layer_cache, sprite_display_width};

fn median(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values[values.len() / 2]
}

#[test]
fn jungle_far_tiles_share_one_period_across_z() {
    let Some(cache) = build_bg_layer_cache("Jungle", None) else {
        panic!("jungle cache");
    };
    let Some(theme) = bg_data::get_theme("Jungle") else {
        panic!("jungle theme");
    };
    let sprites = cache.sprites(theme);

    let far_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02"
        })
        .map(|(idx, _)| idx)
        .collect();

    assert!(
        far_indices.len() > 4,
        "expected far hill sprites in jungle theme"
    );

    let Some(first_width) = cache
        .tile_info
        .get(&far_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("first far hill should tile");
    };

    for idx in far_indices {
        let Some(width) = cache
            .tile_info
            .get(&idx)
            .map(|(block_width, _)| *block_width)
        else {
            panic!("every far hill should tile");
        };
        assert!(
            (width - first_width).abs() < 0.001,
            "expected shared block width, got {width} vs {first_width}"
        );
    }
}

#[test]
fn jungle_far_wrap_gap_matches_internal_spacing() {
    let Some(cache) = build_bg_layer_cache("Jungle", None) else {
        panic!("jungle cache");
    };
    let Some(theme) = bg_data::get_theme("Jungle") else {
        panic!("jungle theme");
    };
    let sprites = cache.sprites(theme);

    let mut far_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02"
        })
        .map(|(idx, _)| idx)
        .collect();
    far_indices.sort_by(|a, b| {
        sprites[*a]
            .world_x
            .partial_cmp(&sprites[*b].world_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert!(
        !far_indices.is_empty(),
        "expected far hill sprites in jungle theme"
    );
    let min_x = sprites[far_indices[0]].world_x;
    let max_x = sprites[far_indices[far_indices.len() - 1]].world_x;
    let mut diffs: Vec<f32> = far_indices
        .windows(2)
        .map(|pair| sprites[pair[1]].world_x - sprites[pair[0]].world_x)
        .collect();
    let expected_wrap_gap = median(&mut diffs);

    let Some(block_width) = cache
        .tile_info
        .get(&far_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("far hills should tile");
    };
    let actual_wrap_gap = block_width - (max_x - min_x);

    assert!(
        (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
        "expected wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
    );
}

#[test]
fn ocean_parent_group_splits_by_name_when_names_differ() {
    let Some(cache) = build_bg_layer_cache("Jungle", None) else {
        panic!("jungle cache");
    };
    let Some(theme) = bg_data::get_theme("Jungle") else {
        panic!("jungle theme");
    };
    let sprites = cache.sprites(theme);

    let Some(dummy_index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| sprite.parent_group == "Ocean" && sprite.name == "Dummy")
        .map(|(idx, _)| idx)
    else {
        panic!("dummy sprite");
    };
    let wave_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| sprite.parent_group == "Ocean" && sprite.name == "Waves")
        .map(|(idx, _)| idx)
        .collect();

    assert!(!wave_indices.is_empty(), "expected Ocean wave sprites");
    let Some(first_wave_width) = cache
        .tile_info
        .get(&wave_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("wave strip should tile");
    };
    let dummy_tile_width = cache.tile_info.get(&dummy_index).map(|(width, _)| *width);

    assert!(
        dummy_tile_width != Some(first_wave_width),
        "expected Ocean Dummy not to inherit the Waves repeat width"
    );
    for idx in wave_indices {
        let Some(width) = cache
            .tile_info
            .get(&idx)
            .map(|(block_width, _)| *block_width)
        else {
            panic!("every Ocean wave should tile");
        };
        assert!(
            (width - first_wave_width).abs() < 0.001,
            "expected Ocean waves to share one block width"
        );
    }
}

#[test]
fn background_cloud_sprites_do_not_drift_over_time() {
    let offset_start =
        bg_sprite_x_animation_offset("background_clouds _forest_01", 0.0, &bg_data::BgLayer::Sky);
    let offset_later = bg_sprite_x_animation_offset(
        "background_clouds _forest_01",
        123.45,
        &bg_data::BgLayer::Sky,
    );

    assert_eq!(offset_start, 0.0);
    assert_eq!(offset_later, 0.0);
}

#[test]
fn morning_cloud_wrap_gap_matches_internal_edge_gap() {
    let Some(cache) = build_bg_layer_cache("Morning", None) else {
        panic!("morning cache");
    };
    let Some(theme) = bg_data::get_theme("Morning") else {
        panic!("morning theme");
    };
    let sprites = cache.sprites(theme);

    let mut cloud_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerClouds" && sprite.name == "Background_Clouds _Forest_01"
        })
        .map(|(idx, _)| idx)
        .collect();
    cloud_indices.sort_by(|a, b| {
        sprites[*a]
            .world_x
            .partial_cmp(&sprites[*b].world_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert!(
        !cloud_indices.is_empty(),
        "expected BGLayerClouds sprites in morning theme"
    );
    let first = &sprites[cloud_indices[0]];
    let last = &sprites[cloud_indices[cloud_indices.len() - 1]];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    let mut edge_gaps: Vec<f32> = cloud_indices
        .windows(2)
        .map(|pair| {
            let a = &sprites[pair[0]];
            let b = &sprites[pair[1]];
            let a_right = a.world_x + sprite_display_width(a) * 0.5;
            let b_left = b.world_x - sprite_display_width(b) * 0.5;
            b_left - a_right
        })
        .collect();
    let expected_wrap_gap = median(&mut edge_gaps);

    let Some(block_width) = cache
        .tile_info
        .get(&cloud_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("cloud strip should tile");
    };
    let actual_wrap_gap = block_width - (max_right - min_left);

    assert!(
        (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
        "expected wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
    );
}

#[test]
fn morning_foreground_unique_names_still_share_one_repeat_group() {
    let Some(cache) = build_bg_layer_cache("Morning", None) else {
        panic!("morning cache");
    };
    let Some(theme) = bg_data::get_theme("Morning") else {
        panic!("morning theme");
    };
    let sprites = cache.sprites(theme);

    let mut forest_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerForeground"
                && sprite.name.starts_with("Foreground _Forest_")
        })
        .map(|(idx, _)| idx)
        .collect();
    forest_indices.sort_by(|a, b| {
        sprites[*a]
            .world_x
            .partial_cmp(&sprites[*b].world_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert_eq!(
        forest_indices.len(),
        14,
        "expected 14 foreground tree sprites"
    );

    let Some(first_width) = cache
        .tile_info
        .get(&forest_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("foreground tree strip should still tile as one group");
    };

    for idx in [
        forest_indices[0],
        forest_indices[1],
        forest_indices[forest_indices.len() - 1],
    ] {
        let Some(width) = cache
            .tile_info
            .get(&idx)
            .map(|(block_width, _)| *block_width)
        else {
            panic!("every foreground tree should share the strip width");
        };
        assert!(
            (width - first_width).abs() < 0.001,
            "expected unified foreground strip width, got {width} vs {first_width}"
        );
    }
    assert!(
        first_width > 340.0 && first_width < 360.0,
        "unexpected foreground strip width {first_width}"
    );
}

#[test]
fn halloween_near_same_z_names_get_distinct_periods() {
    let Some(cache) = build_bg_layer_cache("Halloween", None) else {
        panic!("halloween cache");
    };
    let Some(theme) = bg_data::get_theme("Halloween") else {
        panic!("halloween theme");
    };
    let sprites = cache.sprites(theme);

    let lamp_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Lamp_01"
                && (sprite.world_z - 5.5).abs() < 0.001
        })
        .map(|(idx, _)| idx)
        .collect();
    let pumpkin_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Pumpkin_01"
                && (sprite.world_z - 5.5).abs() < 0.001
        })
        .map(|(idx, _)| idx)
        .collect();

    assert!(!lamp_indices.is_empty(), "expected Lamp_01 strip");
    assert!(!pumpkin_indices.is_empty(), "expected Pumpkin_01 strip");

    let Some(lamp_width) = cache
        .tile_info
        .get(&lamp_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("Lamp_01 should tile");
    };
    let Some(pumpkin_width) = cache
        .tile_info
        .get(&pumpkin_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("Pumpkin_01 should tile");
    };

    for idx in lamp_indices {
        let Some(width) = cache
            .tile_info
            .get(&idx)
            .map(|(block_width, _)| *block_width)
        else {
            panic!("every Lamp_01 sprite should tile");
        };
        assert!((width - lamp_width).abs() < 0.001, "Lamp_01 width mismatch");
    }
    for idx in pumpkin_indices {
        let Some(width) = cache
            .tile_info
            .get(&idx)
            .map(|(block_width, _)| *block_width)
        else {
            panic!("every Pumpkin_01 sprite should tile");
        };
        assert!(
            (width - pumpkin_width).abs() < 0.001,
            "Pumpkin_01 width mismatch"
        );
    }

    assert!(
        lamp_width > 184.0 && lamp_width < 188.0,
        "unexpected Lamp_01 block width {lamp_width:.3}"
    );
    assert!(
        pumpkin_width > 204.0 && pumpkin_width < 208.0,
        "unexpected Pumpkin_01 block width {pumpkin_width:.3}"
    );
    assert!(
        (lamp_width - pumpkin_width).abs() > 10.0,
        "expected Lamp_01 and Pumpkin_01 to keep distinct repeat periods"
    );
}

#[test]
fn maya_temple_near_bottom_pattern_tiles_as_one_block() {
    let Some(cache) = build_bg_layer_cache("MayaTemple", None) else {
        panic!("maya temple cache");
    };
    let Some(theme) = bg_data::get_theme("MayaTemple") else {
        panic!("maya temple theme");
    };
    let sprites = cache.sprites(theme);

    let mut sample_indices = Vec::new();
    for sprite_name in [
        "Background_Maya_Temple_Near_01",
        "Background_Maya_Temple_Near_02",
        "Background_Maya_Temple_Near_03",
        "Background_Maya_Temple_Near_04",
    ] {
        let Some(index) = sprites
            .iter()
            .enumerate()
            .find(|(_, sprite)| {
                sprite.parent_group == "BGLayerNearBottom" && sprite.name == sprite_name
            })
            .map(|(index, _)| index)
        else {
            panic!("missing MayaTemple near-bottom sprite {sprite_name}");
        };
        sample_indices.push(index);
    }

    let Some(pattern_width) = cache.tile_info.get(&sample_indices[0]).map(|(width, _)| *width)
    else {
        panic!("expected MayaTemple near-bottom pattern to tile as one block");
    };
    let pattern_phase = cache.tile_phase.get(&sample_indices[0]).copied().unwrap_or(0.0);

    assert!(
        pattern_width > 239.5 && pattern_width < 240.2,
        "expected MayaTemple near-bottom pattern to follow the combined NearBottom motif width, got {pattern_width}"
    );
    assert!(
        pattern_phase > -106.0 && pattern_phase < -104.5,
        "expected MayaTemple near-bottom pattern seam phase to follow the shared NearBottom seam, got {pattern_phase}"
    );

    for idx in sample_indices {
        let Some(width) = cache.tile_info.get(&idx).map(|(width, _)| *width) else {
            panic!("expected every MayaTemple near-bottom pattern sprite to tile");
        };
        assert!(
            (width - pattern_width).abs() < 0.001,
            "expected MayaTemple near-bottom pattern to share one block width, got {width} vs {pattern_width}"
        );
    }

    let Some(base_width) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| {
            sprite.parent_group == "BGLayerNearBottom"
                && sprite.name == "Background_Maya_Temple_Near_Base"
        })
        .and_then(|(index, _)| cache.tile_info.get(&index).map(|(width, _)| *width))
    else {
        panic!("expected MayaTemple near-bottom base strip to tile");
    };

    assert!(
        (pattern_width - base_width).abs() < 0.001,
        "expected MayaTemple near-bottom pattern and base to share one repeat width"
    );
}

#[test]
fn maya_temple_near_bottom_base_tiles() {
    let Some(cache) = build_bg_layer_cache("MayaTemple", None) else {
        panic!("maya temple cache");
    };
    let Some(theme) = bg_data::get_theme("MayaTemple") else {
        panic!("maya temple theme");
    };
    let sprites = cache.sprites(theme);

    let mut base_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerNearBottom"
                && sprite.name == "Background_Maya_Temple_Near_Base"
        })
        .map(|(index, _)| index)
        .collect();
    base_indices.sort_by(|a, b| {
        sprites[*a]
            .world_x
            .partial_cmp(&sprites[*b].world_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert_eq!(base_indices.len(), 9, "expected 9 MayaTemple near-bottom base sprites");

    let Some(first_width) = cache
        .tile_info
        .get(&base_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("expected MayaTemple near-bottom base strip to tile");
    };
    let base_phase = cache.tile_phase.get(&base_indices[0]).copied().unwrap_or(0.0);

    assert!(
        base_phase > -106.0 && base_phase < -104.5,
        "expected MayaTemple near-bottom base seam phase to follow the shared NearBottom seam, got {base_phase}"
    );

    for idx in base_indices {
        let Some(width) = cache.tile_info.get(&idx).map(|(width, _)| *width) else {
            panic!("expected every MayaTemple near-bottom base sprite to tile");
        };
        assert!(
            (width - first_width).abs() < 0.001,
            "expected shared MayaTemple near-bottom base width, got {width} vs {first_width}"
        );
    }

    let Some(pattern_phase) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| {
            sprite.parent_group == "BGLayerNearBottom"
                && sprite.name == "Background_Maya_Temple_Near_01"
        })
        .and_then(|(index, _)| cache.tile_phase.get(&index).copied())
    else {
        panic!("expected MayaTemple near-bottom pattern seam phase");
    };
    assert!(
        (pattern_phase - base_phase).abs() < 0.001,
        "expected MayaTemple near-bottom pattern and base to share one seam phase"
    );
}

#[test]
fn maya_temple_near_top_still_tiles() {
    let Some(cache) = build_bg_layer_cache("MayaTemple", None) else {
        panic!("maya temple cache");
    };
    let Some(theme) = bg_data::get_theme("MayaTemple") else {
        panic!("maya temple theme");
    };
    let sprites = cache.sprites(theme);

    let Some(index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| {
            sprite.parent_group == "BGLayerNearTop" && sprite.name == "Background_Maya_Temple_Near_Top"
        })
        .map(|(index, _)| index)
    else {
        panic!("missing MayaTemple near-top strip sprite");
    };

    assert!(
        cache.tile_info.contains_key(&index),
        "expected MayaTemple near-top strip to keep tiling"
    );
}
