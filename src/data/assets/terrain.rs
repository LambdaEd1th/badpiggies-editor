//! Terrain name → texture filename maps and lookup helpers.

use std::collections::HashMap;
use std::sync::OnceLock;

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
                "Ground_Rocks_Outline_Texture_03.png",
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
            ("e2dTerrainBase_MM_TempleDarkRock", "Border_Maya_Cave.png"),
            ("e2dTerrainBase_MM_caveSand", "Border_Maya_Cave.png"),
            // Dark variants with different Splat1 than their base
            ("e2dTerrainDark_02", "Ground_Rocks_Outline_Texture.png"),
            ("e2dTerrainDark_03", "Ground_Rocks_Outline_Texture_03.png"),
            (
                "e2dTerrainDark_05_night(150)",
                "Ground_Rocks_Outline_Texture_04.png",
            ),
            ("e2dTerrainDark_MM", "Border_Maya_Cave.png"),
            ("e2dTerrainDark_MM_CaveSand", "Border_Maya_Cave.png"),
            ("e2dTerrainDark_MM_rock", "Border.png"),
            ("e2dTerrainDark_MM_TempleDarkRock", "Border_Maya_Cave.png"),
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
pub fn get_terrain_splat1_for_level(_level_key: &str, terrain_name: &str) -> Option<&'static str> {
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
