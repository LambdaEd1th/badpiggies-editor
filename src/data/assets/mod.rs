//! Asset loading — embedded project assets, terrain texture maps, BG theme
//! detection, and the egui texture cache.

mod atlas_materials;
mod embedded;
mod terrain;
mod texture_cache;
mod theme;

pub use atlas_materials::atlas_for_material_guid;
pub use embedded::{list_asset_paths, read_asset, read_asset_text};
pub use terrain::{
    get_terrain_fill_texture, get_terrain_splat0,
    get_terrain_splat1_for_level, is_dark_terrain, terrain_splat1_prefers_prefab_over_level_refs,
};
pub use texture_cache::TextureCache;
pub use theme::{
    detect_bg_theme, get_object_color, ground_color, props_tint_color, should_skip_render,
    skip_props_tint, sky_top_color, theme_name_for_background_prefab,
};

pub fn effect_texture_name_for_material_guid(material_guid: &str) -> Option<&'static str> {
    match material_guid {
        // Glow, wind, fan, and several world particle prefabs all point at the
        // shared Particles_Sheet_01 material family.
        "884b9b90b5f2e49343f6ec0608bc01c9" => Some("Particles_Sheet_01.png"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::terrain::get_terrain_splat1;
    use super::{
        effect_texture_name_for_material_guid, get_terrain_splat1_for_level, list_asset_paths,
        read_asset_text,
        terrain_splat1_prefers_prefab_over_level_refs,
    };

    #[test]
    fn embedded_asset_listing_includes_goal_area_prefab() {
        assert!(list_asset_paths("Prefab/", ".prefab")
            .iter()
            .any(|path| path == "GoalArea_01.prefab"));
    }

    #[test]
    fn embedded_asset_text_reads_animation_clip() {
        let text = read_asset_text("unity/animation/BirdSleep2.anim").expect("missing anim");
        assert!(text.contains("AnimationClip"));
    }

    #[test]
    fn shared_particle_effect_material_guid_maps_to_particles_sheet() {
        assert_eq!(
            effect_texture_name_for_material_guid("884b9b90b5f2e49343f6ec0608bc01c9"),
            Some("Particles_Sheet_01.png")
        );
    }

    #[test]
    fn mm_maya_splat1_defaults_match_prefab_curve_textures() {
        assert_eq!(
            get_terrain_splat1("e2dTerrainBase_MM_rock"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1("e2dTerrainBase_MM_sand"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1("e2dTerrainBase_MM_TempleDarkRock"),
            Some("Border_Maya_Cave.png")
        );
        assert_eq!(
            get_terrain_splat1("e2dTerrainDark_MM_rock"),
            Some("Border.png")
        );
    }

    #[test]
    fn night_outline_defaults_match_prefab_curve_textures() {
        assert_eq!(
            get_terrain_splat1("e2dTerrainBase_05_night"),
            Some("Ground_Rocks_Outline_Texture_03.png")
        );
        assert_eq!(
            get_terrain_splat1("e2dTerrainDark_03"),
            Some("Ground_Rocks_Outline_Texture_03.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("scenario_12_data", "e2dTerrainBase_05_night"),
            Some("Ground_Rocks_Outline_Texture_03.png")
        );
    }

    #[test]
    fn mm_maya_splat1_level_fallback_uses_prefab_defaults() {
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_5_data", "e2dTerrainBase_MM_sand"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("Episode_6_Dark Sandbox_data", "e2dTerrainBase_MM_rock"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_10_data", "e2dTerrainBase_MM_rock"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_18_data", "e2dTerrainDark_MM"),
            Some("Border_Maya_Cave.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_18_data", "e2dTerrainDark_MM_rock"),
            Some("Border.png")
        );
    }

    #[test]
    fn mm_maya_cave_dark_groups_keep_prefab_border_splat1() {
        assert!(terrain_splat1_prefers_prefab_over_level_refs(
            "e2dTerrainBase_MM_rock"
        ));
        assert!(terrain_splat1_prefers_prefab_over_level_refs(
            "e2dTerrainBase_MM_sand"
        ));
        assert!(terrain_splat1_prefers_prefab_over_level_refs(
            "e2dTerrainBase_MM_TempleDarkRock"
        ));
        assert!(terrain_splat1_prefers_prefab_over_level_refs(
            "e2dTerrainDark_MM_rock"
        ));
        assert!(!terrain_splat1_prefers_prefab_over_level_refs(
            "e2dTerrainBase_MM_Ice"
        ));
    }
}
