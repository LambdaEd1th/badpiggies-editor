//! Background theme detection, theme-derived colors and object rendering rules.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::domain::prefab_override::parse_override_text;

#[cfg(test)]
use crate::domain::prefab_override::OverrideNode;

const BG_THEME_PRIORITY: &[&str] = &[
    "MayaCave2Dark",
    "MayaCaveDark",
    "MayaCave",
    "MayaTemple",
    "MayaHigh",
    "Maya",
    "Jungle",
    "Plateau",
    "Morning",
    "Night",
    "Halloween",
    "Cave",
];

pub fn theme_name_for_background_prefab(name: &str) -> Option<&'static str> {
    let normalized = normalize_bg_name(name);

    background_theme_aliases()
        .get(&normalized)
        .copied()
        .or_else(|| canonical_background_theme_name(&normalized))
}

fn normalize_bg_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut last_was_separator = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }

    normalized.trim_matches('_').to_string()
}

fn background_theme_aliases() -> &'static HashMap<String, &'static str> {
    static ALIASES: OnceLock<HashMap<String, &'static str>> = OnceLock::new();

    ALIASES.get_or_init(build_background_theme_aliases)
}

fn build_background_theme_aliases() -> HashMap<String, &'static str> {
    let mut aliases = HashMap::new();

    for prefab_path in super::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let filename = prefab_path
            .strip_prefix("Assets/Resources/environment/background/")
            .unwrap_or(prefab_path.as_str());
        let Some(prefab_name) = Path::new(filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
        else {
            continue;
        };
        let normalized_prefab_name = normalize_bg_name(prefab_name);
        let Some(theme_name) = theme_name_from_background_prefab_stem(&normalized_prefab_name)
        else {
            continue;
        };

        aliases.insert(normalized_prefab_name, theme_name);
        aliases.insert(normalize_bg_name(filename), theme_name);
        aliases.insert(normalize_bg_name(&prefab_path), theme_name);
    }

    aliases
}

fn theme_name_from_background_prefab_stem(normalized: &str) -> Option<&'static str> {
    let normalized = normalized.strip_prefix("background_").unwrap_or(normalized);

    match normalized {
        "cave_01_set_1" => Some("Cave"),
        "forest_01_set_1" => Some("Morning"),
        "halloween" => Some("Halloween"),
        "jungle_01_set" => Some("Jungle"),
        "mm_01_set" => Some("Maya"),
        "mm_cave_01_set" => Some("MayaCave"),
        "mm_cave_01_set_dark" => Some("MayaCaveDark"),
        "mm_cave_02_set_dark" => Some("MayaCave2Dark"),
        "mm_high_01_set" => Some("MayaHigh"),
        "mm_temple_01_set_01" => Some("MayaTemple"),
        "night_01_set_1" => Some("Night"),
        "plateau_01_set" => Some("Plateau"),
        _ => None,
    }
}

fn canonical_background_theme_name(normalized: &str) -> Option<&'static str> {
    BG_THEME_PRIORITY
        .iter()
        .copied()
        .find(|theme_name| normalize_bg_name(theme_name) == normalized)
}

fn looks_like_background_name(normalized: &str) -> bool {
    normalized.starts_with("background_")
        || normalized.starts_with("background")
        || normalized.starts_with("bg_")
}

fn looks_like_cloud_set_name(normalized: &str) -> bool {
    normalized.starts_with("cloud") && normalized.ends_with("set")
}

fn theme_priority(theme: &str) -> usize {
    BG_THEME_PRIORITY
        .iter()
        .position(|candidate| *candidate == theme)
        .unwrap_or(usize::MAX)
}

fn detect_bg_theme_from_names_bple_order(
    object_names: &[String],
    is_dark_level: bool,
) -> Option<&'static str> {
    let has_object = |target: &str| {
        object_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(target))
    };

    if has_object("Background_Jungle_01_SET") {
        return detect_bg_theme_from_name("Background_Jungle_01_SET");
    }
    if has_object("Background_Plateau_01_SET") {
        return detect_bg_theme_from_name("Background_Plateau_01_SET");
    }
    if is_dark_level || has_object("Background_Cave_01_SET 1") {
        return detect_bg_theme_from_name("Background_Cave_01_SET 1");
    }
    if has_object("Background_Night_01_SET 1") {
        return detect_bg_theme_from_name("Background_Night_01_SET 1");
    }
    if has_object("Background_Forest_01_SET 1") {
        return detect_bg_theme_from_name("Background_Forest_01_SET 1");
    }
    if has_object("Background_Halloween") {
        return detect_bg_theme_from_name("Background_Halloween");
    }
    if has_object("Background_MM_01_SET") {
        return detect_bg_theme_from_name("Background_MM_01_SET");
    }
    if has_object("Background_MM_Temple_01_SET_01") {
        return detect_bg_theme_from_name("Background_MM_Temple_01_SET_01");
    }

    None
}

fn detect_bg_theme_from_names(
    object_names: &[String],
    is_dark_level: bool,
) -> Option<&'static str> {
    if let Some(theme) = detect_bg_theme_from_names_bple_order(object_names, is_dark_level) {
        return Some(theme);
    }

    // Prefer explicit Background*/bg_* objects over decorative cloud set names.
    // This mirrors Unity's runtime intent where ambient/background selection is
    // driven by concrete background objects.
    let mut best_background_match = None;
    for name in object_names {
        let normalized = normalize_bg_name(name);
        if !looks_like_background_name(&normalized) {
            continue;
        }
        if let Some(theme) = theme_name_for_background_prefab(name) {
            match best_background_match {
                Some(current) if theme_priority(current) <= theme_priority(theme) => {}
                _ => best_background_match = Some(theme),
            }
        }
    }
    if best_background_match.is_some() {
        return best_background_match;
    }

    // Fallback for levels with no explicit Background* root (legacy/edge cases).
    // Cloud sets are visual decoration and not part of the BPLE background
    // branch chain, so they are excluded from theme detection.
    let mut best_match = None;

    for name in object_names {
        let normalized = normalize_bg_name(name);
        if looks_like_cloud_set_name(&normalized) {
            continue;
        }
        if let Some(theme) = theme_name_for_background_prefab(name) {
            if theme == "Cave" && !looks_like_background_name(&normalized) {
                continue;
            }
            match best_match {
                Some(current) if theme_priority(current) <= theme_priority(theme) => {}
                _ => best_match = Some(theme),
            }
        }
    }

    best_match
}

fn detect_bg_theme_from_name(name: &str) -> Option<&'static str> {
    theme_name_for_background_prefab(name)
}

fn background_prefab_ref_index(raw: &str) -> Option<i32> {
    parse_override_text(raw)
        .iter()
        .find_map(|root| {
            root.find_descendant(&|node| {
                node.node_type == "ObjectReference" && node.name == "prefab"
            })
        })
        .and_then(|node| node.value_as_i32())
}

fn background_override_root_name(raw: &str) -> Option<String> {
    parse_override_text(raw)
        .into_iter()
        .next()
        .map(|node| node.name)
}

fn background_prefab_name(level_key: &str, bg_override_text: Option<&str>) -> Option<&'static str> {
    let raw = bg_override_text?;
    let ref_index = background_prefab_ref_index(raw)?;
    crate::domain::level::refs::get_background_prefab_ref(level_key, ref_index)
}

fn detection_name_candidates(
    level_key: &str,
    object_names: &[String],
    bg_override_text: Option<&str>,
) -> Vec<String> {
    let mut names = object_names.to_vec();

    if let Some(raw) = bg_override_text {
        if let Some(prefab_name) = background_prefab_name(level_key, Some(raw)) {
            names.push(prefab_name.to_string());
        }
        if let Some(root_name) = background_override_root_name(raw) {
            names.push(root_name);
        }
    }

    names
}

/// Detect which background theme to use.
#[allow(dead_code)]
pub fn detect_bg_theme(
    level_key: &str,
    object_names: &[String],
    bg_override_text: Option<&str>,
) -> Option<&'static str> {
    detect_bg_theme_with_dark_level(level_key, object_names, bg_override_text, false)
}

pub fn detect_bg_theme_with_dark_level(
    level_key: &str,
    object_names: &[String],
    bg_override_text: Option<&str>,
    is_dark_level: bool,
) -> Option<&'static str> {
    // BackgroundObject's serialized prefab reference is the concrete Unity
    // background selection. In particular, EP6 dark levels set m_darkLevel
    // for the lighting overlay while still using the MayaCave2Dark prefab.
    // Only use the generic Cave branch when no explicit prefab can be resolved.
    if let Some(theme) = bg_override_text.and_then(|raw| {
        badpiggies_editor_core::data::assets::theme_name_for_background_override(level_key, raw)
    }) {
        return Some(theme);
    }

    let names = detection_name_candidates(level_key, object_names, bg_override_text);
    detect_bg_theme_from_names(&names, is_dark_level)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        background_override_root_name, background_prefab_name, background_prefab_ref_index,
        detect_bg_theme, detect_bg_theme_with_dark_level, get_object_color, ground_color,
        ground_color_candidate_rgb, is_ground_color_candidate_sprite, props_tint_color_for_prefab,
        should_skip_render, sky_top_color, theme_name_for_background_prefab,
    };
    use crate::domain::level::refs::{get_prefab_override, level_key_from_filename};
    use crate::domain::parser::parse_level;
    use crate::domain::types::LevelObject;
    use std::path::Path;

    fn parse_test_level(relative_path: &str) -> crate::domain::types::LevelData {
        let level_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../test_levels")
            .join(relative_path);
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()))
    }

    fn parsed_object_names(relative_path: &str) -> Vec<String> {
        let level = parse_test_level(relative_path);

        level
            .objects
            .iter()
            .map(|object| match object {
                LevelObject::Prefab(prefab) => prefab.name.clone(),
                LevelObject::Parent(parent) => parent.name.clone(),
            })
            .collect()
    }

    fn parsed_bg_override(relative_path: &str) -> Option<String> {
        let level = parse_test_level(relative_path);

        level.objects.iter().find_map(|object| match object {
            LevelObject::Prefab(prefab) if prefab.name.contains("Background") => prefab
                .override_data
                .as_ref()
                .map(|data| data.raw_text.clone()),
            _ => None,
        })
    }

    fn collect_test_level_paths(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_test_level_paths(&path, out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("bytes") {
                continue;
            }
            let Ok(relative) = path
                .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test_levels"))
            else {
                continue;
            };
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    fn all_test_level_paths() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test_levels");
        let mut paths = Vec::new();
        collect_test_level_paths(&root, &mut paths);
        paths.sort();
        paths
    }

    #[test]
    fn prefab_asset_lookups_follow_sprite_name_normalization() {
        assert_color_close(
            props_tint_color_for_prefab("Twig_1 (2)"),
            [112.0 / 255.0, 135.0 / 255.0, 148.0 / 255.0, 1.0],
        );
        assert_eq!(
            get_object_color("Twig_1 (2)", 0, true),
            crate::gpu2d::Color32::from_rgb(112, 135, 148)
        );
        assert!(should_skip_render("CloudNightSet01"));
        assert!(!should_skip_render("GoalArea_01 (2)"));
    }

    fn assert_color_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-6,
                "expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn episode6_background_override_refs_drive_theme_detection() {
        let cave_path = "assetbundles/episode_6_levels.unity3d/episode_6_level_5_data.bytes";
        let cave_names = parsed_object_names(cave_path);
        assert_eq!(
            detect_bg_theme(
                "episode_6_level_5_data",
                &cave_names,
                parsed_bg_override(cave_path).as_deref(),
            ),
            Some("MayaCave2Dark")
        );

        let high_path = "assetbundles/episode_6_levels.unity3d/episode_6_level_10_data.bytes";
        let high_names = parsed_object_names(high_path);
        assert_eq!(
            detect_bg_theme(
                "episode_6_level_10_data",
                &high_names,
                parsed_bg_override(high_path).as_deref(),
            ),
            Some("MayaHigh")
        );

        let temple_path = "assetbundles/episode_6_levels.unity3d/episode_6_level_15_data.bytes";
        let temple_names = parsed_object_names(temple_path);
        assert_eq!(
            detect_bg_theme(
                "episode_6_level_15_data",
                &temple_names,
                parsed_bg_override(temple_path).as_deref(),
            ),
            Some("MayaTemple")
        );
    }

    #[test]
    fn episode6_sandbox_background_override_refs_drive_theme_detection() {
        let ice_path =
            "assetbundles/episode_sandbox_levels.unity3d/Episode_6_Ice Sandbox_data.bytes";
        let ice_names = parsed_object_names(ice_path);
        assert_eq!(
            detect_bg_theme(
                "Episode_6_Ice Sandbox_data",
                &ice_names,
                parsed_bg_override(ice_path).as_deref(),
            ),
            Some("MayaTemple")
        );

        let tower_path =
            "assetbundles/episode_sandbox_levels.unity3d/Episode_6_Tower Sandbox_data.bytes";
        let tower_names = parsed_object_names(tower_path);
        assert_eq!(
            detect_bg_theme(
                "Episode_6_Tower Sandbox_data",
                &tower_names,
                parsed_bg_override(tower_path).as_deref(),
            ),
            Some("Maya")
        );
    }

    #[test]
    fn level_sandbox_10_prefers_background_forest_over_cloud_plateau_set() {
        let path = "assetbundles/episode_sandbox_levels.unity3d/Level_Sandbox_10_data.bytes";
        let names = parsed_object_names(path);
        assert_eq!(
            detect_bg_theme(
                "Level_Sandbox_10_data",
                &names,
                parsed_bg_override(path).as_deref(),
            ),
            Some("Morning")
        );
    }

    #[test]
    fn dark_level_flag_matches_bple_cave_branch() {
        let names = vec!["CloudPlateauSet".to_string(), "SomeOtherObject".to_string()];
        assert_eq!(
            detect_bg_theme_with_dark_level("level_key", &names, None, true),
            Some("Cave")
        );
    }

    #[test]
    fn episode6_dark_background_prefab_beats_generic_cave_fallback() {
        let path = "assetbundles/episode_6_levels.unity3d/episode_6_level_5_data.bytes";
        let names = parsed_object_names(path);
        assert_eq!(
            detect_bg_theme_with_dark_level(
                "episode_6_level_5_data",
                &names,
                parsed_bg_override(path).as_deref(),
                true,
            ),
            Some("MayaCave2Dark")
        );
    }

    #[test]
    fn bple_branch_order_prefers_jungle_when_multiple_backgrounds_exist() {
        let names = vec![
            "Background_MM_Temple_01_SET_01".to_string(),
            "Background_MM_01_SET".to_string(),
            "Background_Forest_01_SET 1".to_string(),
            "Background_Night_01_SET 1".to_string(),
            "Background_Cave_01_SET 1".to_string(),
            "Background_Plateau_01_SET".to_string(),
            "Background_Jungle_01_SET".to_string(),
        ];

        assert_eq!(detect_bg_theme("level_key", &names, None), Some("Jungle"));
    }

    #[test]
    fn cloud_set_aliases_are_fallback_only_when_no_background_object_exists() {
        let only_cloud = vec!["CloudPlateauSet".to_string()];
        assert_eq!(detect_bg_theme("level_key", &only_cloud, None), None);

        let cloud_plus_background = vec![
            "CloudPlateauSet".to_string(),
            "Background_Forest_01_SET 1".to_string(),
        ];
        assert_eq!(
            detect_bg_theme("level_key", &cloud_plus_background, None),
            Some("Morning")
        );
    }

    #[test]
    fn background_theme_name_normalizes_known_name_forms() {
        assert_eq!(
            theme_name_for_background_prefab("background_mm_cave_02_set_dark.prefab"),
            Some("MayaCave2Dark")
        );
        assert_eq!(
            theme_name_for_background_prefab("Background_Cave_01_SET 1"),
            Some("Cave")
        );
        assert_eq!(
            theme_name_for_background_prefab("background_forest_01_set 1"),
            Some("Morning")
        );
        assert_eq!(
            theme_name_for_background_prefab("MayaHigh"),
            Some("MayaHigh")
        );
        assert_eq!(theme_name_for_background_prefab("CloudJungleSet"), None);
        assert_eq!(theme_name_for_background_prefab("CloudLPASet"), None);
        assert_eq!(theme_name_for_background_prefab("backgroundobject"), None);
    }

    #[test]
    fn embedded_background_prefab_paths_resolve_to_theme_names() {
        for prefab_path in crate::data::assets::list_pathnames(
            "Assets/Resources/environment/background/",
            ".prefab",
        ) {
            let prefab_name = Path::new(&prefab_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("background prefab stem");

            if super::normalize_bg_name(prefab_name) == "backgroundobject" {
                assert_eq!(theme_name_for_background_prefab(&prefab_path), None);
                continue;
            }

            assert!(
                theme_name_for_background_prefab(&prefab_path).is_some(),
                "missing theme alias for {prefab_path}"
            );
            assert!(
                theme_name_for_background_prefab(prefab_name).is_some(),
                "missing theme alias for {prefab_name}"
            );
        }
    }

    #[test]
    fn object_name_fallback_ignores_generic_cave_names() {
        let names = vec![
            "CaveGrid".to_string(),
            "Background_Jungle_01_SET".to_string(),
        ];

        assert_eq!(detect_bg_theme("level_key", &names, None), Some("Jungle"));
    }

    #[test]
    fn object_name_fallback_keeps_jungle_ahead_of_legacy_cave_hint() {
        let names = vec![
            "Background_Cave_01_SET 1".to_string(),
            "Background_Jungle_01_SET".to_string(),
        ];

        assert_eq!(detect_bg_theme("level_key", &names, None), Some("Jungle"));
    }

    #[test]
    fn props_tint_colors_can_come_from_prefab_materials() {
        assert_color_close(
            props_tint_color_for_prefab("Mushroom_06"),
            [190.0 / 255.0, 190.0 / 255.0, 1.0, 1.0],
        );
        assert_color_close(
            props_tint_color_for_prefab("Twig_1"),
            [112.0 / 255.0, 135.0 / 255.0, 148.0 / 255.0, 1.0],
        );
        assert_color_close(props_tint_color_for_prefab("Rock_01"), [1.0, 1.0, 1.0, 1.0]);
        assert_color_close(
            props_tint_color_for_prefab("Shell_01"),
            [1.0, 1.0, 1.0, 1.0],
        );
        assert_color_close(
            props_tint_color_for_prefab("Bottle_01"),
            [1.0, 1.0, 1.0, 1.0],
        );
        assert!(!super::props_tint_is_alpha_blend("Shell_01"));
        assert!(!super::props_tint_is_alpha_blend("Bottle_01"));
    }

    #[test]
    fn themed_props_tints_can_use_alpha_blend_material_variants() {
        assert!(super::props_tint_is_alpha_blend("Mushroom_06"));
        assert!(super::props_tint_is_alpha_blend("Twig_1"));
        assert!(super::props_tint_is_alpha_blend("Cave_Mushroom_06"));
    }

    #[test]
    fn object_colors_can_come_from_prefab_materials() {
        assert_eq!(
            get_object_color("Mushroom_06", 0, true),
            crate::gpu2d::Color32::from_rgb(190, 190, 255)
        );
        assert_eq!(
            get_object_color("Twig_1", 0, true),
            crate::gpu2d::Color32::from_rgb(112, 135, 148)
        );
    }

    #[test]
    fn object_colors_keep_generic_hsl_fallback_when_prefab_materials_are_missing() {
        assert_eq!(
            get_object_color("MissingPrefab", 0, false),
            super::hsl_to_rgb(0.0, 0.6, 0.55)
        );
    }

    #[test]
    fn textured_object_colors_default_to_white_without_material_tint() {
        assert_eq!(
            get_object_color("Rock_01", 7, true),
            crate::gpu2d::Color32::WHITE
        );
        assert_eq!(
            get_object_color("StarBox", 7, true),
            crate::gpu2d::Color32::WHITE
        );
        assert_eq!(
            get_object_color("DynamicStarBox", 7, true),
            crate::gpu2d::Color32::WHITE
        );
        assert_eq!(
            get_object_color("TNT_Box", 7, true),
            crate::gpu2d::Color32::WHITE
        );
    }

    #[test]
    fn skip_render_can_come_from_prefab_visual_presence() {
        assert!(should_skip_render("CameraSystem"));
        assert!(should_skip_render("LevelStart"));
        assert!(should_skip_render("LevelManager"));
        assert!(should_skip_render("LitArea"));
        assert!(should_skip_render("CloudNightSet"));
        assert!(should_skip_render("Tutorial"));
        assert!(should_skip_render("RotationTutorial"));
        assert!(should_skip_render("AreaAchievement"));
        assert!(should_skip_render("CollectBoxesAchievement"));
        assert!(!should_skip_render("GoalArea_01"));
    }

    #[test]
    fn skip_render_can_come_from_embedded_background_prefabs() {
        for name in [
            "BackgroundObject",
            "Background_Cave_01_SET 1",
            "Background_Forest_01_SET 1",
            "Background_Halloween",
            "Background_Jungle_01_SET",
            "Background_MM_01_SET",
            "Background_Night_01_SET 1",
            "Background_Plateau_01_SET",
        ] {
            assert!(
                super::is_background_prefab_container(name),
                "missing background prefab container: {name}"
            );
            assert!(
                should_skip_render(name),
                "expected background prefab container to stay hidden: {name}"
            );
        }
    }

    #[test]
    fn skip_render_can_come_from_challenge_root_scripts() {
        for name in ["BoxChallenge", "DynamicBoxChallenge"] {
            assert!(
                super::prefab_asset_lookup(super::prefab_skip_render_by_root_scripts(), name)
                    .is_some(),
                "missing challenge root-script skip marker: {name}"
            );
            assert!(
                should_skip_render(name),
                "expected challenge root to stay hidden: {name}"
            );
        }
    }

    #[test]
    fn skip_render_legacy_exact_and_decoration_filters_have_no_test_level_hits() {
        let mut hits = Vec::new();

        for relative_path in all_test_level_paths() {
            let level = parse_test_level(&relative_path);
            let filename = Path::new(&relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test level file name");
            let level_key = level_key_from_filename(filename);

            for object in &level.objects {
                let LevelObject::Prefab(prefab) = object else {
                    continue;
                };
                let name =
                    get_prefab_override(&level_key, prefab.prefab_index).unwrap_or(&prefab.name);
                if ["Props", "Prop", "Challenges", "DessertPlaces", "reference"].contains(&name)
                    || name.contains("Decoration ")
                {
                    hits.push(format!("{relative_path}: {name}"));
                }
            }
        }

        assert!(
            hits.is_empty(),
            "unexpected legacy skip_render hits: {hits:?}"
        );
    }

    #[test]
    fn skip_render_challenge_hits_are_asset_backed_in_test_levels() {
        let mut hits = Vec::new();

        for relative_path in all_test_level_paths() {
            let level = parse_test_level(&relative_path);
            let filename = Path::new(&relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test level file name");
            let level_key = level_key_from_filename(filename);

            for object in &level.objects {
                let LevelObject::Prefab(prefab) = object else {
                    continue;
                };
                let name =
                    get_prefab_override(&level_key, prefab.prefab_index).unwrap_or(&prefab.name);
                if name.contains("Challenge") {
                    hits.push(format!("{relative_path}: {name}"));
                    let hidden_by_missing_visuals =
                        super::prefab_asset_lookup(super::prefab_basic_visuals_by_name(), name)
                            == Some(&false);
                    let hidden_by_root_script = super::prefab_asset_lookup(
                        super::prefab_skip_render_by_root_scripts(),
                        name,
                    )
                    .is_some();
                    assert!(
                        hidden_by_missing_visuals || hidden_by_root_script,
                        "challenge hit is not asset-backed: {relative_path}: {name}"
                    );
                    assert!(
                        should_skip_render(name),
                        "challenge hit should stay hidden: {relative_path}: {name}"
                    );
                }
            }
        }

        assert!(
            !hits.is_empty(),
            "expected at least one Challenge hit in test_levels"
        );
    }

    #[test]
    fn skip_props_tint_can_come_from_props_alpha_profile() {
        assert!(super::skip_props_tint("Glow"));
        assert!(super::skip_props_tint("Glow_01"));
        assert!(super::skip_props_tint("Secret_01"));
        assert!(super::skip_props_tint("LitCrystal_01"));
        assert!(!super::skip_props_tint("GoalArea_01"));
        assert!(!super::skip_props_tint("GoalSprite"));
        assert!(!super::skip_props_tint("Star_01"));
        assert!(!super::skip_props_tint("Crystal_01"));
        assert!(!super::skip_props_tint("Rock_01"));
    }

    #[test]
    #[ignore]
    fn debug_skip_props_tint_alpha_candidates() {
        for name in [
            "Glow",
            "Glow_01",
            "GoalSprite",
            "Star_01",
            "Crystal_01",
            "Secret_01",
            "Rock_01",
            "Mushroom_06",
            "Twig_1",
        ] {
            println!("{name:?} -> {:?}", super::props_sprite_alpha_stats(name));
        }
    }

    #[test]
    #[ignore]
    fn debug_skip_props_tint_star_crystal_level_usage() {
        for relative_path in all_test_level_paths() {
            let level = parse_test_level(&relative_path);
            let filename = Path::new(&relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test level file name");
            let level_key = level_key_from_filename(filename);

            for object in &level.objects {
                let LevelObject::Prefab(prefab) = object else {
                    continue;
                };
                let name =
                    get_prefab_override(&level_key, prefab.prefab_index).unwrap_or(&prefab.name);
                if name.starts_with("Star_") || name.starts_with("Crystal_") {
                    println!("{relative_path}: {}", name);
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_skip_props_tint_marker_comparison() {
        for name in [
            "Glow_01",
            "Secret_01",
            "LitCrystal_01",
            "Star_01",
            "Crystal_01",
            "Rock_01",
        ] {
            let asset_path = format!("Assets/Prefab/{name}.prefab");
            let prefab_text = super::super::read_pathname_text(&asset_path);
            let material_guid_prefix = prefab_text
                .as_deref()
                .and_then(super::prefab_material_guid)
                .map(|guid| guid.get(..8).unwrap_or(guid.as_str()).to_string());
            let (root_components, root_scripts) = prefab_text
                .as_deref()
                .and_then(crate::domain::prefab_asset::PrefabAssetDocument::parse)
                .map(|prefab| (prefab.root_component_suffixes(), prefab.root_script_guids()))
                .unwrap_or_default();

            println!(
                "name={} skip={} alpha_skip={} alpha_stats={:?} component_skip={} tint={:?} has_prefab={} material_guid_prefix={:?} root_components={:?} root_scripts={:?}",
                name,
                super::skip_props_tint(name),
                super::sprite_asset_lookup(super::props_alpha_skip_names(), name).is_some(),
                super::props_sprite_alpha_stats(name),
                super::prefab_asset_lookup(
                    super::prefab_skip_props_tint_by_root_components(),
                    name
                )
                .is_some(),
                super::props_tint_color_for_prefab(name),
                prefab_text.is_some(),
                material_guid_prefix,
                root_components,
                root_scripts,
            );
        }
    }

    #[test]
    #[ignore]
    fn debug_skip_props_tint_runtime_override_markers() {
        let mut star_printed = 0usize;
        let mut crystal_printed = 0usize;
        let mut rock_printed = 0usize;

        for relative_path in all_test_level_paths() {
            let level = parse_test_level(&relative_path);
            let filename = Path::new(&relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test level file name");
            let level_key = level_key_from_filename(filename);

            for object in &level.objects {
                let LevelObject::Prefab(prefab) = object else {
                    continue;
                };
                let name =
                    get_prefab_override(&level_key, prefab.prefab_index).unwrap_or(&prefab.name);
                let bucket = if name.starts_with("Star_") {
                    &mut star_printed
                } else if name.starts_with("Crystal_") {
                    &mut crystal_printed
                } else if name == "Rock_01" {
                    &mut rock_printed
                } else {
                    continue;
                };
                if *bucket >= 6 {
                    continue;
                }

                let Some(raw_text) = prefab
                    .override_data
                    .as_ref()
                    .map(|data| data.raw_text.as_str())
                else {
                    println!(
                        "{}: {} override=false object_refs=[] components=[]",
                        relative_path, name
                    );
                    *bucket += 1;
                    if star_printed >= 6 && crystal_printed >= 6 && rock_printed >= 6 {
                        return;
                    }
                    continue;
                };

                let roots = crate::domain::prefab_override::parse_override_text(raw_text);
                let object_refs = roots
                    .iter()
                    .flat_map(super::collect_object_reference_values)
                    .collect::<Vec<_>>();
                let components = roots
                    .iter()
                    .flat_map(|root| {
                        root.children_of_type("Component")
                            .map(|component| component.name.clone())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                println!(
                    "{}: {} override=true object_refs={:?} components={:?}",
                    relative_path, name, object_refs, components
                );
                *bucket += 1;
                if star_printed >= 6 && crystal_printed >= 6 && rock_printed >= 6 {
                    return;
                }
            }
        }

        println!(
            "printed_counts star={} crystal={} rock={}",
            star_printed, crystal_printed, rock_printed
        );
    }

    #[test]
    fn sky_top_colors_can_come_from_sky_assets() {
        assert_eq!(
            sky_top_color("Morning"),
            super::sample_sky_texture_top_color("Morning_Sky_Texture.png")
                .expect("missing Morning sky sample")
        );
        assert_eq!(
            sky_top_color("Maya"),
            crate::gpu2d::Color32::from_rgb(0x7d, 0xbf, 0xe9)
        );
        assert_eq!(
            sky_top_color("MayaTemple"),
            crate::gpu2d::Color32::from_rgb(0xfd, 0xf8, 0x7a)
        );
    }

    #[test]
    fn cave_sky_top_color_matches_backdrop_sprite_visible_top_edge() {
        let theme = crate::data::bg_data::get_theme("Cave").expect("missing Cave theme");
        let sprite = theme
            .sprites
            .iter()
            .find(|sprite| super::is_sky_backdrop_sprite(sprite) && sprite.atlas.is_some())
            .expect("missing Cave sky backdrop atlas sprite");
        let expected = super::sample_bg_atlas_sprite_visible_edge_row_rgb(sprite, true)
            .expect("missing Cave sky top-row sample");

        assert_eq!(
            sky_top_color("Cave"),
            crate::gpu2d::Color32::from_rgb(expected[0], expected[1], expected[2])
        );
    }

    #[test]
    fn morning_cloud_sky_layer_does_not_count_as_backdrop() {
        let theme = crate::data::bg_data::get_theme("Morning").expect("missing Morning theme");
        let cloud_sprite = theme
            .sprites
            .iter()
            .find(|sprite| sprite.parent_group == "BGLayerClouds")
            .expect("missing Morning cloud sprite");

        assert_eq!(cloud_sprite.layer, crate::data::bg_data::BgLayer::Sky);
        assert!(!super::is_sky_backdrop_sprite(cloud_sprite));
    }

    #[test]
    fn ground_colors_can_come_from_ground_assets() {
        assert_eq!(
            ground_color("Jungle"),
            crate::gpu2d::Color32::from_rgb(0x33, 0x88, 0x44)
        );
        assert_eq!(
            ground_color("Plateau"),
            crate::gpu2d::Color32::from_rgb(0x33, 0x77, 0x66)
        );
        assert_eq!(
            ground_color("Night"),
            crate::gpu2d::Color32::from_rgb(0x21, 0x2e, 0x43)
        );
        assert_eq!(
            ground_color("Halloween"),
            crate::gpu2d::Color32::from_rgb(0x2b, 0x26, 0x50)
        );
        assert_eq!(
            ground_color("Morning"),
            crate::gpu2d::Color32::from_rgb(0x0d, 0x14, 0x1e)
        );
        assert_eq!(
            ground_color("Cave"),
            crate::gpu2d::Color32::from_rgb(0x11, 0x21, 0x11)
        );
        assert_eq!(
            ground_color("Maya"),
            crate::gpu2d::Color32::from_rgb(0xa3, 0xa7, 0x49)
        );
        assert_eq!(
            ground_color("MayaCaveDark"),
            crate::gpu2d::Color32::from_rgb(0x11, 0x21, 0x11)
        );
        assert_eq!(
            ground_color("MayaTemple"),
            crate::gpu2d::Color32::from_rgb(0x7e, 0x70, 0x1e)
        );
        assert_eq!(
            ground_color("MayaCave2Dark"),
            crate::gpu2d::Color32::from_rgb(0x04, 0x04, 0x09)
        );
    }

    #[test]
    fn ground_colors_can_come_from_mayahigh_assets() {
        assert_eq!(
            ground_color("MayaHigh"),
            crate::gpu2d::Color32::from_rgb(0xbc, 0xdc, 0xf6)
        );
    }

    #[test]
    fn known_theme_ground_colors_resolve_from_assets() {
        for theme_name in super::BG_THEME_PRIORITY {
            assert!(
                super::resolve_ground_color(theme_name).is_some(),
                "expected {theme_name} ground color to resolve from assets"
            );
        }
    }

    #[test]
    #[ignore]
    fn debug_ground_color_candidates() {
        for theme_name in [
            "Plateau",
            "Night",
            "Halloween",
            "Morning",
            "Maya",
            "MayaHigh",
            "MayaTemple",
            "MayaCave2Dark",
        ] {
            let theme = crate::data::bg_data::get_theme(theme_name)
                .unwrap_or_else(|| panic!("missing {theme_name} theme"));
            let expected = ground_color(theme_name);
            println!(
                "theme={theme_name} hardcoded=[{:02x},{:02x},{:02x}]",
                expected.r(),
                expected.g(),
                expected.b()
            );

            let mut candidates = theme
                .sprites
                .iter()
                .filter(|sprite| is_ground_color_candidate_sprite(sprite))
                .collect::<Vec<_>>();
            candidates.sort_by(|left, right| {
                left.layer
                    .order()
                    .cmp(&right.layer.order())
                    .then_with(|| left.world_z.total_cmp(&right.world_z))
                    .then_with(|| left.world_y.total_cmp(&right.world_y))
                    .then_with(|| left.parent_group.cmp(&right.parent_group))
                    .then_with(|| left.name.cmp(&right.name))
            });

            for sprite in candidates {
                let resolved = ground_color_candidate_rgb(sprite);
                println!(
                    "  layer={:?} group={} name={} y={:.3} z={:.3} fill={:?} atlas={:?} resolved={:?}",
                    sprite.layer,
                    sprite.parent_group,
                    sprite.name,
                    sprite.world_y,
                    sprite.world_z,
                    sprite.fill_color,
                    sprite.atlas,
                    resolved,
                );
            }
        }
    }

    #[test]
    #[ignore]
    fn debug_ground_color_dominant_candidates() {
        for theme_name in ["Night", "Halloween", "Morning"] {
            let theme = crate::data::bg_data::get_theme(theme_name)
                .unwrap_or_else(|| panic!("missing {theme_name} theme"));
            let sprite = theme
                .sprites
                .iter()
                .find(|sprite| super::is_near_fill_ground_sprite(sprite))
                .unwrap_or_else(|| panic!("missing {theme_name} near fill"));

            println!(
                "theme={theme_name} expected=[{:02x},{:02x},{:02x}] avg={:?} dominant={:?}",
                super::legacy_ground_color(theme_name).r(),
                super::legacy_ground_color(theme_name).g(),
                super::legacy_ground_color(theme_name).b(),
                ground_color_candidate_rgb(sprite),
                super::dominant_bg_atlas_sprite_rgb(sprite),
            );
        }
    }

    #[test]
    #[ignore]
    fn debug_ground_color_row_candidates() {
        for theme_name in ["Night", "Halloween", "Morning"] {
            let theme = crate::data::bg_data::get_theme(theme_name)
                .unwrap_or_else(|| panic!("missing {theme_name} theme"));
            let sprite = theme
                .sprites
                .iter()
                .find(|sprite| super::is_near_fill_ground_sprite(sprite))
                .unwrap_or_else(|| panic!("missing {theme_name} near fill"));

            println!(
                "theme={theme_name} expected=[{:02x},{:02x},{:02x}] avg={:?} bottom_visible={:?}",
                super::legacy_ground_color(theme_name).r(),
                super::legacy_ground_color(theme_name).g(),
                super::legacy_ground_color(theme_name).b(),
                ground_color_candidate_rgb(sprite),
                super::sample_bg_atlas_sprite_visible_edge_row_rgb(sprite, false),
            );
        }
    }

    #[test]
    #[ignore]
    fn debug_mayahigh_ground_color_sampling_strategies() {
        let theme_name = "MayaHigh";
        let theme = crate::data::bg_data::get_theme(theme_name)
            .unwrap_or_else(|| panic!("missing {theme_name} theme"));

        println!(
            "theme={theme_name} legacy=[{:02x},{:02x},{:02x}]",
            super::legacy_ground_color(theme_name).r(),
            super::legacy_ground_color(theme_name).g(),
            super::legacy_ground_color(theme_name).b(),
        );

        for sprite_name in [
            "Background_Maya_High_Near_Fill",
            "Background_Maya_High_Near",
            "Background_Maya_High_Further_01",
            "Background_Maya_High_Further_02",
            "Background_Maya_High_Further_03",
            "Foreground_Maya_High_Fill",
            "Foreground_Maya_High_Near",
        ] {
            let sprite = theme
                .sprites
                .iter()
                .find(|sprite| sprite.name == sprite_name)
                .unwrap_or_else(|| panic!("missing {sprite_name}"));
            println!(
                "name={} layer={:?} group={} avg={:?} dominant={:?} top_visible={:?} bottom_visible={:?}",
                sprite.name,
                sprite.layer,
                sprite.parent_group,
                super::ground_color_candidate_rgb(sprite),
                super::dominant_bg_atlas_sprite_rgb(sprite),
                super::sample_bg_atlas_sprite_visible_edge_row_rgb(sprite, true),
                super::sample_bg_atlas_sprite_visible_edge_row_rgb(sprite, false),
            );
        }
    }

    #[test]
    fn episode1_sandbox_cave_background_override_is_detected() {
        let path = "assetbundles/episode_sandbox_levels_2.unity3d/Level_Sandbox_01_data.bytes";
        let names = parsed_object_names(path);
        let bg_override = parsed_bg_override(path).expect("missing BackgroundObject override");

        assert!(
            background_prefab_ref_index(&bg_override).is_none(),
            "expected legacy transform override without prefab ref index"
        );
        assert_eq!(
            background_prefab_name("Level_Sandbox_01_data", Some(&bg_override)),
            None
        );
        assert_eq!(
            background_override_root_name(&bg_override).as_deref(),
            Some("Background_Cave_01_SET 1")
        );
        assert_eq!(
            detect_bg_theme("Level_Sandbox_01_data", &names, Some(&bg_override)),
            Some("Cave")
        );
        assert_eq!(
            detect_bg_theme("Level_Sandbox_01_data", &names, None),
            Some("Jungle")
        );
    }
}

/// Sky top color per theme.
pub fn sky_top_color(theme: &str) -> crate::gpu2d::Color32 {
    sky_top_colors()
        .get(theme)
        .copied()
        .unwrap_or_else(|| crate::gpu2d::Color32::from_rgb(0x26, 0xaa, 0xc2))
}

/// Ground fill color per theme.
pub fn ground_color(theme: &str) -> crate::gpu2d::Color32 {
    ground_colors()
        .get(theme)
        .copied()
        .unwrap_or_else(|| legacy_ground_color(theme))
}

fn ground_colors() -> &'static HashMap<String, crate::gpu2d::Color32> {
    static COLORS: OnceLock<HashMap<String, crate::gpu2d::Color32>> = OnceLock::new();

    COLORS.get_or_init(build_ground_colors)
}

fn build_ground_colors() -> HashMap<String, crate::gpu2d::Color32> {
    BG_THEME_PRIORITY
        .iter()
        .filter_map(|theme_name| {
            resolve_ground_color(theme_name).map(|color| ((*theme_name).to_string(), color))
        })
        .collect()
}

fn resolve_ground_color(theme_name: &str) -> Option<crate::gpu2d::Color32> {
    let theme = crate::data::bg_data::get_theme(theme_name)?;

    match theme_name {
        "Plateau" => resolve_theme_sprite_ground_color(theme, is_grass_fill_ground_sprite),
        "Maya" => resolve_theme_sprite_ground_color(theme, is_beach_ground_sprite),
        "Night" | "Morning" | "Jungle" | "Halloween" | "Cave" | "MayaCave" | "MayaCaveDark"
        | "MayaCave2Dark" | "MayaHigh" | "MayaTemple" => {
            resolve_theme_sprite_ground_color(theme, is_near_fill_ground_sprite)
        }
        _ => None,
    }
}

fn resolve_theme_sprite_ground_color(
    theme: &crate::data::bg_data::BgTheme,
    predicate: impl Fn(&crate::data::bg_data::BgSprite) -> bool,
) -> Option<crate::gpu2d::Color32> {
    theme
        .sprites
        .iter()
        .find(|sprite| predicate(sprite))
        .and_then(ground_color_candidate_rgb)
        .map(color32_from_rgb)
}

fn is_near_fill_ground_sprite(sprite: &crate::data::bg_data::BgSprite) -> bool {
    if sprite.layer != crate::data::bg_data::BgLayer::Near {
        return false;
    }

    let normalized_name = normalize_bg_name(&sprite.name);
    let normalized_group = normalize_bg_name(&sprite.parent_group);

    normalized_group.starts_with("bglayernear")
        && (normalized_name.contains("near_fill") || normalized_name.contains("near_base"))
}

fn is_beach_ground_sprite(sprite: &crate::data::bg_data::BgSprite) -> bool {
    if sprite.layer != crate::data::bg_data::BgLayer::Ground {
        return false;
    }

    normalize_bg_name(&sprite.parent_group) == "beachlayer"
        && normalize_bg_name(&sprite.name) == "beach"
}

fn is_grass_fill_ground_sprite(sprite: &crate::data::bg_data::BgSprite) -> bool {
    sprite.layer == crate::data::bg_data::BgLayer::Ground
        && matches!(sprite.name.as_str(), "Grass_fill" | "Grass_Fill")
}

fn legacy_ground_color(theme: &str) -> crate::gpu2d::Color32 {
    match theme {
        "Jungle" => crate::gpu2d::Color32::from_rgb(0x33, 0x88, 0x44),
        "Plateau" => crate::gpu2d::Color32::from_rgb(0x33, 0x77, 0x66),
        "Night" => crate::gpu2d::Color32::from_rgb(0x20, 0x2d, 0x42),
        "Morning" => crate::gpu2d::Color32::from_rgb(0x3f, 0x4b, 0x5b),
        "Halloween" => crate::gpu2d::Color32::from_rgb(0x3d, 0x2c, 0x4d),
        "Cave" | "MayaCave" | "MayaCaveDark" => crate::gpu2d::Color32::from_rgb(0x11, 0x21, 0x11),
        "Maya" => crate::gpu2d::Color32::from_rgb(0x05, 0x18, 0x26),
        "MayaCave2Dark" => crate::gpu2d::Color32::from_rgb(0x03, 0x12, 0x12),
        "MayaHigh" | "MayaTemple" => crate::gpu2d::Color32::from_rgb(0x05, 0x18, 0x26),
        _ => crate::gpu2d::Color32::from_rgb(0x33, 0x77, 0x66),
    }
}

fn sky_top_colors() -> &'static HashMap<String, crate::gpu2d::Color32> {
    static COLORS: OnceLock<HashMap<String, crate::gpu2d::Color32>> = OnceLock::new();

    COLORS.get_or_init(build_sky_top_colors)
}

fn build_sky_top_colors() -> HashMap<String, crate::gpu2d::Color32> {
    let mut colors = HashMap::new();

    for prefab_path in super::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let Some(theme_name) = theme_name_for_background_prefab(&prefab_path) else {
            continue;
        };
        colors.entry(theme_name.to_string()).or_insert_with(|| {
            resolve_sky_top_color(theme_name)
                .unwrap_or_else(|| crate::gpu2d::Color32::from_rgb(0x26, 0xaa, 0xc2))
        });
    }

    colors
}

fn resolve_sky_top_color(theme_name: &str) -> Option<crate::gpu2d::Color32> {
    let theme = crate::data::bg_data::get_theme(theme_name)?;

    theme
        .sprites
        .iter()
        .filter(|sprite| is_sky_backdrop_sprite(sprite))
        .find_map(|sprite| sprite.sky_texture.as_deref())
        .and_then(sample_sky_texture_top_color)
        .or_else(|| {
            theme
                .sprites
                .iter()
                .filter(|sprite| is_sky_backdrop_sprite(sprite))
                .find_map(|sprite| sprite.fill_color.map(color32_from_rgb))
        })
        .or_else(|| {
            theme
                .sprites
                .iter()
                .filter(|sprite| is_sky_backdrop_sprite(sprite))
                .find_map(|sprite| sample_bg_atlas_sprite_visible_edge_row_rgb(sprite, true))
                .map(color32_from_rgb)
        })
}

fn is_sky_backdrop_sprite(sprite: &crate::data::bg_data::BgSprite) -> bool {
    sprite.sky_texture.is_some() || sprite.parent_group.contains("Background_Sky")
}

fn color32_from_rgb(rgb: [u8; 3]) -> crate::gpu2d::Color32 {
    crate::gpu2d::Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

fn color32_from_rgba(rgba: [u8; 4]) -> crate::gpu2d::Color32 {
    crate::gpu2d::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn sample_sky_texture_top_color(filename: &str) -> Option<crate::gpu2d::Color32> {
    sample_texture_row_color(filename, true)
}

fn sample_texture_row_color(filename: &str, top_row: bool) -> Option<crate::gpu2d::Color32> {
    let data = super::read_pathname(&format!("Assets/Texture2D/{filename}"))
        .or_else(|| super::read_pathname(&format!("Assets/Texture2D/{filename}")))?;
    let image = image::load_from_memory(data.as_ref()).ok()?.to_rgba8();
    let row = if top_row {
        image.rows().next()?
    } else {
        image.rows().next_back()?
    };
    let mut total = [0u64; 3];
    let mut count = 0u64;

    for pixel in row {
        let [red, green, blue, alpha] = pixel.0;
        if alpha == 0 {
            continue;
        }
        total[0] += red as u64;
        total[1] += green as u64;
        total[2] += blue as u64;
        count += 1;
    }

    (count > 0).then(|| {
        crate::gpu2d::Color32::from_rgb(
            (total[0] / count) as u8,
            (total[1] / count) as u8,
            (total[2] / count) as u8,
        )
    })
}

#[cfg(test)]
fn is_ground_color_candidate_sprite(sprite: &crate::data::bg_data::BgSprite) -> bool {
    let normalized_name = normalize_bg_name(&sprite.name);
    let normalized_group = normalize_bg_name(&sprite.parent_group);

    sprite.layer == crate::data::bg_data::BgLayer::Ground
        || normalized_name.contains("grass_fill")
        || normalized_name.contains("near_fill")
        || normalized_name == "fill"
        || normalized_name.contains("ground")
        || normalized_group.contains("ground")
        || normalized_group.contains("near")
}

fn ground_color_candidate_rgb(sprite: &crate::data::bg_data::BgSprite) -> Option<[u8; 3]> {
    sprite
        .fill_color
        .or_else(|| sample_bg_atlas_sprite_rgb(sprite))
}

fn sample_bg_atlas_sprite_rgb(sprite: &crate::data::bg_data::BgSprite) -> Option<[u8; 3]> {
    let atlas_name = sprite.atlas.as_deref()?;
    let data = super::read_pathname(&format!("Assets/Texture2D/{atlas_name}"))
        .or_else(|| super::read_pathname(&format!("Assets/Texture2D/{atlas_name}")))?;
    let image = image::load_from_memory(data.as_ref()).ok()?.to_rgba8();
    let (x0, x1, y0, y1) = bg_sprite_image_bounds(sprite, image.width(), image.height())?;

    average_sprite_region_rgb(&image, x0, x1, y0, y1, true)
        .or_else(|| average_sprite_region_rgb(&image, x0, x1, y0, y1, false))
}

#[cfg(test)]
fn dominant_bg_atlas_sprite_rgb(sprite: &crate::data::bg_data::BgSprite) -> Option<[u8; 3]> {
    let atlas_name = sprite.atlas.as_deref()?;
    let data = super::read_pathname(&format!("Assets/Texture2D/{atlas_name}"))
        .or_else(|| super::read_pathname(&format!("Assets/Texture2D/{atlas_name}")))?;
    let image = image::load_from_memory(data.as_ref()).ok()?.to_rgba8();
    let (x0, x1, y0, y1) = bg_sprite_image_bounds(sprite, image.width(), image.height())?;

    dominant_sprite_region_rgb(&image, x0, x1, y0, y1, true)
        .or_else(|| dominant_sprite_region_rgb(&image, x0, x1, y0, y1, false))
}

fn sample_bg_atlas_sprite_visible_edge_row_rgb(
    sprite: &crate::data::bg_data::BgSprite,
    top_row: bool,
) -> Option<[u8; 3]> {
    let atlas_name = sprite.atlas.as_deref()?;
    let data = super::read_pathname(&format!("Assets/Texture2D/{atlas_name}"))
        .or_else(|| super::read_pathname(&format!("Assets/Texture2D/{atlas_name}")))?;
    let image = image::load_from_memory(data.as_ref()).ok()?.to_rgba8();
    let (x0, x1, y0, y1) = bg_sprite_image_bounds(sprite, image.width(), image.height())?;

    if top_row {
        for y in y0..y1 {
            let avg = average_sprite_region_rgb(&image, x0, x1, y, y.saturating_add(1), true)
                .or_else(|| {
                    average_sprite_region_rgb(&image, x0, x1, y, y.saturating_add(1), false)
                });
            if avg.is_some() {
                return avg;
            }
        }
    } else {
        for y in (y0..y1).rev() {
            let avg = average_sprite_region_rgb(&image, x0, x1, y, y.saturating_add(1), true)
                .or_else(|| {
                    average_sprite_region_rgb(&image, x0, x1, y, y.saturating_add(1), false)
                });
            if avg.is_some() {
                return avg;
            }
        }
    }

    None
}

fn average_sprite_region_rgb(
    image: &image::RgbaImage,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
    opaque_only: bool,
) -> Option<[u8; 3]> {
    let mut total = [0u64; 3];
    let mut count = 0u64;

    for y in y0..y1 {
        for x in x0..x1 {
            let [red, green, blue, alpha] = image.get_pixel(x, y).0;
            if alpha == 0 || (opaque_only && alpha < 128) {
                continue;
            }
            total[0] += red as u64;
            total[1] += green as u64;
            total[2] += blue as u64;
            count += 1;
        }
    }

    (count > 0).then(|| {
        [
            (total[0] / count) as u8,
            (total[1] / count) as u8,
            (total[2] / count) as u8,
        ]
    })
}

#[cfg(test)]
fn dominant_sprite_region_rgb(
    image: &image::RgbaImage,
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
    opaque_only: bool,
) -> Option<[u8; 3]> {
    let mut counts = HashMap::<[u8; 3], usize>::new();

    for y in y0..y1 {
        for x in x0..x1 {
            let [red, green, blue, alpha] = image.get_pixel(x, y).0;
            if alpha == 0 || (opaque_only && alpha < 128) {
                continue;
            }
            *counts.entry([red, green, blue]).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .max_by(|(left_rgb, left_count), (right_rgb, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| left_rgb.cmp(right_rgb))
        })
        .map(|(rgb, _)| rgb)
}

fn bg_sprite_image_bounds(
    sprite: &crate::data::bg_data::BgSprite,
    image_width: u32,
    image_height: u32,
) -> Option<(u32, u32, u32, u32)> {
    if sprite.subdiv <= 0.0 {
        return None;
    }

    let width = image_width as f32;
    let height = image_height as f32;
    let x0 = ((sprite.uv_x / sprite.subdiv) * width).round() as u32;
    let x1 = (((sprite.uv_x + sprite.grid_w) / sprite.subdiv) * width).round() as u32;
    let y0 =
        (((sprite.subdiv - sprite.uv_y - sprite.grid_h) / sprite.subdiv) * height).round() as u32;
    let y1 = (((sprite.subdiv - sprite.uv_y) / sprite.subdiv) * height).round() as u32;
    let x0 = x0.min(image_width);
    let x1 = x1.min(image_width);
    let y0 = y0.min(image_height);
    let y1 = y1.min(image_height);

    (x0 < x1 && y0 < y1).then_some((x0, x1, y0, y1))
}

fn prefab_asset_lookup<'a, T>(entries: &'a HashMap<String, T>, name: &str) -> Option<&'a T> {
    if let Some(value) = entries.get(name) {
        return Some(value);
    }

    if let Some(base) = name.split(" (").next()
        && base != name
        && let Some(value) = entries.get(base)
    {
        return Some(value);
    }

    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != name
        && !trimmed.is_empty()
        && let Some(value) = entries.get(trimmed)
    {
        return Some(value);
    }

    if let Some(pos) = name.rfind('_') {
        let suffix = &name[pos + 1..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            let base = &name[..pos];
            if let Some(value) = entries.get(base) {
                return Some(value);
            }
        }
    }

    None
}

struct PrefabRenderMetadata {
    material_color: Option<[u8; 4]>,
    alpha_blend: bool,
    has_mesh_renderer: bool,
    skip_render: bool,
    skip_props_tint: bool,
}

struct PrefabRenderSource {
    asset_path: String,
    metadata: OnceLock<Option<PrefabRenderMetadata>>,
}

fn prefab_render_sources() -> &'static HashMap<String, PrefabRenderSource> {
    static SOURCES: OnceLock<HashMap<String, PrefabRenderSource>> = OnceLock::new();

    SOURCES.get_or_init(|| {
        super::list_pathnames("Assets/Prefab/", ".prefab")
            .into_iter()
            .filter_map(|asset_path| {
                let filename = asset_path.strip_prefix("Assets/Prefab/")?;
                let name = filename.strip_suffix(".prefab")?.to_string();
                Some((
                    name,
                    PrefabRenderSource {
                        asset_path,
                        metadata: OnceLock::new(),
                    },
                ))
            })
            .collect()
    })
}

fn prefab_render_metadata(name: &str) -> Option<&'static PrefabRenderMetadata> {
    let source = prefab_asset_lookup(prefab_render_sources(), name)?;
    source
        .metadata
        .get_or_init(|| build_prefab_render_metadata(&source.asset_path))
        .as_ref()
}

fn build_prefab_render_metadata(asset_path: &str) -> Option<PrefabRenderMetadata> {
    const SKIP_RENDER_SCRIPT_GUIDS: &[&str] = &[
        "6229298ca21dd3894003b3802b06d98e",
        "7b438279c3f772954a60174359d89e4c",
    ];

    let text = super::read_pathname_text(asset_path)?;
    let material_guid = prefab_material_guid(&text);
    let material_guid_prefix = material_guid
        .as_deref()
        .map(|guid| guid.get(..8).unwrap_or(guid));
    let material_color = material_guid_prefix
        .and_then(crate::domain::level::refs::material_color_rgba_for_guid_prefix);
    let alpha_blend = material_guid_prefix
        .is_some_and(crate::domain::level::refs::material_alpha_blend_for_guid_prefix);
    let prefab = crate::domain::prefab_asset::PrefabAssetDocument::parse(&text);
    let skip_render = prefab.as_ref().is_some_and(|prefab| {
        prefab
            .root_script_guids()
            .iter()
            .any(|guid| SKIP_RENDER_SCRIPT_GUIDS.contains(&guid.as_str()))
    });
    let skip_props_tint = prefab.as_ref().is_some_and(|prefab| {
        prefab
            .root_component_suffixes()
            .iter()
            .any(|suffix| suffix == "PointLightSource" || suffix == "PartSecret")
    });

    Some(PrefabRenderMetadata {
        material_color,
        alpha_blend,
        has_mesh_renderer: prefab_has_mesh_renderer(&text),
        skip_render,
        skip_props_tint,
    })
}

#[cfg(test)]
fn sprite_asset_lookup<'a, T>(entries: &'a HashMap<String, T>, name: &str) -> Option<&'a T> {
    prefab_asset_lookup(entries, name).or_else(|| entries.get(&format!("{name}_01")))
}

fn props_sprite_alpha_stats(name: &str) -> Option<(u8, u64, u64, u64)> {
    let info = crate::data::sprite_db::get_sprite_info(name)?;
    if info.atlas != "Props_Generic_Sheet_01.png" {
        return None;
    }

    let image = props_atlas_image()?;
    let width = image.width() as f32;
    let height = image.height() as f32;
    let x0 = (info.uv.x * width).round() as u32;
    let x1 = ((info.uv.x + info.uv.w) * width).round() as u32;
    let y0 = ((1.0 - info.uv.y - info.uv.h) * height).round() as u32;
    let y1 = ((1.0 - info.uv.y) * height).round() as u32;
    let x0 = x0.min(image.width());
    let x1 = x1.min(image.width());
    let y0 = y0.min(image.height());
    let y1 = y1.min(image.height());
    if x0 >= x1 || y0 >= y1 {
        return None;
    }

    let mut max_alpha = 0u8;
    let mut nonzero = 0u64;
    let mut alpha_ge_128 = 0u64;
    let mut alpha_ge_200 = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            let alpha = image.get_pixel(x, y).0[3];
            max_alpha = max_alpha.max(alpha);
            if alpha > 0 {
                nonzero += 1;
            }
            if alpha >= 128 {
                alpha_ge_128 += 1;
            }
            if alpha >= 200 {
                alpha_ge_200 += 1;
            }
        }
    }

    Some((max_alpha, nonzero, alpha_ge_128, alpha_ge_200))
}

fn props_atlas_image() -> Option<&'static image::RgbaImage> {
    static IMAGE: OnceLock<Option<image::RgbaImage>> = OnceLock::new();

    IMAGE
        .get_or_init(|| {
            let data = super::read_pathname("Assets/Texture2D/Props_Generic_Sheet_01.png")?;
            image::load_from_memory(data.as_ref())
                .ok()
                .map(|image| image.to_rgba8())
        })
        .as_ref()
}

fn props_sprite_has_only_soft_alpha(name: &str) -> bool {
    let Some((max_alpha, _, alpha_ge_128, _)) = props_sprite_alpha_stats(name) else {
        return false;
    };

    max_alpha > 0 && alpha_ge_128 == 0
}

#[cfg(test)]
fn props_alpha_skip_names() -> &'static HashMap<String, bool> {
    static NAMES: OnceLock<HashMap<String, bool>> = OnceLock::new();

    NAMES.get_or_init(build_props_alpha_skip_names)
}

#[cfg(test)]
fn build_props_alpha_skip_names() -> HashMap<String, bool> {
    let mut names = HashMap::new();

    for (name, info) in crate::data::sprite_db::sprite_db() {
        if info.atlas != "Props_Generic_Sheet_01.png" {
            continue;
        }
        if props_sprite_has_only_soft_alpha(name) {
            names.insert(name.clone(), true);
        }
    }

    names
}

/// Returns the `_Color` tint that Unity applies to Props sprites via
/// `GenericProps*.mat` materials serialized on individual prefabs.
pub fn props_tint_color_for_prefab(prefab_name: &str) -> [f32; 4] {
    prefab_render_metadata(prefab_name)
        .and_then(|metadata| metadata.material_color)
        .map(|rgba| {
            [
                rgba[0] as f32 / 255.0,
                rgba[1] as f32 / 255.0,
                rgba[2] as f32 / 255.0,
                1.0,
            ]
        })
        .unwrap_or([1.0, 1.0, 1.0, 1.0])
}

pub fn props_tint_is_alpha_blend(prefab_name: &str) -> bool {
    prefab_render_metadata(prefab_name)
        .map(|metadata| metadata.alpha_blend)
        .unwrap_or(false)
}

fn prefab_material_color_for_prefab(prefab_name: &str) -> Option<crate::gpu2d::Color32> {
    prefab_render_metadata(prefab_name)
        .and_then(|metadata| metadata.material_color)
        .map(color32_from_rgba)
}

fn prefab_material_guid(prefab_text: &str) -> Option<String> {
    for doc in prefab_text.split("--- ").skip(1) {
        let Some(header) = doc.lines().next().map(str::trim) else {
            continue;
        };
        if !header.starts_with("!u!23 ") {
            continue;
        }
        if let Some(guid) = doc.lines().find_map(extract_guid) {
            return Some(guid.to_string());
        }
    }

    None
}

fn prefab_has_mesh_renderer(prefab_text: &str) -> bool {
    prefab_text.split("--- ").skip(1).any(|doc| {
        doc.lines()
            .next()
            .map(str::trim)
            .is_some_and(|header| header.starts_with("!u!23 "))
    })
}

#[cfg(test)]
fn prefab_basic_visuals_by_name() -> &'static HashMap<String, bool> {
    static VISUALS: OnceLock<HashMap<String, bool>> = OnceLock::new();

    VISUALS.get_or_init(build_prefab_basic_visuals_by_name)
}

#[cfg(test)]
fn build_prefab_basic_visuals_by_name() -> HashMap<String, bool> {
    let mut visuals = HashMap::new();

    for prefab_path in super::list_pathnames("Assets/Prefab/", ".prefab") {
        let asset_path = prefab_path.clone();
        let Some(text) = super::read_pathname_text(&asset_path) else {
            continue;
        };
        let filename = prefab_path
            .strip_prefix("Assets/Prefab/")
            .unwrap_or(prefab_path.as_str());
        let prefab_name = filename.strip_suffix(".prefab").unwrap_or(filename);
        visuals.insert(prefab_name.to_string(), prefab_has_mesh_renderer(&text));
    }

    visuals
}

fn background_prefab_names() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();

    NAMES.get_or_init(build_background_prefab_names)
}

fn build_background_prefab_names() -> HashSet<String> {
    let mut names = HashSet::new();

    for prefab_path in super::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let filename = prefab_path
            .strip_prefix("Assets/Resources/environment/background/")
            .unwrap_or(prefab_path.as_str());
        let prefab_name = filename.strip_suffix(".prefab").unwrap_or(filename);
        names.insert(normalize_bg_name(prefab_name));
    }

    names
}

fn is_background_prefab_container(name: &str) -> bool {
    background_prefab_names().contains(&normalize_bg_name(name))
}

#[cfg(test)]
fn prefab_skip_render_by_root_scripts() -> &'static HashMap<String, bool> {
    static PREFABS: OnceLock<HashMap<String, bool>> = OnceLock::new();

    PREFABS.get_or_init(build_prefab_skip_render_by_root_scripts)
}

#[cfg(test)]
fn build_prefab_skip_render_by_root_scripts() -> HashMap<String, bool> {
    const SKIP_RENDER_SCRIPT_GUIDS: &[&str] = &[
        "6229298ca21dd3894003b3802b06d98e",
        "7b438279c3f772954a60174359d89e4c",
    ];

    let mut prefabs = HashMap::new();

    for prefab_path in super::list_pathnames("Assets/Prefab/", ".prefab") {
        let asset_path = prefab_path.clone();
        let Some(text) = super::read_pathname_text(&asset_path) else {
            continue;
        };
        let Some(prefab) = crate::domain::prefab_asset::PrefabAssetDocument::parse(&text) else {
            continue;
        };
        let script_guids = prefab.root_script_guids();
        if !script_guids
            .iter()
            .any(|guid| SKIP_RENDER_SCRIPT_GUIDS.contains(&guid.as_str()))
        {
            continue;
        }

        let filename = prefab_path
            .strip_prefix("Assets/Prefab/")
            .unwrap_or(prefab_path.as_str());
        let prefab_name = filename.strip_suffix(".prefab").unwrap_or(filename);
        prefabs.insert(prefab_name.to_string(), true);
    }

    prefabs
}

#[cfg(test)]
fn prefab_skip_props_tint_by_root_components() -> &'static HashMap<String, bool> {
    static PREFABS: OnceLock<HashMap<String, bool>> = OnceLock::new();

    PREFABS.get_or_init(build_prefab_skip_props_tint_by_root_components)
}

#[cfg(test)]
fn build_prefab_skip_props_tint_by_root_components() -> HashMap<String, bool> {
    let mut prefabs = HashMap::new();

    for prefab_path in super::list_pathnames("Assets/Prefab/", ".prefab") {
        let asset_path = prefab_path.clone();
        let Some(text) = super::read_pathname_text(&asset_path) else {
            continue;
        };
        let Some(prefab) = crate::domain::prefab_asset::PrefabAssetDocument::parse(&text) else {
            continue;
        };
        let suffixes = prefab.root_component_suffixes();
        if !suffixes
            .iter()
            .any(|suffix| suffix == "PointLightSource" || suffix == "PartSecret")
        {
            continue;
        }

        let filename = prefab_path
            .strip_prefix("Assets/Prefab/")
            .unwrap_or(prefab_path.as_str());
        let prefab_name = filename.strip_suffix(".prefab").unwrap_or(filename);
        prefabs.insert(prefab_name.to_string(), true);
    }

    prefabs
}

fn extract_guid(line: &str) -> Option<&str> {
    let (_, rest) = line.split_once("guid: ")?;
    rest.split([',', '}'])
        .next()
        .map(str::trim)
        .filter(|guid| !guid.is_empty())
}

#[cfg(test)]
fn collect_object_reference_values(node: &OverrideNode) -> Vec<i32> {
    let mut values = Vec::new();
    if let Some(value) = node.value_as_i32()
        && node.node_type == "ObjectReference"
    {
        values.push(value);
    }
    for child in &node.children {
        values.extend(collect_object_reference_values(child));
    }
    values
}

/// Sprites that keep their original material and are NOT tinted by
/// GenericPropsNight / GenericPropsMorning2 at runtime in Unity.
pub fn skip_props_tint(name: &str) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(value) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(name)
        .copied()
    {
        return value;
    }

    let value = props_sprite_has_only_soft_alpha(name)
        || prefab_render_metadata(name).is_some_and(|metadata| metadata.skip_props_tint);
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(name.to_string(), value);
    value
}

fn legacy_object_color(_name: &str, prefab_index: i16) -> crate::gpu2d::Color32 {
    // HSL-based color from prefab index
    let hue = ((prefab_index as i32 * 47) % 360 + 360) % 360;
    hsl_to_rgb(hue as f32, 0.6, 0.55)
}

/// Per-object draw color.
pub fn get_object_color(name: &str, prefab_index: i16, textured: bool) -> crate::gpu2d::Color32 {
    if let Some(color) = prefab_material_color_for_prefab(name) {
        return color;
    }

    if textured {
        return crate::gpu2d::Color32::WHITE;
    }

    legacy_object_color(name, prefab_index)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> crate::gpu2d::Color32 {
    let a = s * l.min(1.0 - l);
    let f = |n: f32| -> f32 {
        let k = (n + h / 30.0) % 12.0;
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    crate::gpu2d::Color32::from_rgb(
        (f(0.0) * 255.0) as u8,
        (f(8.0) * 255.0) as u8,
        (f(4.0) * 255.0) as u8,
    )
}

/// Whether this object should be skipped during rendering.
pub fn should_skip_render(name: &str) -> bool {
    if prefab_render_metadata(name).is_some_and(|metadata| !metadata.has_mesh_renderer) {
        return true;
    }

    if is_background_prefab_container(name) {
        return true;
    }

    prefab_render_metadata(name).is_some_and(|metadata| metadata.skip_render)
}
