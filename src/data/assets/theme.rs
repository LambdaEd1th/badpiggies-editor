//! Background theme detection, theme-derived colors and object rendering rules.

use eframe::egui;

/// Exact level-key → theme mapping for Episode 6 (Maya) levels.
fn ep6_theme_for_key(key: &str) -> Option<&'static str> {
    let suffix = key
        .to_ascii_lowercase()
        .replace("episode_6_level_", "")
        .replace("_data", "");
    match suffix.as_str() {
        // Maya (outdoor): levels 1-4, 33-36, star I, IX
        "1" | "2" | "3" | "4" | "33" | "34" | "35" | "36" | "i" | "ix" => Some("Maya"),
        // MayaCave2Dark: levels 5-8, 17-20, 29-32, star II, V, VIII
        "5" | "6" | "7" | "8" | "17" | "18" | "19" | "20" | "29" | "30" | "31" | "32" | "ii"
        | "v" | "viii" => Some("MayaCave2Dark"),
        // MayaHigh: levels 9-12, 25-28, star III, VII
        "9" | "10" | "11" | "12" | "25" | "26" | "27" | "28" | "iii" | "vii" => Some("MayaHigh"),
        // MayaTemple: levels 13-16, 21-24, star IV, VI
        "13" | "14" | "15" | "16" | "21" | "22" | "23" | "24" | "iv" | "vi" => Some("MayaTemple"),
        _ => None,
    }
}

/// Exact sandbox level-key → theme.
fn sandbox_theme_for_key(key: &str) -> Option<&'static str> {
    let lower = key.to_ascii_lowercase();
    if lower.contains("episode_6_dark") {
        Some("MayaCave2Dark")
    } else if lower.contains("episode_6_ice") {
        Some("MayaTemple")
    } else if lower.contains("episode_6_tower") {
        Some("Maya")
    } else {
        None
    }
}

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

/// Detect which background theme to use.
pub fn detect_bg_theme(level_key: &str, object_names: &[String]) -> Option<&'static str> {
    if let Some(theme) = ep6_theme_for_key(level_key) {
        return Some(theme);
    }
    if let Some(theme) = sandbox_theme_for_key(level_key) {
        return Some(theme);
    }
    for name in object_names {
        for &(pattern, theme) in BG_THEME_PATTERNS {
            if name.contains(pattern) {
                return Some(theme);
            }
        }
    }
    None
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
