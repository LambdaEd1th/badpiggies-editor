use super::parse::{parse_prefab, read_embedded_text};
use super::tables::explicit_parallax_layer;
use super::{
    BgLayer, atlas_for_material_guid, bg_atlas_files, get_theme, parse_bg_overrides,
    parse_position_serializer_overrides, parse_runtime_bg_overrides, sky_texture_files,
};
use crate::domain::level::refs::MaterialShaderKind;
use crate::domain::parser::parse_level;
use crate::domain::types::LevelObject;

#[test]
fn background_themes_can_come_from_prefab_scan() {
    for theme_name in [
        "Cave",
        "Morning",
        "Halloween",
        "Jungle",
        "Maya",
        "MayaCave",
        "MayaCaveDark",
        "MayaCave2Dark",
        "MayaHigh",
        "MayaTemple",
        "Night",
        "Plateau",
    ] {
        assert!(
            get_theme(theme_name).is_some(),
            "missing scanned theme {theme_name}"
        );
    }

    assert!(get_theme("backgroundobject").is_none());
}

#[test]
fn atlas_for_material_guid_resolves_generated_maya_prefixes() {
    assert_eq!(
        atlas_for_material_guid("0de59521"),
        Some("Background_Maya_Sheet_02.png")
    );
    assert_eq!(
        atlas_for_material_guid("d2458d0c"),
        Some("Background_Maya_Sheet_05.png")
    );
}

#[test]
fn background_texture_lists_can_come_from_embedded_asset_scan() {
    assert!(
        bg_atlas_files()
            .iter()
            .any(|name| name == "Background_Maya_Sheet_05.png")
    );
    assert!(
        bg_atlas_files()
            .iter()
            .any(|name| name == "Background_Night_Sheet_01.png")
    );
    assert!(
        sky_texture_files()
            .iter()
            .any(|name| name == "Maya_Backgrounds_sky.png")
    );
    assert!(
        sky_texture_files()
            .iter()
            .any(|name| name == "Morning_Sky_Texture.png")
    );
    assert!(sky_texture_files().iter().all(|name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with("_sky_texture.png") || lower.ends_with("_sky.png")
    }));
}

#[test]
fn far_fill_colors_can_come_from_material_assets() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_fill = morning
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("missing Morning far fill");
    assert_eq!(morning_fill.fill_color, Some([0x6d, 0x7e, 0x96]));
    assert!(morning_fill.atlas.is_none());

    let plateau = get_theme("Plateau").expect("missing Plateau theme");
    let plateau_fill = plateau
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("missing Plateau far fill");
    assert_eq!(plateau_fill.fill_color, Some([0xcc, 0xaa, 0x21]));
    assert!(plateau_fill.atlas.is_none());

    for theme_name in ["MayaCave", "MayaCaveDark"] {
        let theme = get_theme(theme_name).unwrap_or_else(|| panic!("missing {theme_name} theme"));
        let fill = theme
            .sprites
            .iter()
            .find(|sprite| {
                sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill"
            })
            .unwrap_or_else(|| panic!("missing {theme_name} far fill"));
        assert_eq!(
            fill.fill_color,
            Some([0x21, 0x44, 0x21]),
            "unexpected far fill for {theme_name}"
        );
        assert!(
            fill.atlas.is_none(),
            "unexpected far fill atlas for {theme_name}"
        );
    }

    let cave = get_theme("Cave").expect("missing Cave theme");
    let cave_fill = cave
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("missing Cave far fill");
    assert_eq!(cave_fill.fill_color, Some([0x21, 0x44, 0x21]));
    assert!(cave_fill.atlas.is_none());

    let jungle = get_theme("Jungle").expect("missing Jungle theme");
    let jungle_fill = jungle
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("missing Jungle far fill");
    assert_eq!(jungle_fill.fill_color, Some([0x54, 0xaa, 0x44]));
    assert!(jungle_fill.atlas.is_none());

    let maya = get_theme("Maya").expect("missing Maya theme");
    let maya_fill = maya
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Far_fill")
        .expect("missing Maya far fill");
    assert_eq!(maya_fill.fill_color, Some([0xcd, 0xab, 0x74]));
    assert!(maya_fill.atlas.is_none());

    let maya_fill2 = maya
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerFurther" && sprite.name == "Background_Far_fill2"
        })
        .expect("missing Maya far fill2");
    assert_eq!(maya_fill2.fill_color, Some([0xdd, 0xdd, 0xdd]));
    assert!(maya_fill2.atlas.is_none());
}

#[test]
fn near_fill_colors_can_come_from_material_assets() {
    for theme_name in ["MayaCave", "MayaCaveDark"] {
        let theme = get_theme(theme_name).unwrap_or_else(|| panic!("missing {theme_name} theme"));
        let fill = theme
            .sprites
            .iter()
            .find(|sprite| {
                sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
            })
            .unwrap_or_else(|| panic!("missing {theme_name} near fill"));
        assert_eq!(
            fill.fill_color,
            Some([0x11, 0x21, 0x11]),
            "unexpected near fill for {theme_name}"
        );
        assert!(
            fill.atlas.is_none(),
            "unexpected near fill atlas for {theme_name}"
        );
    }

    let cave = get_theme("Cave").expect("missing Cave theme");
    let cave_fill = cave
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
        })
        .expect("missing Cave near fill");
    assert_eq!(cave_fill.fill_color, Some([0x11, 0x21, 0x11]));
    assert!(cave_fill.atlas.is_none());

    let jungle = get_theme("Jungle").expect("missing Jungle theme");
    let jungle_fill = jungle
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
        })
        .expect("missing Jungle near fill");
    assert_eq!(jungle_fill.fill_color, Some([0x33, 0x88, 0x44]));
    assert!(jungle_fill.atlas.is_none());

    let plateau = get_theme("Plateau").expect("missing Plateau theme");
    let plateau_fill = plateau
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
        })
        .expect("missing Plateau near fill");
    assert_eq!(plateau_fill.fill_color, Some([0x88, 0x77, 0x21]));
    assert!(plateau_fill.atlas.is_none());

    let plateau_grass_fill = plateau
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "GrassLayer" && sprite.name == "Grass_fill")
        .expect("missing Plateau grass fill");
    assert_eq!(plateau_grass_fill.fill_color, Some([0x33, 0x77, 0x66]));
    assert!(plateau_grass_fill.atlas.is_none());
}

#[test]
fn morning_near_fill_keeps_uniform_tinted_atlas() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_fill = morning
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
        })
        .expect("missing Morning near fill");
    assert!(morning_fill.atlas.is_some());
    assert!(morning_fill.fill_color.is_none());
}

#[test]
fn foreground_fill_materials_match_runtime_resolution() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_fill = morning
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerForeground" && sprite.name == "Fill")
        .expect("missing Morning foreground fill");
    assert!(morning_fill.atlas.is_some());
    assert!(morning_fill.fill_color.is_none());

    let plateau = get_theme("Plateau").expect("missing Plateau theme");
    let plateau_fill = plateau
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "FGLayer" && sprite.name == "Fill")
        .expect("missing Plateau foreground fill");
    assert_eq!(plateau_fill.fill_color, Some([0x21, 0x44, 0x44]));
    assert!(plateau_fill.atlas.is_none());
}

#[test]
fn jungle_theme_preserves_runtime_material_shader_modes() {
    let jungle = get_theme("Jungle").expect("missing Jungle theme");

    let near_fill = jungle
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Near_fill"
        })
        .expect("missing Jungle near fill");
    assert_eq!(
        near_fill.shader_kind,
        MaterialShaderKind::CustomUnlitMonochrome
    );
    assert_eq!(near_fill.main_tex_st, [1.0, 1.0, 0.0, 0.0]);

    let far_bg = jungle
        .sprites
        .iter()
        .find(|sprite| {
            sprite.atlas.is_some()
                && sprite.shader_kind == MaterialShaderKind::CustomUnlitAlpha8BitColor
        })
        .expect("missing Jungle alpha8bit atlas sprite");
    assert_eq!(
        far_bg.shader_kind,
        MaterialShaderKind::CustomUnlitAlpha8BitColor
    );
    assert_eq!(far_bg.main_tex_st, [1.0, 1.0, 0.0, 0.0]);

    let sky = jungle
        .sprites
        .iter()
        .find(|sprite| sprite.sky_texture.is_some())
        .expect("missing Jungle sky sprite");
    assert_eq!(sky.shader_kind, MaterialShaderKind::BuiltinUnlitTransparent);
    assert_eq!(sky.main_tex_st, [1.0, 1.0, 0.0, 0.0]);
}

#[test]
fn remaining_fill_colors_can_come_from_material_assets() {
    let maya_cave2_dark = get_theme("MayaCave2Dark").expect("missing MayaCave2Dark theme");
    let sky_fill = maya_cave2_dark
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "Background_Sky_Fill1" && sprite.name == "Background_Sky_Fill1"
        })
        .expect("missing MayaCave2Dark sky fill");
    assert_eq!(sky_fill.fill_color, Some([0x04, 0x0b, 0x12]));
    assert!(sky_fill.atlas.is_none());

    let grass_fill = maya_cave2_dark
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "GroundLayer" && sprite.name == "Grass_fill")
        .expect("missing MayaCave2Dark grass fill");
    assert_eq!(grass_fill.fill_color, Some([0x41, 0x41, 0x28]));
    assert!(grass_fill.atlas.is_none());

    let maya_temple = get_theme("MayaTemple").expect("missing MayaTemple theme");
    let temple_sky = maya_temple
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "Background_Sky" && sprite.name == "Background_Sky")
        .expect("missing MayaTemple sky fill");
    assert_eq!(temple_sky.fill_color, Some([0xfd, 0xf8, 0x7a]));
    assert!(temple_sky.atlas.is_none());
}

#[test]
fn morning_background_sky_keeps_runtime_sky_texture() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_sky = morning
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "Background_Sky" && sprite.name == "Background_Sky")
        .expect("missing Morning sky sprite");

    assert_eq!(
        morning_sky.sky_texture.as_deref(),
        Some("Morning_Sky_Texture.png")
    );
    assert!(morning_sky.atlas.is_none());
    assert!(morning_sky.fill_color.is_none());
}

#[test]
fn maya_tree_fill_keeps_flat_atlas_path() {
    let maya = get_theme("Maya").expect("missing Maya theme");
    let available_fill_names = maya
        .sprites
        .iter()
        .filter(|sprite| sprite.name.to_ascii_lowercase().contains("fill"))
        .map(|sprite| sprite.name.as_str())
        .collect::<Vec<_>>();
    let tree_fill = maya
        .sprites
        .iter()
        .find(|sprite| sprite.name == "Tree_fill")
        .unwrap_or_else(|| {
            panic!("missing Maya tree fill; available fill sprites: {available_fill_names:?}")
        });

    assert!(
        tree_fill.atlas.is_some(),
        "Tree_fill should stay atlas-backed"
    );
    assert!(
        tree_fill.fill_color.is_none(),
        "Tree_fill should not be rewritten into fill_color"
    );
}

#[test]
fn level27_cave_background_override_uses_legacy_transform_path() {
    let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../test_levels/assetbundles/episode_1_levels.unity3d/Level_27_data.bytes");
    let bytes = std::fs::read(&level_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
    let level = parse_level(bytes)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

    let bg_override_text = level
        .objects
        .iter()
        .find_map(|object| match object {
            LevelObject::Prefab(prefab)
                if prefab.name.contains("Background") && prefab.override_data.is_some() =>
            {
                prefab
                    .override_data
                    .as_ref()
                    .map(|data| data.raw_text.clone())
            }
            _ => None,
        })
        .expect("level 27 background override text");

    assert!(
        !bg_override_text.contains("PositionSerializer"),
        "expected Level_27 Cave background override to stay on the legacy Transform path"
    );
    assert!(
        !bg_override_text.contains("childLocalPositions"),
        "expected Level_27 Cave background override to not carry PositionSerializer child positions"
    );
    assert!(
        bg_override_text.contains("Component UnityEngine.Transform"),
        "expected Level_27 Cave background override to include Transform overrides"
    );

    let cave_theme = get_theme("Cave").expect("missing Cave theme");
    let legacy = parse_bg_overrides(&bg_override_text);
    assert!(
        !legacy.sprites.is_empty(),
        "expected Level_27 background override AST to include sprite-level transform overrides"
    );

    let serializer =
        parse_position_serializer_overrides(&bg_override_text, &cave_theme.child_order);
    assert!(
        serializer.groups.is_empty() && serializer.sprites.is_empty(),
        "expected Level_27 Cave background override to avoid serializer-derived group or sprite overrides"
    );

    let runtime = parse_runtime_bg_overrides(&bg_override_text, &cave_theme.child_order);
    for (name, expected) in &legacy.sprites {
        assert_eq!(
            runtime.sprites.get(name),
            Some(expected),
            "expected runtime background override parsing to preserve sprite override {name}"
        );
    }
}

#[test]
fn alpha_blend_can_come_from_materials_camera_layers_and_soft_alpha() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_far_bg = morning
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02")
        .expect("missing Morning far-bg sprite");
    assert!(morning_far_bg.alpha_blend);

    let morning_control = morning
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNear" && sprite.name == "Background_Jungle_01"
        })
        .expect("missing Morning near control sprite");
    assert!(!morning_control.alpha_blend);

    let jungle = get_theme("Jungle").expect("missing Jungle theme");
    let jungle_far_bg = jungle
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02")
        .expect("missing Jungle far-bg sprite");
    assert!(jungle_far_bg.alpha_blend);

    let maya = get_theme("Maya").expect("missing Maya theme");
    let maya_soft_alpha = maya
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Maya_01")
        .expect("missing Maya soft-alpha sprite");
    assert!(maya_soft_alpha.alpha_blend);

    let maya_control = maya
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Maya_02")
        .expect("missing Maya far-layer control sprite");
    assert!(!maya_control.alpha_blend);

    let night = get_theme("Night").expect("missing Night theme");
    let moon = night
        .sprites
        .iter()
        .find(|sprite| sprite.layer == BgLayer::Camera && sprite.name == "Moon")
        .expect("missing Night moon sprite");
    assert!(moon.alpha_blend);

    let halloween = get_theme("Halloween").expect("missing Halloween theme");
    for sprite in halloween
        .sprites
        .iter()
        .filter(|sprite| sprite.layer == BgLayer::Camera)
    {
        assert!(
            sprite.alpha_blend,
            "expected camera-layer alpha blend for {}",
            sprite.name
        );
    }
}

#[test]
fn moon_and_castle_root_groups_use_explicit_camera_tags() {
    let mut matched = Vec::new();

    for prefab_path in
        crate::data::assets::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let asset_path = prefab_path.clone();
        let Some(raw) = read_embedded_text(&asset_path) else {
            continue;
        };
        let Some(prefab) = parse_prefab(&raw) else {
            continue;
        };
        let Some(root_transform) = prefab.transforms.get(&prefab.root_transform_id) else {
            continue;
        };

        for child_id in &root_transform.children {
            let Some(transform) = prefab.transforms.get(child_id) else {
                continue;
            };
            let Some(game_object) = prefab.game_objects.get(&transform.game_object_id) else {
                continue;
            };
            let lower = game_object.name.to_ascii_lowercase();
            if !lower.contains("moon") && !lower.contains("castle") {
                continue;
            }

            matched.push((
                prefab_path.clone(),
                game_object.name.clone(),
                game_object.tag.clone(),
            ));
            assert_eq!(
                explicit_parallax_layer(&game_object.tag),
                Some(BgLayer::Camera),
                "expected explicit camera tag for {} / {}",
                prefab_path,
                game_object.name
            );
        }
    }

    assert!(
        !matched.is_empty(),
        "expected at least one root moon/castle group"
    );
}

#[test]
fn cloud_root_groups_use_explicit_parallax_tags() {
    let mut matched = Vec::new();

    for prefab_path in
        crate::data::assets::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let asset_path = prefab_path.clone();
        let Some(raw) = read_embedded_text(&asset_path) else {
            continue;
        };
        let Some(prefab) = parse_prefab(&raw) else {
            continue;
        };
        let Some(root_transform) = prefab.transforms.get(&prefab.root_transform_id) else {
            continue;
        };

        for child_id in &root_transform.children {
            let Some(transform) = prefab.transforms.get(child_id) else {
                continue;
            };
            let Some(game_object) = prefab.game_objects.get(&transform.game_object_id) else {
                continue;
            };
            if !game_object.name.to_ascii_lowercase().contains("cloud") {
                continue;
            }

            matched.push((
                prefab_path.clone(),
                game_object.name.clone(),
                game_object.tag.clone(),
            ));
            assert!(
                explicit_parallax_layer(&game_object.tag).is_some(),
                "expected explicit parallax tag for {} / {}",
                prefab_path,
                game_object.name
            );
        }
    }

    assert!(
        !matched.is_empty(),
        "expected at least one root cloud group"
    );
}

#[test]
fn standard_layer_root_groups_use_explicit_parallax_tags() {
    let mut matched = Vec::new();

    for prefab_path in
        crate::data::assets::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let asset_path = prefab_path.clone();
        let Some(raw) = read_embedded_text(&asset_path) else {
            continue;
        };
        let Some(prefab) = parse_prefab(&raw) else {
            continue;
        };
        let Some(root_transform) = prefab.transforms.get(&prefab.root_transform_id) else {
            continue;
        };

        for child_id in &root_transform.children {
            let Some(transform) = prefab.transforms.get(child_id) else {
                continue;
            };
            let Some(game_object) = prefab.game_objects.get(&transform.game_object_id) else {
                continue;
            };

            let lower = game_object.name.to_ascii_lowercase();
            let expected = if lower.contains("sky") {
                Some(BgLayer::Sky)
            } else if lower.contains("further") {
                Some(BgLayer::Further)
            } else if lower.contains("foreground") || lower.starts_with("fglayer") {
                Some(BgLayer::Foreground)
            } else if lower.contains("far") {
                Some(BgLayer::Far)
            } else if lower.contains("near") {
                Some(BgLayer::Near)
            } else {
                None
            };

            let Some(expected_layer) = expected else {
                continue;
            };

            matched.push((
                prefab_path.clone(),
                game_object.name.clone(),
                game_object.tag.clone(),
                expected_layer,
            ));
            assert_eq!(
                explicit_parallax_layer(&game_object.tag),
                Some(expected_layer),
                "expected explicit {:?} tag for {} / {}",
                expected_layer,
                prefab_path,
                game_object.name
            );
        }
    }

    assert!(
        !matched.is_empty(),
        "expected at least one standard named root layer group"
    );
}

#[test]
fn ocean_dummy_sprites_keep_their_atlas_tiles() {
    let morning = get_theme("Morning").expect("missing Morning theme");
    let morning_ocean = morning
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Dummy")
        .expect("missing Morning ocean dummy");
    assert!(morning_ocean.atlas.is_some());
    assert!(morning_ocean.fill_color.is_none());

    let jungle = get_theme("Jungle").expect("missing Jungle theme");
    let jungle_ocean = jungle
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Dummy")
        .expect("missing Jungle ocean dummy");
    assert!(jungle_ocean.atlas.is_some());
    assert!(jungle_ocean.fill_color.is_none());

    let maya = get_theme("Maya").expect("missing Maya theme");
    let maya_ocean = maya
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Dummy")
        .expect("missing Maya ocean dummy");
    assert!(maya_ocean.atlas.is_some());
    assert!(maya_ocean.fill_color.is_none());
}

#[test]
fn nested_parallax_tag_starts_its_own_group_context() {
    let Some(theme) = get_theme("MayaCave2Dark") else {
        panic!("missing MayaCave2Dark theme");
    };
    let Some(sprite) = theme
        .sprites
        .iter()
        .find(|sprite| sprite.name == "Background_Sky_Fill1")
    else {
        panic!("missing Background_Sky_Fill1");
    };

    assert_eq!(sprite.parent_group, "Background_Sky_Fill1");
    assert_eq!(sprite.fill_color, Some([0x04, 0x0b, 0x12]));
    assert!(sprite.atlas.is_none());

    let Some(control) = theme
        .sprites
        .iter()
        .find(|sprite| sprite.name == "Background_Sky_Fill2")
    else {
        panic!("missing Background_Sky_Fill2");
    };

    assert_eq!(control.parent_group, "Background_Sky");
}

#[test]
fn maya_cave2dark_fg_uses_sheet_02() {
    let Some(theme) = get_theme("MayaCave2Dark") else {
        panic!("missing MayaCave2Dark theme");
    };

    let fill2 = theme
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "FGLayer" && sprite.name == "Fill2")
        .expect("missing MayaCave2Dark Fill2");
    assert_eq!(fill2.atlas.as_deref(), Some("Background_Maya_Sheet_02.png"));
    assert!(fill2.fill_color.is_none());

    let pillars = theme
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "FGLayer" && sprite.name == "Pillars01")
        .expect("missing MayaCave2Dark Pillars01");
    assert_eq!(
        pillars.atlas.as_deref(),
        Some("Background_Maya_Sheet_02.png")
    );
}

#[test]
fn dark_cave_foreground_renderlast_queue_survives_theme_build() {
    let Some(theme) = get_theme("MayaCaveDark") else {
        panic!("missing MayaCaveDark theme");
    };

    let fill1 = theme
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "FGLayer" && sprite.name == "Fill1")
        .expect("missing MayaCaveDark Fill1");
    assert_eq!(fill1.custom_render_queue, Some(3006));

    let Some(theme2) = get_theme("MayaCave2Dark") else {
        panic!("missing MayaCave2Dark theme");
    };
    let fill2 = theme2
        .sprites
        .iter()
        .find(|sprite| sprite.parent_group == "FGLayer" && sprite.name == "Fill2")
        .expect("missing MayaCave2Dark Fill2");
    assert_eq!(fill2.custom_render_queue, Some(3006));
}

#[test]
fn maya_temple_uses_expected_maya_sheets() {
    let Some(theme) = get_theme("MayaTemple") else {
        panic!("missing MayaTemple theme");
    };

    for sprite in theme.sprites.iter().filter(|sprite| {
        sprite.parent_group == "FGLayer"
            && matches!(
                sprite.name.as_str(),
                "Background_Maya_Temple_FG"
                    | "Background_Maya_Temple_Near_Base"
                    | "Background_Maya_Temple_Near_Top"
            )
    }) {
        assert_eq!(
            sprite.atlas.as_deref(),
            Some("Background_Maya_Sheet_05.png"),
            "unexpected FG atlas for {}",
            sprite.name
        );
    }

    let fg_fill = theme
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "FGLayer" && sprite.name == "Background_Maya_Temple_FG_Fill"
        })
        .expect("missing MayaTemple FG fill");
    assert!(fg_fill.atlas.is_some());
    assert!(fg_fill.fill_color.is_none());

    let near_fill = theme
        .sprites
        .iter()
        .find(|sprite| {
            sprite.parent_group == "BGLayerNearBottom"
                && sprite.name == "Background_Maya_Temple_Near_Fill"
        })
        .expect("missing MayaTemple near fill");
    assert!(near_fill.atlas.is_some());
    assert!(near_fill.fill_color.is_none());

    for sprite in theme.sprites.iter().filter(|sprite| {
        sprite.parent_group == "BGLayerNearBottom"
            && matches!(
                sprite.name.as_str(),
                "Background_Maya_Temple_Near_01"
                    | "Background_Maya_Temple_Near_02"
                    | "Background_Maya_Temple_Near_03"
                    | "Background_Maya_Temple_Near_04"
            )
    }) {
        assert_eq!(
            sprite.atlas.as_deref(),
            Some("Background_Maya_Sheet_04.png"),
            "unexpected near-bottom atlas for {}",
            sprite.name
        );
    }
}
