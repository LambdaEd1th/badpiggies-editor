#![cfg(test)]
//! Background unit tests.

mod tests {
    use super::*;

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

        let Some(first_width) = cache.tile_info.get(&far_indices[0]).map(|(width, _)| *width) else {
            panic!("first far hill should tile");
        };

        for idx in far_indices {
            let Some(width) = cache.tile_info.get(&idx).map(|(block_width, _)| *block_width) else {
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

        assert!(!far_indices.is_empty(), "expected far hill sprites in jungle theme");
        let min_x = sprites[far_indices[0]].world_x;
        let max_x = sprites[far_indices[far_indices.len() - 1]].world_x;
        let mut diffs: Vec<f32> = far_indices
            .windows(2)
            .map(|pair| sprites[pair[1]].world_x - sprites[pair[0]].world_x)
            .collect();
        let expected_wrap_gap = median(&mut diffs);

        let Some(block_width) = cache.tile_info.get(&far_indices[0]).map(|(width, _)| *width) else {
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
        let Some(theme) = bg_data::get_theme("Jungle") else {
            panic!("jungle theme");
        };
        let sprites = &theme.sprites;

        let ocean_name_count = sprites
            .iter()
            .filter(|sprite| sprite.parent_group == "Ocean")
            .map(|sprite| sprite.name.to_ascii_lowercase())
            .collect::<HashSet<_>>()
            .len();

        assert!(
            ocean_name_count >= 2,
            "expected multiple Ocean sprite names"
        );

        let Some(wave) = sprites
            .iter()
            .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Waves")
        else {
            panic!("wave sprite");
        };
        let Some(foam) = sprites
            .iter()
            .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Foam")
        else {
            panic!("foam sprite");
        };

        let Some(wave_key) = tile_group_key(wave, "waves", ocean_name_count) else {
            panic!("wave key");
        };
        let Some(foam_key) = tile_group_key(foam, "foam", ocean_name_count) else {
            panic!("foam key");
        };

        assert!(
            wave_key != foam_key,
            "expected Ocean sub-bands to keep separate repeat groups"
        );
    }

    #[test]
    fn background_cloud_sprites_do_not_drift_over_time() {
        let offset_start = bg_sprite_x_animation_offset(
            "background_clouds _forest_01",
            0.0,
            &bg_data::BgLayer::Sky,
        );
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
                sprite.parent_group == "BGLayerClouds"
                    && sprite.name == "Background_Clouds _Forest_01"
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

        let Some(block_width) = cache.tile_info.get(&cloud_indices[0]).map(|(width, _)| *width)
        else {
            panic!("cloud strip should tile");
        };
        let actual_wrap_gap = block_width - (max_right - min_left);

        assert!(
            (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
            "expected wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
        );
    }

    /// Halloween BGLayerNear has 4 sprite names at 3 different Z values.
    /// Background_Plateau_02 (Z=6) and Lamp_01 (Z=5.5) both round to z_key=6
    /// under the old scheme, merging their interleaved X positions and producing
    /// a completely wrong block_width (~173 instead of ~185).
    /// With name-based splitting each type gets its own clean tile group.
    #[test]
    fn halloween_near_plateau_tiles_at_correct_period() {
        let Some(cache) = build_bg_layer_cache("Halloween", None) else {
            panic!("halloween cache");
        };
        let Some(theme) = bg_data::get_theme("Halloween") else {
            panic!("halloween theme");
        };
        let sprites = cache.sprites(theme);

        let mut plateau_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.parent_group == "BGLayerNear" && s.name == "Background_Plateau_02")
            .map(|(idx, _)| idx)
            .collect();
        assert!(
            plateau_indices.len() >= 4,
            "expected BGLayerNear plateau sprites"
        );

        plateau_indices.sort_by(|a, b| {
            sprites[*a]
                .world_x
                .partial_cmp(&sprites[*b].world_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let Some(block_width) = cache.tile_info.get(&plateau_indices[0]).map(|(w, _)| *w) else {
            panic!("plateau should tile");
        };

        // Correct period ≈ 7 * 26.4 ≈ 184–185.  Old (broken) period was ~173.
        assert!(
            block_width > 180.0,
            "block_width {block_width:.2} too small — Z-rounding collision bug"
        );
        assert!(
            block_width < 195.0,
            "block_width {block_width:.2} too large"
        );
    }
}
