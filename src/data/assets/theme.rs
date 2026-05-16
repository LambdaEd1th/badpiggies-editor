//! Background theme detection, theme-derived colors and object rendering rules.

use eframe::egui;

const BG_THEME_PATTERNS: &[(&str, &str)] = &[
    ("MM_Cave_02_SET_DARK", "MayaCave2Dark"),
    ("MM_Cave_01_SET_DARK", "MayaCaveDark"),
    ("MM_Cave", "MayaCave"),
    ("MM_Temple", "MayaTemple"),
    ("MM_High", "MayaHigh"),
    ("MM_01_SET", "Maya"),
    ("MayaCave2Dark", "MayaCave2Dark"),
    ("MayaCaveDark", "MayaCaveDark"),
    ("MayaCave", "MayaCave"),
    ("MayaTemple", "MayaTemple"),
    ("MayaHigh", "MayaHigh"),
    ("Maya", "Maya"),
    ("Jungle", "Jungle"),
    ("Plateau", "Plateau"),
    ("Morning", "Morning"),
    ("Forest", "Morning"),
    ("Night", "Night"),
    ("Halloween", "Halloween"),
    ("Cave", "Cave"),
];

pub fn theme_name_for_background_prefab(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();

    for &(pattern, theme) in BG_THEME_PATTERNS {
        if name.contains(&pattern.to_ascii_lowercase()) {
            return Some(theme);
        }
    }

    None
}

fn detect_bg_theme_from_names(object_names: &[String]) -> Option<&'static str> {
    for &(pattern, theme) in BG_THEME_PATTERNS {
        let pattern = pattern.to_ascii_lowercase();
        for name in object_names {
            if name.to_ascii_lowercase().contains(&pattern) {
                return Some(theme);
            }
        }
    }
    None
}

fn detect_bg_theme_from_name(name: &str) -> Option<&'static str> {
    theme_name_for_background_prefab(name)
}

fn background_prefab_ref_index(raw: &str) -> Option<i32> {
    raw.lines().find_map(|line| {
        line.trim_start_matches('\u{feff}')
            .trim()
            .strip_prefix("ObjectReference prefab = ")
            .and_then(|value| value.trim().parse::<i32>().ok())
    })
}

fn background_prefab_name(level_key: &str, bg_override_text: Option<&str>) -> Option<&'static str> {
    let raw = bg_override_text?;
    let ref_index = background_prefab_ref_index(raw)?;
    crate::domain::level::refs::get_background_prefab_ref(level_key, ref_index)
}

/// Detect which background theme to use.
pub fn detect_bg_theme(
    level_key: &str,
    object_names: &[String],
    bg_override_text: Option<&str>,
) -> Option<&'static str> {
    if let Some(prefab_name) = background_prefab_name(level_key, bg_override_text)
        && let Some(theme) = detect_bg_theme_from_name(prefab_name)
    {
        return Some(theme);
    }

    detect_bg_theme_from_names(object_names)
}

#[cfg(test)]
mod tests {
    use super::detect_bg_theme;
    use crate::domain::parser::parse_level;
    use crate::domain::types::LevelObject;
    use std::path::Path;

    fn parse_test_level(relative_path: &str) -> crate::domain::types::LevelData {
        let level_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels")
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
            LevelObject::Prefab(prefab) if prefab.name.contains("Background") => {
                prefab.override_data.as_ref().map(|data| data.raw_text.clone())
            }
            _ => None,
        })
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
        let ice_path = "assetbundles/episode_sandbox_levels.unity3d/Episode_6_Ice Sandbox_data.bytes";
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
}

/// Sky top color per theme.
pub fn sky_top_color(theme: &str) -> egui::Color32 {
    match theme {
        "Jungle" => egui::Color32::from_rgb(0x26, 0xaa, 0xc2),
        "Plateau" => egui::Color32::from_rgb(0x26, 0x78, 0xc2),
        "Night" => egui::Color32::from_rgb(0x43, 0x47, 0x54),
        "Morning" => egui::Color32::from_rgb(0xf7, 0xf8, 0xda),
        "Halloween" => egui::Color32::from_rgb(0x0a, 0x4b, 0x38),
        "Cave" | "MayaCave" | "MayaCaveDark" => egui::Color32::from_rgb(0x11, 0x21, 0x11),
        "Maya" => egui::Color32::from_rgb(0x7d, 0xbf, 0xe9),
        "MayaCave2Dark" => egui::Color32::from_rgb(0x03, 0x12, 0x12),
        "MayaHigh" => egui::Color32::from_rgb(0x7d, 0xbf, 0xe9),
        "MayaTemple" => egui::Color32::from_rgb(0xfd, 0xf8, 0x7b),
        _ => egui::Color32::from_rgb(0x26, 0xaa, 0xc2),
    }
}

/// Ground fill color per theme.
pub fn ground_color(theme: &str) -> egui::Color32 {
    match theme {
        "Jungle" => egui::Color32::from_rgb(0x33, 0x88, 0x44),
        "Plateau" => egui::Color32::from_rgb(0x33, 0x77, 0x66),
        "Night" => egui::Color32::from_rgb(0x20, 0x2d, 0x42),
        "Morning" => egui::Color32::from_rgb(0x3f, 0x4b, 0x5b),
        "Halloween" => egui::Color32::from_rgb(0x3d, 0x2c, 0x4d),
        "Cave" | "MayaCave" | "MayaCaveDark" => egui::Color32::from_rgb(0x11, 0x21, 0x11),
        "Maya" => egui::Color32::from_rgb(0x05, 0x18, 0x26),
        "MayaCave2Dark" => egui::Color32::from_rgb(0x03, 0x12, 0x12),
        "MayaHigh" | "MayaTemple" => egui::Color32::from_rgb(0x05, 0x18, 0x26),
        _ => egui::Color32::from_rgb(0x33, 0x77, 0x66),
    }
}

/// Returns the `_Color` tint that Unity applies to Props sprites via
/// `GenericPropsNight.mat` / `GenericPropsMorning2.mat`.
pub fn props_tint_color(theme: Option<&str>) -> [f32; 4] {
    match theme {
        Some("Night" | "Halloween" | "MayaCaveDark" | "MayaCave2Dark") => {
            [0.7450981, 0.7450981, 1.0, 1.0]
        }
        Some("Morning") => [0.443, 0.532, 0.582, 1.0],
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

/// Sprites that keep their original material and are NOT tinted by
/// GenericPropsNight / GenericPropsMorning2 at runtime in Unity.
pub fn skip_props_tint(name: &str) -> bool {
    name.starts_with("Crystal_")
        || name == "Glow"
        || name.starts_with("Glow_")
        || name.starts_with("GoalArea")
        || name.starts_with("GoalSprite")
        || name.starts_with("Lit")
        || name.starts_with("Secret_")
        || name.starts_with("Star_")
}

/// Named prefab colors for known types.
pub fn get_object_color(name: &str, prefab_index: i16) -> egui::Color32 {
    if name.contains("Background") {
        return egui::Color32::from_rgb(0x2a, 0x4a, 0x2e);
    }
    if name.contains("Goal") {
        return egui::Color32::from_rgb(0xff, 0xd7, 0x00);
    }
    if name.contains("StarBox") {
        return egui::Color32::from_rgb(0xff, 0xeb, 0x3b);
    }
    if name.contains("DessertPlace") {
        return egui::Color32::from_rgb(0xff, 0x98, 0x00);
    }
    if name.contains("TNT") {
        return egui::Color32::from_rgb(0xf4, 0x43, 0x36);
    }
    if name.contains("Pig") {
        return egui::Color32::from_rgb(0xff, 0x69, 0xb4);
    }

    // HSL-based color from prefab index
    let hue = ((prefab_index as i32 * 47) % 360 + 360) % 360;
    hsl_to_rgb(hue as f32, 0.6, 0.55)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> egui::Color32 {
    let a = s * l.min(1.0 - l);
    let f = |n: f32| -> f32 {
        let k = (n + h / 30.0) % 12.0;
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    egui::Color32::from_rgb(
        (f(0.0) * 255.0) as u8,
        (f(8.0) * 255.0) as u8,
        (f(4.0) * 255.0) as u8,
    )
}

/// Whether this object should be skipped during rendering.
pub fn should_skip_render(name: &str) -> bool {
    if name.starts_with("Cloud") && name.ends_with("Set") {
        return true;
    }
    const SKIP_EXACT: &[&str] = &[
        "Props",
        "Prop",
        "Challenges",
        "DessertPlaces",
        "LitArea",
        "reference",
    ];
    if SKIP_EXACT.contains(&name) {
        return true;
    }
    const SKIP_CONTAINS: &[&str] = &[
        "CameraSystem",
        "LevelManager",
        "LevelStart",
        "Background",
        "Decoration ",
        "DontUsePart",
        "Challenge",
        "Tutorial",
        "Achievement",
    ];
    SKIP_CONTAINS.iter().any(|s| name.contains(s))
}
