//! Asset loading — unitypackage-backed project assets, terrain texture maps, BG theme
//! detection, and the egui texture cache.

mod atlas_materials;
mod unitypackage_loader;
mod terrain;
mod texture_cache;
mod theme;

pub use atlas_materials::atlas_for_material_guid;
pub use unitypackage_loader::{
    guid_for_pathname, list_pathnames, pathname_for_guid, read_guid_text, read_pathname,
    read_pathname_text,
};
pub use terrain::{get_terrain_fill_texture, get_terrain_splat0, get_terrain_splat1_for_level};
pub use texture_cache::TextureCache;
pub use theme::{
    detect_bg_theme, get_object_color, ground_color, props_tint_color_for_prefab,
    props_tint_is_alpha_blend, should_skip_render, skip_props_tint, sky_top_color,
    theme_name_for_background_prefab,
};

pub fn effect_texture_name_for_material_guid(material_guid: &str) -> Option<&'static str> {
    crate::domain::level::refs::texture_name_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_texture_name_for_guid(material_guid))
}

pub fn terrain_texture_asset_key(filename: &str) -> Option<String> {
    let path = format!("Assets/Texture2D/{filename}");
    if read_pathname(&path).is_some() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::terrain::get_terrain_splat1;
    use super::{
        effect_texture_name_for_material_guid, get_terrain_splat1_for_level, list_pathnames,
        read_pathname_text,
    };

    #[test]
    fn unitypackage_asset_listing_includes_goal_area_prefab() {
        assert!(
            list_pathnames("Assets/Prefab/", ".prefab")
                .iter()
                .any(|path| path == "Assets/Prefab/GoalArea_01.prefab")
        );
    }

    #[test]
    fn unitypackage_asset_text_reads_animation_clip() {
        let text =
            read_pathname_text("Assets/AnimationClip/BirdSleep2.anim").expect("missing anim");
        assert!(text.contains("AnimationClip"));
    }

    #[test]
    fn particle_material_guid_resolves_sheet_from_material_asset() {
        assert_eq!(
            effect_texture_name_for_material_guid("884b9b90b5f2e49343f6ec0608bc01c9"),
            Some("Particles_Sheet_01.png")
        );
    }

    #[test]
    fn mm_maya_splat1_defaults_match_prefab_border_textures_for_cave_dark_groups() {
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
    fn mm_maya_splat1_level_fallback_matches_prefab_curve_textures() {
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_5_data", "e2dTerrainBase_MM_sand"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_10_data", "e2dTerrainBase_MM_rock"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level(
                "episode_6_level_18_data",
                "e2dTerrainDark_MM_TempleDarkRock"
            ),
            Some("Border_Maya_Cave.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("Episode_6_Dark Sandbox_data", "e2dTerrainBase_MM_rock"),
            Some("Border.png")
        );
        assert_eq!(
            get_terrain_splat1_for_level("episode_6_level_10_data", "e2dTerrainBase_MM_Ice"),
            Some("Ground_Ice_Outline.png")
        );
    }
}
