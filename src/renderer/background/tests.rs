#![cfg(test)]
//! Background unit tests.

use crate::data::bg_data;
use crate::domain::parser::parse_level;
use crate::domain::types::LevelObject;

use super::cache::{
    bg_sprite_x_animation_offset, build_bg_layer_cache, build_bg_layer_cache_with_root_offset,
    sprite_display_height, sprite_display_width,
};
use super::draw::{background_base_color, content_ratio_x_for_bg_sprite, should_extend_fill_like};

fn median(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values[values.len() / 2]
}

fn is_maya_high_further_core(sprite: &bg_data::BgSprite) -> bool {
    sprite.parent_group == "BGLayerFurther"
        && sprite.name.starts_with("Background_Maya_High_Further_")
        && !sprite.name.contains("Fill")
        && sprite.local_x >= -58.5
        && sprite.local_x <= 15.0
}

fn is_maya_high_near_core(sprite: &bg_data::BgSprite) -> bool {
    sprite.parent_group == "BGLayerNear"
        && sprite.name == "Background_Maya_High_Near"
        && sprite.local_x <= 90.0
}

fn sprite_top_y(sprite: &bg_data::BgSprite) -> f32 {
    sprite.world_y + sprite_display_height(sprite) * 0.5
}

fn edge_based_block_width_for_indices(
    indices: &[usize],
    sprites: &[bg_data::BgSprite],
) -> Option<f32> {
    if indices.len() < 2 {
        return None;
    }

    let mut sorted = indices.to_vec();
    sorted.sort_by(|a, b| sprites[*a].world_x.total_cmp(&sprites[*b].world_x));

    let mut edge_gaps: Vec<f32> = sorted
        .windows(2)
        .map(|pair| {
            let a = &sprites[pair[0]];
            let b = &sprites[pair[1]];
            let a_right = a.world_x + sprite_display_width(a) * 0.5;
            let b_left = b.world_x - sprite_display_width(b) * 0.5;
            b_left - a_right
        })
        .collect();
    let median_edge_gap = median(&mut edge_gaps);

    let first = &sprites[sorted[0]];
    let last = &sprites[*sorted.last()?];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    Some(max_right - min_left + median_edge_gap)
}

fn collect_override_sprite_axis_values(
    raw: &str,
    group_name: &str,
    sprite_name: &str,
    axis_name: &str,
) -> Vec<f32> {
    let mut current_group = String::new();
    let mut current_sprite = String::new();
    let mut parsing_for_sprite = false;
    let mut values = Vec::new();

    for line in raw.lines() {
        let stripped = line.trim_end_matches('\r');
        let depth = stripped.len() - stripped.trim_start_matches('\t').len();
        let content = stripped.trim();
        if content.is_empty() {
            continue;
        }

        if depth == 1 && content.starts_with("GameObject ") {
            current_group = content[11..].trim().to_string();
            current_sprite.clear();
            parsing_for_sprite = false;
        } else if depth == 2 && content.starts_with("GameObject ") {
            current_sprite = content[11..].trim().to_string();
            parsing_for_sprite = false;
        } else if depth == 3 && content == "Component UnityEngine.Transform" {
            parsing_for_sprite = current_group == group_name && current_sprite == sprite_name;
        } else if parsing_for_sprite
            && content.starts_with("Float ")
            && let Some(rest) = content.strip_prefix("Float ")
            && let Some((axis, value)) = rest.split_once('=')
            && axis.trim() == axis_name
            && let Ok(parsed) = value.trim().parse::<f32>()
        {
            values.push(parsed);
        }
    }

    values
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
fn cave_background_base_color_uses_ground_fill_color() {
    assert_eq!(
        background_base_color(Some("Cave")),
        crate::data::assets::ground_color("Cave")
    );
    assert_eq!(
        background_base_color(Some("Jungle")),
        crate::data::assets::sky_top_color("Jungle")
    );
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
fn maya_high_near_core_strip_tiles_continuously() {
    let Some(cache) = build_bg_layer_cache("MayaHigh", None) else {
        panic!("maya high cache");
    };
    let Some(theme) = bg_data::get_theme("MayaHigh") else {
        panic!("maya high theme");
    };
    let sprites = cache.sprites(theme);

    let near_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Maya_High_Near"
        })
        .map(|(idx, _)| idx)
        .collect();

    assert_eq!(
        near_indices.len(),
        9,
        "expected the authored MayaHigh near cloud strip to keep its 9 prefab instances"
    );

    let core_indices: Vec<usize> = near_indices
        .iter()
        .copied()
        .filter(|idx| is_maya_high_near_core(&sprites[*idx]))
        .collect();
    assert_eq!(
        core_indices.len(),
        8,
        "expected MayaHigh near tiling to use the uniformly spaced 8-sprite core strip"
    );

    let Some(first_width) = cache
        .tile_info
        .get(&core_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("expected MayaHigh near core strip to tile");
    };

    for idx in core_indices {
        let Some(width) = cache.tile_info.get(&idx).map(|(width, _)| *width) else {
            panic!("expected every MayaHigh near core sprite to tile");
        };
        assert!(
            (width - first_width).abs() < 0.001,
            "expected MayaHigh near core sprites to share one repeat width, got {width} vs {first_width}"
        );
    }

    let non_core_indices: Vec<usize> = near_indices
        .into_iter()
        .filter(|idx| !is_maya_high_near_core(&sprites[*idx]))
        .collect();
    assert_eq!(
        non_core_indices.len(),
        1,
        "expected one non-periodic MayaHigh near tail sprite to stay finite"
    );
    assert!(
        !cache.tile_info.contains_key(&non_core_indices[0]),
        "expected the non-periodic MayaHigh near tail sprite to stay outside the tiled core"
    );
}

#[test]
fn maya_high_near_wrap_gap_matches_internal_edge_gap() {
    let Some(cache) = build_bg_layer_cache("MayaHigh", None) else {
        panic!("maya high cache");
    };
    let Some(theme) = bg_data::get_theme("MayaHigh") else {
        panic!("maya high theme");
    };
    let sprites = cache.sprites(theme);

    let mut near_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| is_maya_high_near_core(sprite))
        .map(|(idx, _)| idx)
        .collect();
    near_indices.sort_by(|a, b| {
        sprites[*a]
            .world_x
            .partial_cmp(&sprites[*b].world_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    assert!(
        near_indices.len() == 8,
        "expected the MayaHigh near tiled core to contain 8 evenly spaced sprites"
    );
    let first = &sprites[near_indices[0]];
    let last = &sprites[near_indices[near_indices.len() - 1]];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    let mut edge_gaps: Vec<f32> = near_indices
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
        .get(&near_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("maya high near strip should tile");
    };
    let actual_wrap_gap = block_width - (max_right - min_left);

    assert!(
        (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
        "expected MayaHigh near wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
    );
}

#[test]
fn maya_high_further_variants_share_one_repeat_group() {
    let Some(cache) = build_bg_layer_cache("MayaHigh", None) else {
        panic!("maya high cache");
    };
    let Some(theme) = bg_data::get_theme("MayaHigh") else {
        panic!("maya high theme");
    };
    let sprites = cache.sprites(theme);

    let mut sample_indices = Vec::new();
    for sprite_name in [
        "Background_Maya_High_Further_01",
        "Background_Maya_High_Further_02",
        "Background_Maya_High_Further_03",
    ] {
        let Some(index) = sprites
            .iter()
            .enumerate()
            .find(|(_, sprite)| sprite.name == sprite_name && is_maya_high_further_core(sprite))
            .map(|(index, _)| index)
        else {
            panic!("missing MayaHigh further sprite {sprite_name}");
        };
        sample_indices.push(index);
    }

    let Some(first_width) = cache
        .tile_info
        .get(&sample_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("expected MayaHigh further strip to tile");
    };
    let first_phase = cache
        .tile_phase
        .get(&sample_indices[0])
        .copied()
        .unwrap_or(0.0);

    assert!(
        first_width > 77.3 && first_width < 77.5,
        "expected MayaHigh further core strip width near the exact repeated 77.4-world translation, got {first_width}"
    );
    assert!(
        first_phase.abs() < 0.001,
        "expected MayaHigh further core strip to repeat without an extra phase offset, got {first_phase}"
    );

    for idx in sample_indices {
        let Some(width) = cache.tile_info.get(&idx).map(|(width, _)| *width) else {
            panic!("expected every MayaHigh further variant to tile");
        };
        let phase = cache.tile_phase.get(&idx).copied().unwrap_or(0.0);
        assert!(
            (width - first_width).abs() < 0.001,
            "expected MayaHigh further variants to share one block width, got {width} vs {first_width}"
        );
        assert!(
            (phase - first_phase).abs() < 0.001,
            "expected MayaHigh further variants to share one seam phase"
        );
    }
}

#[test]
fn maya_high_further_wrap_gap_matches_internal_edge_gap() {
    let Some(cache) = build_bg_layer_cache("MayaHigh", None) else {
        panic!("maya high cache");
    };
    let Some(theme) = bg_data::get_theme("MayaHigh") else {
        panic!("maya high theme");
    };
    let sprites = cache.sprites(theme);

    let core_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| is_maya_high_further_core(sprite))
        .map(|(idx, _)| idx)
        .collect();
    assert_eq!(
        core_indices.len(),
        7,
        "expected the exact repeating MayaHigh core strip to contain 7 sprites"
    );

    let Some(block_width) = cache
        .tile_info
        .get(&core_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("maya high further core strip should tile");
    };
    assert!(
        block_width > 77.3 && block_width < 77.5,
        "expected MayaHigh core strip to use the authored 77.4-world repeat, got {block_width}"
    );

    let mut exact_repeat_matches = 0;
    for &idx in &core_indices {
        let sprite = &sprites[idx];
        if !cache.tile_info.contains_key(&idx) {
            panic!("every MayaHigh core sprite should tile");
        }
        if sprites.iter().any(|other| {
            other.name == sprite.name
                && other.parent_group == sprite.parent_group
                && (other.local_x - (sprite.local_x + block_width)).abs() < 0.2
        }) {
            exact_repeat_matches += 1;
        }
    }
    assert!(
        exact_repeat_matches >= 6,
        "expected most MayaHigh core sprites to have an authored sibling one block-width to the right, got {exact_repeat_matches}"
    );

    let non_core_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerFurther"
                && sprite.name.starts_with("Background_Maya_High_Further_")
                && !sprite.name.contains("Fill")
                && !is_maya_high_further_core(sprite)
        })
        .map(|(idx, _)| idx)
        .collect();
    assert!(
        !non_core_indices.is_empty(),
        "expected non-repeating MayaHigh lead-in sprites to remain outside the tiled core"
    );
    for idx in non_core_indices {
        assert!(
            !cache.tile_info.contains_key(&idx),
            "expected non-core MayaHigh decoration sprites to stay finite"
        );
    }
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

    let Some(pattern_width) = cache
        .tile_info
        .get(&sample_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("expected MayaTemple near-bottom pattern to tile as one block");
    };
    let pattern_phase = cache
        .tile_phase
        .get(&sample_indices[0])
        .copied()
        .unwrap_or(0.0);

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

    assert_eq!(
        base_indices.len(),
        9,
        "expected 9 MayaTemple near-bottom base sprites"
    );

    let Some(first_width) = cache
        .tile_info
        .get(&base_indices[0])
        .map(|(width, _)| *width)
    else {
        panic!("expected MayaTemple near-bottom base strip to tile");
    };
    let base_phase = cache
        .tile_phase
        .get(&base_indices[0])
        .copied()
        .unwrap_or(0.0);

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
            sprite.parent_group == "BGLayerNearTop"
                && sprite.name == "Background_Maya_Temple_Near_Top"
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

#[test]
fn episode6_level1_background_cache_applies_level_root_offset() {
    let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_levels/assetbundles/episode_6_levels.unity3d/episode_6_level_1_data.bytes");
    let bytes = std::fs::read(&level_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
    let level = parse_level(bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

    let mut bg_override_text = None;
    let mut bg_root_offset = None;
    for object in &level.objects {
        if let LevelObject::Prefab(prefab) = object
            && prefab.name.contains("Background")
            && let Some(ref override_data) = prefab.override_data
        {
            bg_override_text = Some(override_data.raw_text.clone());
            bg_root_offset = Some([prefab.position.x, prefab.position.y, prefab.position.z]);
            break;
        }
    }

    let bg_override_text = bg_override_text.expect("episode 6 level 1 background override text");
    let bg_root_offset = bg_root_offset.expect("episode 6 level 1 background root offset");
    assert!(
        bg_root_offset[0].abs() > 1.0 || bg_root_offset[1].abs() > 1.0,
        "expected episode 6 level 1 to carry a non-default background root offset"
    );

    let Some(cache_without_offset) = build_bg_layer_cache("Maya", Some(&bg_override_text)) else {
        panic!("maya cache without level root offset");
    };
    let Some(cache_with_offset) = build_bg_layer_cache_with_root_offset(
        "Maya",
        Some(&bg_override_text),
        Some(bg_root_offset),
    ) else {
        panic!("maya cache with level root offset");
    };
    let Some(theme) = bg_data::get_theme("Maya") else {
        panic!("maya theme");
    };

    let without_offset = cache_without_offset
        .sprites(theme)
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("maya far fill without level root offset");
    let with_offset = cache_with_offset
        .sprites(theme)
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("maya far fill with level root offset");

    assert_eq!(with_offset.fill_color, Some([0xcd, 0xab, 0x74]));
    assert!((with_offset.world_x - without_offset.world_x - bg_root_offset[0]).abs() < 0.001);
    assert!((with_offset.world_y - without_offset.world_y - bg_root_offset[1]).abs() < 0.001);
    assert!((with_offset.world_z - without_offset.world_z - bg_root_offset[2]).abs() < 0.001);
}

#[test]
fn cave_far_fill_keeps_authored_height_and_sorts_behind_hills() {
    let Some(cache) = build_bg_layer_cache("Cave", None) else {
        panic!("cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let Some(fill_index) = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerFar"
                && sprite.name == "Background_Far_fill"
                && sprite.fill_color.is_some()
        })
        .min_by(|(_, a), (_, b)| sprite_top_y(a).total_cmp(&sprite_top_y(b)))
        .map(|(index, _)| index)
    else {
        panic!("missing Cave lower far fill");
    };

    let fill = &sprites[fill_index];
    let companion_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == fill.parent_group
                && sprite.layer == fill.layer
                && sprite.fill_color.is_none()
                && sprite.sky_texture.is_none()
                && !sprite.name.to_ascii_lowercase().contains("fill")
        })
        .map(|(index, _)| index)
        .collect();
    assert!(!companion_indices.is_empty(), "expected Cave far hill companions");

    let companion_max_z = companion_indices
        .iter()
        .map(|&index| sprites[index].world_z)
        .max_by(|a, b| a.total_cmp(b))
        .expect("companion z");

    assert!(
        !cache.fill_top_world_y.contains_key(&fill_index),
        "background fills should keep their authored height"
    );

    let overridden_sort_z = cache.sort_world_z(sprites, fill_index);
    assert!(
        (overridden_sort_z - (companion_max_z + 0.001)).abs() < 0.001,
        "expected Cave far fill sort z to move behind hills, got {overridden_sort_z} vs {}",
        companion_max_z + 0.001
    );

    let fill_pos = cache
        .sorted_indices
        .iter()
        .position(|&index| index == fill_index)
        .expect("fill position");
    let first_companion_pos = companion_indices
        .iter()
        .filter_map(|index| cache.sorted_indices.iter().position(|candidate| candidate == index))
        .min()
        .expect("companion positions");
    assert!(
        fill_pos < first_companion_pos,
        "expected Cave far fill to draw before companion hills"
    );
}

#[test]
fn cave_preserves_both_fill_bands_and_tiles_top_bottom_rows_separately() {
    let Some(cache) = build_bg_layer_cache("Cave", None) else {
        panic!("cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let near_fill_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Background_Near_fill"
                && sprite.fill_color.is_some()
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(near_fill_indices.len(), 2, "expected upper and lower Cave near fills");

    let far_fill_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "BGLayerFar"
                && sprite.name == "Background_Far_fill"
                && sprite.fill_color.is_some()
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(far_fill_indices.len(), 2, "expected upper and lower Cave far fills");

    for parent_group in ["BGLayerNear", "BGLayerFar"] {
        for row_sign in [1.0_f32, -1.0_f32] {
            let row_indices: Vec<usize> = sprites
                .iter()
                .enumerate()
                .filter(|(_, sprite)| {
                    sprite.parent_group == parent_group
                        && sprite.fill_color.is_none()
                        && sprite.sky_texture.is_none()
                        && !sprite.name.to_ascii_lowercase().contains("fill")
                        && sprite.scale_y.signum() == row_sign
                })
                .map(|(index, _)| index)
                .collect();

            assert!(
                row_indices.len() >= 4,
                "expected Cave {parent_group} row with sign {row_sign} to have repeated strip sprites"
            );

            let expected_width = edge_based_block_width_for_indices(&row_indices, sprites)
                .expect("expected per-row Cave block width");

            for index in row_indices {
                let Some((actual_width, _)) = cache.tile_info.get(&index) else {
                    panic!("expected Cave {parent_group} row sprite {index} to tile");
                };
                assert!(
                    (*actual_width - expected_width).abs() < 0.01,
                    "expected Cave {parent_group} row sign {row_sign} to tile with its own block width, got {actual_width} vs {expected_width}"
                );
            }
        }
    }
}

#[test]
fn level27_cave_override_keeps_duplicate_fill_and_strip_instances_distinct() {
    let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_levels/assetbundles/episode_1_levels.unity3d/Level_27_data.bytes");
    let bytes = std::fs::read(&level_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
    let level = parse_level(bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

    let mut bg_override_text = None;
    let mut bg_root_offset = None;
    for object in &level.objects {
        if let LevelObject::Prefab(prefab) = object
            && prefab.name.contains("Background")
            && let Some(ref override_data) = prefab.override_data
        {
            bg_override_text = Some(override_data.raw_text.clone());
            bg_root_offset = Some([prefab.position.x, prefab.position.y, prefab.position.z]);
            break;
        }
    }

    let bg_override_text = bg_override_text.expect("level 27 background override text");
    let bg_root_offset = bg_root_offset.expect("level 27 background root offset");

    let Some(cache) = build_bg_layer_cache_with_root_offset(
        "Cave",
        Some(&bg_override_text),
        Some(bg_root_offset),
    ) else {
        panic!("level 27 cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let mut near_fill_world_y: Vec<f32> = sprites
        .iter()
        .filter(|sprite| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Background_Near_fill"
                && sprite.fill_color.is_some()
        })
        .map(|sprite| sprite.world_y)
        .collect();
    near_fill_world_y.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(near_fill_world_y.len(), 2, "expected two Cave near fills after Level_27 overrides");
    assert!(
        near_fill_world_y[1] - near_fill_world_y[0] > 20.0,
        "expected Level_27 near fill overrides to keep upper and lower bands distinct, got {:?}",
        near_fill_world_y
    );

    let mut far_fill_world_y: Vec<f32> = sprites
        .iter()
        .filter(|sprite| {
            sprite.parent_group == "BGLayerFar"
                && sprite.name == "Background_Far_fill"
                && sprite.fill_color.is_some()
        })
        .map(|sprite| sprite.world_y)
        .collect();
    far_fill_world_y.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(far_fill_world_y.len(), 2, "expected two Cave far fills after Level_27 overrides");
    assert!(
        far_fill_world_y[1] - far_fill_world_y[0] > 3.0,
        "expected Level_27 far fill overrides to keep upper and lower bands distinct, got {:?}",
        far_fill_world_y
    );

    for parent_group in ["BGLayerNear", "BGLayerFar"] {
        let Some(strip_name) = sprites
            .iter()
            .find(|sprite| {
                sprite.parent_group == parent_group
                    && sprite.fill_color.is_none()
                    && sprite.sky_texture.is_none()
                    && !sprite.name.to_ascii_lowercase().contains("fill")
            })
            .map(|sprite| sprite.name.clone())
        else {
            panic!("missing strip sprite in {parent_group}");
        };

        let override_x_values = collect_override_sprite_axis_values(
            &bg_override_text,
            parent_group,
            &strip_name,
            "x",
        );
        let mut distinct_override_x = override_x_values.clone();
        distinct_override_x.sort_by(|a, b| a.total_cmp(b));
        distinct_override_x.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        let has_sprite_level_duplicates = distinct_override_x.len() > 1;

        let mut distinct_world_x: Vec<f32> = sprites
            .iter()
            .filter(|sprite| sprite.parent_group == parent_group && sprite.name == strip_name)
            .map(|sprite| sprite.world_x)
            .collect();
        distinct_world_x.sort_by(|a, b| a.total_cmp(b));
        distinct_world_x.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        if has_sprite_level_duplicates {
            assert!(
                distinct_world_x.len() > 1,
                "expected Level_27 override application to keep duplicate {strip_name} instances in {parent_group} at distinct x positions, got {:?}",
                distinct_world_x
            );
        }
    }
}

#[test]
fn sandbox_cave_override_keeps_duplicate_fill_and_strip_instances_distinct() {
    let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_levels/assetbundles/episode_sandbox_levels_2.unity3d/Level_Sandbox_01_data.bytes");
    let bytes = std::fs::read(&level_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
    let level = parse_level(bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

    let mut bg_override_text = None;
    let mut bg_root_offset = None;
    for object in &level.objects {
        if let LevelObject::Prefab(prefab) = object
            && prefab.name.contains("Background")
            && let Some(ref override_data) = prefab.override_data
        {
            bg_override_text = Some(override_data.raw_text.clone());
            bg_root_offset = Some([prefab.position.x, prefab.position.y, prefab.position.z]);
            break;
        }
    }

    let bg_override_text = bg_override_text.expect("sandbox background override text");
    let bg_root_offset = bg_root_offset.expect("sandbox background root offset");

    let Some(cache) = build_bg_layer_cache_with_root_offset(
        "Cave",
        Some(&bg_override_text),
        Some(bg_root_offset),
    ) else {
        panic!("sandbox cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let mut near_fill_world_y: Vec<f32> = sprites
        .iter()
        .filter(|sprite| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Background_Near_fill"
                && sprite.fill_color.is_some()
        })
        .map(|sprite| sprite.world_y)
        .collect();
    near_fill_world_y.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(near_fill_world_y.len(), 2, "expected two sandbox Cave near fills after overrides");
    assert!(
        near_fill_world_y[1] - near_fill_world_y[0] > 20.0,
        "expected sandbox near fill overrides to keep upper and lower bands distinct, got {:?}",
        near_fill_world_y
    );

    let mut far_fill_world_y: Vec<f32> = sprites
        .iter()
        .filter(|sprite| {
            sprite.parent_group == "BGLayerFar"
                && sprite.name == "Background_Far_fill"
                && sprite.fill_color.is_some()
        })
        .map(|sprite| sprite.world_y)
        .collect();
    far_fill_world_y.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(far_fill_world_y.len(), 2, "expected two sandbox Cave far fills after overrides");
    assert!(
        far_fill_world_y[1] - far_fill_world_y[0] > 3.0,
        "expected sandbox far fill overrides to keep upper and lower bands distinct, got {:?}",
        far_fill_world_y
    );

    for parent_group in ["BGLayerNear", "BGLayerFar"] {
        for row_sign in [1.0_f32, -1.0_f32] {
            let row_indices: Vec<usize> = sprites
                .iter()
                .enumerate()
                .filter(|(_, sprite)| {
                    sprite.parent_group == parent_group
                        && sprite.fill_color.is_none()
                        && sprite.sky_texture.is_none()
                        && !sprite.name.to_ascii_lowercase().contains("fill")
                        && sprite.scale_y.signum() == row_sign
                })
                .map(|(index, _)| index)
                .collect();

            assert!(
                row_indices.len() >= 4,
                "expected sandbox Cave {parent_group} row with sign {row_sign} to have repeated strip sprites"
            );

            for index in row_indices {
                assert!(
                    cache.tile_info.contains_key(&index),
                    "expected sandbox Cave {parent_group} row sprite {} to tile",
                    sprites[index].name
                );
            }
        }
    }
}

#[test]
fn sandbox_cave_foreground_fill_extends_and_pillars_tile() {
    let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_levels/assetbundles/episode_sandbox_levels_2.unity3d/Level_Sandbox_01_data.bytes");
    let bytes = std::fs::read(&level_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
    let level = parse_level(bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

    let mut bg_override_text = None;
    let mut bg_root_offset = None;
    for object in &level.objects {
        if let LevelObject::Prefab(prefab) = object
            && prefab.name.contains("Background")
            && let Some(ref override_data) = prefab.override_data
        {
            bg_override_text = Some(override_data.raw_text.clone());
            bg_root_offset = Some([prefab.position.x, prefab.position.y, prefab.position.z]);
            break;
        }
    }

    let bg_override_text = bg_override_text.expect("sandbox background override text");
    let bg_root_offset = bg_root_offset.expect("sandbox background root offset");

    let Some(cache) = build_bg_layer_cache_with_root_offset(
        "Cave",
        Some(&bg_override_text),
        Some(bg_root_offset),
    ) else {
        panic!("sandbox cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let fill_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "FGLayer"
                && (sprite.name == "Fill1" || sprite.name == "Fill1_2")
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(fill_indices.len(), 2, "expected both sandbox cave foreground fill sprites");
    for index in fill_indices {
        assert!(
            should_extend_fill_like(&sprites[index], &cache.name_lower[index], &cache, index),
            "expected sandbox cave foreground fill {} to extend like the legacy PIXI renderer",
            sprites[index].name
        );
    }

    let pillar_indices: Vec<usize> = sprites
        .iter()
        .enumerate()
        .filter(|(_, sprite)| {
            sprite.parent_group == "FGLayer"
                && (sprite
                    .name
                    .strip_prefix("Pillars")
                    .and_then(|suffix| suffix.trim_end_matches("_2").parse::<u32>().ok())
                    .is_some_and(|index| (1..=18).contains(&index)))
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(pillar_indices.len(), 36, "expected both sandbox cave foreground stalactite rows and their duplicated continuations");
    for index in pillar_indices {
        assert!(
            cache.tile_info.contains_key(&index),
            "expected sandbox cave pillar sprite {} to stay in the tiling path",
            sprites[index].name
        );
    }

    for row_sign in [1.0_f32, -1.0_f32] {
        let row_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, sprite)| {
                sprite.parent_group == "FGLayer"
                    && (sprite
                        .name
                        .strip_prefix("Pillars")
                        .and_then(|suffix| suffix.trim_end_matches("_2").parse::<u32>().ok())
                        .is_some_and(|index| (1..=18).contains(&index)))
                    && sprite.scale_y.signum() == row_sign
            })
            .map(|(index, _)| index)
            .collect();

        assert_eq!(
            row_indices.len(),
            18,
            "expected one full sandbox cave foreground pillar row for sign {row_sign}"
        );

        let expected_width = edge_based_block_width_for_indices(&row_indices, sprites)
            .expect("expected sandbox cave foreground row block width");
        let expected_phase = cache
            .tile_phase
            .get(&row_indices[0])
            .copied()
            .unwrap_or(0.0);

        for index in row_indices {
            let Some((actual_width, _)) = cache.tile_info.get(&index) else {
                panic!("expected sandbox cave foreground pillar row sprite {index} to tile");
            };
            assert!(
                (*actual_width - expected_width).abs() < 0.01,
                "expected sandbox cave foreground row sign {row_sign} to share one block width, got {actual_width} vs {expected_width} for {}",
                sprites[index].name
            );

            let actual_phase = cache.tile_phase.get(&index).copied().unwrap_or(0.0);
            assert!(
                (actual_phase - expected_phase).abs() < 0.01,
                "expected sandbox cave foreground row sign {row_sign} to share one phase, got {actual_phase} vs {expected_phase} for {}",
                sprites[index].name
            );
        }
    }
}

#[test]
fn jungle_near_fill_keeps_authored_height() {
    let Some(cache) = build_bg_layer_cache("Jungle", None) else {
        panic!("jungle cache");
    };
    let Some(theme) = bg_data::get_theme("Jungle") else {
        panic!("jungle theme");
    };
    let sprites = cache.sprites(theme);

    let Some(fill_index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| {
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Background_Near_fill"
                && sprite.fill_color.is_some()
        })
        .map(|(index, _)| index)
    else {
        panic!("missing Jungle near fill");
    };

    assert!(
        !cache.fill_top_world_y.contains_key(&fill_index),
        "background fills should keep authored Jungle near-fill height"
    );
}

#[test]
fn foreground_fill_does_not_use_background_hill_override() {
    let Some(cache) = build_bg_layer_cache("MayaCave2Dark", None) else {
        panic!("maya cave2dark cache");
    };
    let Some(theme) = bg_data::get_theme("MayaCave2Dark") else {
        panic!("maya cave2dark theme");
    };
    let sprites = cache.sprites(theme);

    let Some(fill_index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| sprite.parent_group == "FGLayer" && sprite.name == "Fill2")
        .map(|(index, _)| index)
    else {
        panic!("missing MayaCave2Dark FGLayer Fill2");
    };

    let fill = &sprites[fill_index];
    let has_fg_companions = sprites.iter().any(|sprite| {
        sprite.parent_group == fill.parent_group
            && sprite.layer == fill.layer
            && sprite.fill_color.is_none()
            && sprite.sky_texture.is_none()
            && !sprite.name.to_ascii_lowercase().contains("fill")
    });
    assert!(
        has_fg_companions,
        "expected foreground fill test case to have non-fill companions"
    );

    assert!(
        !cache.fill_top_world_y.contains_key(&fill_index),
        "foreground fills should keep authored top edge"
    );
    assert!(
        !cache.sort_z_overrides.contains_key(&fill_index),
        "foreground fills should keep authored sort z"
    );
}

#[test]
fn solid_background_fills_extend_with_viewport_width() {
    let Some(cache) = build_bg_layer_cache("Cave", None) else {
        panic!("cave cache");
    };
    let Some(theme) = bg_data::get_theme("Cave") else {
        panic!("cave theme");
    };
    let sprites = cache.sprites(theme);

    let Some(fill_index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| {
            sprite.parent_group == "BGLayerFar"
                && sprite.name == "Background_Far_fill"
                && sprite.fill_color.is_some()
        })
        .map(|(index, _)| index)
    else {
        panic!("missing Cave far fill");
    };

    assert!(
        should_extend_fill_like(
            &sprites[fill_index],
            &cache.name_lower[fill_index],
            &cache,
            fill_index,
        ),
        "solid background fills should expand with the viewport like Unity BackgroundScaler"
    );
}

#[test]
fn maya_high_atlas_fill_uses_stretched_extended_uvs() {
    let Some(cache) = build_bg_layer_cache("MayaHigh", None) else {
        panic!("maya high cache");
    };
    let Some(theme) = bg_data::get_theme("MayaHigh") else {
        panic!("maya high theme");
    };
    let sprites = cache.sprites(theme);

    let Some(fill_index) = sprites
        .iter()
        .enumerate()
        .find(|(_, sprite)| sprite.name == "Background_Maya_High_Near_Fill")
        .map(|(index, _)| index)
    else {
        panic!("missing MayaHigh near fill");
    };

    let sprite = &sprites[fill_index];
    assert!(sprite.atlas.is_some(), "expected MayaHigh near fill to stay atlas-backed");

    let extend_fill_like = should_extend_fill_like(
        sprite,
        &cache.name_lower[fill_index],
        &cache,
        fill_index,
    );
    assert!(extend_fill_like, "expected MayaHigh near fill to use the viewport extension path");

    let orig_display_w = sprite_display_width(sprite);
    let extended_display_w = orig_display_w * 3.0;
    assert_eq!(
        content_ratio_x_for_bg_sprite(sprite, extend_fill_like, orig_display_w, extended_display_w),
        1.0,
        "extended atlas fills should stretch their texture like the old TS renderer"
    );
}

#[test]
fn fill_sort_override_does_not_reorder_background_layers() {
    let Some(cache) = build_bg_layer_cache("Jungle", None) else {
        panic!("jungle cache");
    };
    let Some(theme) = bg_data::get_theme("Jungle") else {
        panic!("jungle theme");
    };
    let sprites = cache.sprites(theme);

    let far_max_pos = cache
        .sorted_indices
        .iter()
        .enumerate()
        .filter_map(|(pos, &index)| (sprites[index].layer == bg_data::BgLayer::Far).then_some(pos))
        .max()
        .expect("expected Jungle far-layer sprites");

    let near_fill_pos = cache
        .sorted_indices
        .iter()
        .position(|&index| {
            let sprite = &sprites[index];
            sprite.parent_group == "BGLayerNear"
                && sprite.name == "Background_Near_fill"
                && sprite.fill_color.is_some()
        })
        .expect("missing Jungle near fill");

    assert!(
        far_max_pos < near_fill_pos,
        "fill sort overrides must not move a near-layer fill ahead of far-layer sprites"
    );
}
