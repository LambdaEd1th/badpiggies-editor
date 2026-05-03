//! Asset loading — terrain name→texture maps, sprite data, texture cache, BG theme detection.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui;

/// Embedded game assets (compiled into the binary).
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
pub struct EmbeddedAssets;

/// Read asset bytes by relative path (e.g. "sprites/IngameAtlas.png").
pub fn read_asset(key: &str) -> Option<Cow<'static, [u8]>> {
    EmbeddedAssets::get(key).map(|f| f.data)
}

/// Build an `egui::ColorImage` with gamma-space premultiplied alpha.
///
/// egui 0.33+ uses `Rgba8Unorm` (not sRGB) textures — the shader receives raw
/// bytes and does `vertex_color * tex_gamma` in gamma space.  Therefore stored
/// premultiplied values must use gamma-space premultiply: `r' = r * a / 255`.
fn color_image_premultiplied(size: [usize; 2], rgba: &[u8]) -> egui::ColorImage {
    let pixels = rgba
        .chunks_exact(4)
        .map(|p| {
            let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
            if a == 255 {
                egui::Color32::from_rgba_premultiplied(r, g, b, 255)
            } else if a == 0 {
                egui::Color32::TRANSPARENT
            } else {
                let af = a as f32 * (1.0 / 255.0);
                let rp = (r as f32 * af + 0.5) as u8;
                let gp = (g as f32 * af + 0.5) as u8;
                let bp = (b as f32 * af + 0.5) as u8;
                egui::Color32::from_rgba_premultiplied(rp, gp, bp, a)
            }
        })
        .collect();
    egui::ColorImage::new(size, pixels)
}

// ── Terrain name → texture filename maps ─────────────

/// Terrain prefab name → 512x512 rock fill texture.
fn terrain_fill_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Rocks_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Rocks_Texture_02.png"),
            ("e2dTerrainBase_04", "Ground_Rocks_Texture_04.png"),
            ("e2dTerrainBase_05_night", "Ground_Rocks_Texture.png"),
            ("e2dTerrainBase_Halloween", "Ground_Halloween_Texture.png"),
            ("e2dTerrainBase_MM_Ice", "Ground_Ice_Texture.png"),
            ("e2dTerrainBase_morning", "Ground_Rocks_Texture_06.png"),
            ("e2dTerrainBase_MM_rock", "Ground_Temple_Tile_Texture.png"),
            ("e2dTerrainBase_MM_sand", "Ground_Temple_Rock_Texture.png"),
            ("e2dTerrainBase_MM_TempleDarkRock", "Ground_Temple_cave.png"),
            ("e2dTerrainBase_MM_caveSand", "Ground_Maya_cave_texture.png"),
            // Dark MM variants have their own distinct fill textures
            ("e2dTerrainDark_MM", "Ground_Temple_Dark_Texture.png"),
            (
                "e2dTerrainDark_MM_rock",
                "Ground_Temple_Dark_Texture_02.png",
            ),
            (
                "e2dTerrainDark_MM_TempleDarkRock",
                "Ground_Temple_cave_dark.png",
            ),
            (
                "e2dTerrainDark_MM_CaveSand",
                "Ground_Maya_cave_texture_bg.png",
            ),
        ])
    })
}

/// Terrain prefab → Splat0 (surface/grass) 16x16 texture.
fn terrain_splat0_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Grass_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Grass_Texture.png"),
            ("e2dTerrainBase_04", "Ground_Cave_Texture.png"),
            ("e2dTerrainBase_05_night", "Ground_Grass_Texture_02.png"),
            (
                "e2dTerrainBase_Halloween",
                "Ground_Halloween_Cream_Texture.png",
            ),
            ("e2dTerrainBase_MM_Ice", "Ground_Snow_Texture.png"),
            ("e2dTerrainBase_morning", "Ground_Grass_Texture_3.png"),
            ("e2dTerrainBase_MM_rock", "Ground_Grass_Maya_Texture.png"),
            ("e2dTerrainBase_MM_sand", "Ground_Grass_Maya_Texture.png"),
            (
                "e2dTerrainBase_MM_TempleDarkRock",
                "Ground_Grass_Maya_Texture.png",
            ),
            (
                "e2dTerrainBase_MM_caveSand",
                "Ground_Grass_Maya_Texture.png",
            ),
            // Dark variants with different Splat0 than their base
            ("e2dTerrainDark_MM", "Ground_Grass_Texture.png"),
            ("e2dTerrainDark_MM_CaveSand", "Ground_Grass_Texture.png"),
            ("e2dTerrainDark_MM_rock", "Border_Maya_Cave.png"),
        ])
    })
}

/// Terrain prefab → Splat1 (outline) 16x16 texture.
fn terrain_splat1_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Rocks_Outline_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Rocks_Outline_Texture_02.png"),
            ("e2dTerrainBase_04", "Ground_Rocks_Outline_Texture_04.png"),
            (
                "e2dTerrainBase_05_night",
                "Ground_Rocks_Outline_Texture_05.png",
            ),
            (
                "e2dTerrainBase_Halloween",
                "Ground_Halloween_Outline_Texture.png",
            ),
            ("e2dTerrainBase_MM_Ice", "Ground_Ice_Outline.png"),
            (
                "e2dTerrainBase_morning",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
            ("e2dTerrainBase_MM_rock", "Border.png"),
            ("e2dTerrainBase_MM_sand", "Border.png"),
            (
                "e2dTerrainBase_MM_TempleDarkRock",
                "Border_Maya_Cave.png",
            ),
            (
                "e2dTerrainBase_MM_caveSand",
                "Border_Maya_Cave.png",
            ),
            // Dark variants with different Splat1 than their base
            ("e2dTerrainDark_02", "Ground_Rocks_Outline_Texture.png"),
            ("e2dTerrainDark_03", "Ground_Rocks_Outline_Texture_05.png"),
            (
                "e2dTerrainDark_05_night(150)",
                "Ground_Rocks_Outline_Texture_04.png",
            ),
            ("e2dTerrainDark_MM", "Border_Maya_Cave.png"),
            (
                "e2dTerrainDark_MM_CaveSand",
                "Border_Maya_Cave.png",
            ),
            ("e2dTerrainDark_MM_rock", "Border.png"),
            (
                "e2dTerrainDark_MM_TempleDarkRock",
                "Border_Maya_Cave.png",
            ),
        ])
    })
}

/// Dark terrain variants → base prefab name they share textures with.
fn dark_terrain_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainDark", "e2dTerrainBase"),
            ("e2dTerrainDark_02", "e2dTerrainBase_02"),
            ("e2dTerrainDark_03", "e2dTerrainBase_02"),
            ("e2dTerrainDark_05_night(150)", "e2dTerrainBase_05_night"),
            ("e2dTerrainDark_MM", "e2dTerrainBase_MM_sand"),
            ("e2dTerrainDark_MM_CaveSand", "e2dTerrainBase_MM_caveSand"),
            (
                "e2dTerrainDark_MM_TempleDarkRock",
                "e2dTerrainBase_MM_TempleDarkRock",
            ),
            ("e2dTerrainDark_MM_rock", "e2dTerrainBase_MM_rock"),
            ("e2dTerrainDark Halloween", "e2dTerrainBase_Halloween"),
            ("e2dTerrainDark morning", "e2dTerrainBase_morning"),
            ("e2dTerrainDark_morning", "e2dTerrainBase_morning"),
            ("e2dTerrainDark cave", "e2dTerrainBase"),
            ("e2dTerrainDark_cave", "e2dTerrainBase"),
        ])
    })
}

/// Normalize a binary terrain object name to a known prefab key.
fn normalize_terrain(raw: &str) -> String {
    let mut n = raw.to_string();
    // Strip transition suffixes: " _ to ..." or " - to ..."
    if let Some(pos) = n.find(" _ to ").or_else(|| n.find(" - to ")) {
        n.truncate(pos);
    }
    // Strip trailing annotations like " EP1"
    if let Some(pos) = n.rfind(" EP") {
        n.truncate(pos);
    }
    // Strip trailing " - ..."
    if let Some(pos) = n.rfind(" - ") {
        n.truncate(pos);
    }
    // Strip trailing digit suffixes like " 131x3"
    let trimmed = n.trim_end();
    if let Some(pos) = trimmed.rfind(' ') {
        let tail = &trimmed[pos + 1..];
        if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            n.truncate(pos);
        }
    }
    n = n.trim().to_string();
    // Strip _X/_x suffix
    if n.ends_with("_X") || n.ends_with("_x") {
        n.truncate(n.len() - 2);
    }
    // Strip trailing " -"
    if n.ends_with(" -") {
        n.truncate(n.len() - 2);
        n = n.trim().to_string();
    }
    n
}

/// Resolve terrain name (possibly dark variant) to its base prefab key.
fn resolve_terrain_base(name: &str) -> String {
    let key = normalize_terrain(name);
    dark_terrain_map()
        .get(key.as_str())
        .map(|s| s.to_string())
        .unwrap_or(key)
}

/// Get the fill texture filename for a terrain object name.
pub fn get_terrain_fill_texture(terrain_name: &str) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    // Check direct entry first (dark variants may have their own texture)
    terrain_fill_map().get(key.as_str()).copied().or_else(|| {
        let base = resolve_terrain_base(terrain_name);
        terrain_fill_map().get(base.as_str()).copied()
    })
}

/// Get Splat0 (surface/grass) texture filename.
pub fn get_terrain_splat0(terrain_name: &str) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    // Check direct entry first (dark variants may have their own texture)
    terrain_splat0_map().get(key.as_str()).copied().or_else(|| {
        let base = resolve_terrain_base(terrain_name);
        terrain_splat0_map().get(base.as_str()).copied()
    })
}

/// Get Splat1 (outline) texture filename.
pub fn get_terrain_splat1(terrain_name: &str) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    // Check direct entry first (dark variants may have their own texture)
    terrain_splat1_map().get(key.as_str()).copied().or_else(|| {
        let base = resolve_terrain_base(terrain_name);
        terrain_splat1_map().get(base.as_str()).copied()
    })
}

/// Get Splat1 (outline) texture filename with an Episode 6 MM/Maya fallback rule.
///
/// Unity loader refs remain the source of truth. This helper only applies when
/// those refs are unavailable or resolve to a non-embedded asset. Prefab defaults
/// show that MM rock/sand use `Border.png`, while the cave / temple / dark MM
/// groups use `Border_Maya_Cave.png` except for `e2dTerrainDark_MM_rock`, which
/// uses `Border.png`.
pub fn get_terrain_splat1_for_level(
    _level_key: &str,
    terrain_name: &str,
) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    match key.as_str() {
        "e2dTerrainBase_MM_rock" | "e2dTerrainBase_MM_sand" => Some("Border.png"),
        "e2dTerrainBase_MM_Ice" => Some("Ground_Ice_Outline.png"),
        "e2dTerrainBase_MM_TempleDarkRock"
        | "e2dTerrainBase_MM_caveSand"
        | "e2dTerrainDark_MM"
        | "e2dTerrainDark_MM_CaveSand"
        | "e2dTerrainDark_MM_TempleDarkRock" => Some("Border_Maya_Cave.png"),
        "e2dTerrainDark_MM_rock" => Some("Border.png"),
        _ => get_terrain_splat1(terrain_name),
    }
}

/// Some Maya cave / temple / dark prefabs should keep their prefab-authored
/// Border splat1 even when level refs point at a shared outline texture.
pub fn terrain_splat1_prefers_prefab_over_level_refs(terrain_name: &str) -> bool {
    let key = normalize_terrain(terrain_name);
    matches!(
        key.as_str(),
        "e2dTerrainBase_MM_rock"
            | "e2dTerrainBase_MM_sand"
            | "e2dTerrainBase_MM_TempleDarkRock"
            | "e2dTerrainBase_MM_caveSand"
            | "e2dTerrainDark_MM"
            | "e2dTerrainDark_MM_CaveSand"
            | "e2dTerrainDark_MM_TempleDarkRock"
            | "e2dTerrainDark_MM_rock"
    )
}

/// Whether this is a "dark" terrain (underground fill).
pub fn is_dark_terrain(terrain_name: &str) -> bool {
    let key = normalize_terrain(terrain_name);
    dark_terrain_map().contains_key(key.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        get_terrain_splat1, get_terrain_splat1_for_level,
        terrain_splat1_prefers_prefab_over_level_refs,
    };

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
    fn mm_maya_splat1_level_rule_keeps_prefab_border_textures() {
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
    fn mm_maya_cave_dark_groups_override_shared_level_refs() {
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

// ── Background theme detection ───────────────────────

/// Exact level-key → theme mapping for Episode 6 (Maya) levels.
/// The level binary does NOT contain the background SET name; it stores an
/// opaque prefab reference index that can only be resolved via the Unity
/// loader prefab's `m_references` array.  Heuristic name-matching fails for
/// ~85% of EP6 levels, so we use a hardcoded table derived from the loader
/// analysis.
fn ep6_theme_for_key(key: &str) -> Option<&'static str> {
    // Strip to just the key portion after "episode_6_level_"
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
    // Level binaries use "MM" (Mayan Mischief) prefix for Maya episode backgrounds.
    // Match the specific MM_ variants BEFORE the generic fallbacks.
    ("MM_Cave_02_SET_DARK", "MayaCave2Dark"),
    ("MM_Cave_01_SET_DARK", "MayaCaveDark"),
    ("MM_Cave", "MayaCave"),
    ("MM_Temple", "MayaTemple"),
    ("MM_High", "MayaHigh"),
    ("MM_01_SET", "Maya"),
    // Also keep original patterns for any prefab names that DO contain "Maya".
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
///
/// For Episode 6 levels the theme is determined by a hardcoded lookup table
/// (derived from Unity loader prefab `m_references[4]` GUIDs), because the
/// level binary object names do NOT reliably encode the background variant.
/// For all other episodes, substring matching on object names is sufficient.
pub fn detect_bg_theme(level_key: &str, object_names: &[String]) -> Option<&'static str> {
    // 1. Try exact EP6 level lookup first
    if let Some(theme) = ep6_theme_for_key(level_key) {
        return Some(theme);
    }
    // 2. Try sandbox lookup
    if let Some(theme) = sandbox_theme_for_key(level_key) {
        return Some(theme);
    }
    // 3. Fall back to heuristic name matching (works for EP1-5)
    for name in object_names {
        for &(pattern, theme) in BG_THEME_PATTERNS {
            if name.contains(pattern) {
                return Some(theme);
            }
        }
    }
    None
}

/// Sky top color per theme (sampled from first pixel row of each sky PNG).
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

/// Ground fill color per theme (sampled from Background_*_Sheet fill sprite UV regions).
pub fn ground_color(theme: &str) -> egui::Color32 {
    // Must match the Near_fill color for ocean themes (Jungle/Morning/Maya)
    // so the semi-transparent wave bottom blends seamlessly against it, just
    // like Unity's Camera.backgroundColor fills behind the scene.
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

// ── Props tint per theme (Unity GenericProps material `_Color`) ──

/// Returns the `_Color` tint that Unity applies to Props sprites via
/// `GenericPropsNight.mat` / `GenericPropsMorning2.mat`.
pub fn props_tint_color(theme: Option<&str>) -> [f32; 4] {
    match theme {
        // GenericPropsNight.mat  _Color = (0.745, 0.745, 1, 1)
        Some("Night" | "Halloween" | "MayaCaveDark" | "MayaCave2Dark") => {
            [0.7450981, 0.7450981, 1.0, 1.0]
        }
        // GenericPropsMorning2.mat  _Color = (0.443, 0.532, 0.582, 1)
        Some("Morning") => [0.443, 0.532, 0.582, 1.0],
        // GenericProps.mat  _Color = (1, 1, 1, 1)
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

// ── Color helpers ────────────────────────────────────

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

// ── Texture cache ────────────────────────────────────

/// Texture cache for egui texture handles.
pub struct TextureCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Load a PNG and register it as an egui texture.
    /// `path` is a relative asset key (e.g. "sprites/IngameAtlas.png").
    pub fn load_texture(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let handle = ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a texture from raw RGBA bytes (for control textures decoded from level data).
    #[allow(dead_code)]
    pub fn load_from_rgba(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        pixels: &[u8],
        width: usize,
        height: usize,
    ) -> egui::TextureId {
        if let Some(handle) = self.textures.get(key) {
            return handle.id();
        }
        let color_image = color_image_premultiplied([width, height], pixels);
        let handle = ctx.load_texture(key, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);
        id
    }

    /// Load a PNG texture with repeat (tiling) wrap mode.
    /// `path` is a relative asset key (e.g. "ground/Ground_Rocks_Texture.png").
    pub fn load_texture_repeat(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat wrap and vertical flip.
    /// Flipping matches Unity/Three.js convention where V=0 is image bottom.
    pub fn load_texture_repeat_flipv(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let img = image::imageops::flip_vertical(&img);
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a sprite region cropped from its atlas, keeping original RGBA pixels.
    ///
    /// This matches Unity's `tex2D(_MainTex, uv) * _Color` with `_Color = (1,1,1,1)`,
    /// i.e. the texture passes through unmodified.
    /// `uv_rect` is `[x, y, w, h]` in Unity UV space (V=0 at bottom).
    pub fn load_sprite_crop(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        atlas_path: &str,
        uv_rect: [f32; 4],
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(key) {
            return Some(handle.id());
        }
        let [uv_x, uv_y, uv_w, uv_h] = uv_rect;
        let data = read_asset(atlas_path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let (aw, ah) = (img.width(), img.height());
        // UV → pixel coords (Unity V=0 at bottom)
        let px0 = (uv_x * aw as f32) as u32;
        let py0 = ((1.0 - uv_y - uv_h) * ah as f32) as u32;
        let pw = (uv_w * aw as f32) as u32;
        let ph = (uv_h * ah as f32) as u32;
        let crop = image::imageops::crop_imm(&img, px0, py0, pw, ph).to_image();
        let size = [crop.width() as usize, crop.height() as usize];
        let pixels: Vec<egui::Color32> = crop
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        let color_image = egui::ColorImage::new(size, pixels);
        let handle = ctx.load_texture(key, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);
        Some(id)
    }

    pub fn get(&self, path: &str) -> Option<egui::TextureId> {
        self.textures.get(path).map(|h| h.id())
    }

    /// Get the pixel dimensions of a loaded texture.
    pub fn texture_size(&self, name: &str) -> Option<[usize; 2]> {
        self.textures.get(name).map(|h| h.size())
    }
}
