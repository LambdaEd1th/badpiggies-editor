//! Terrain name → texture filename lookup driven by embedded Unity terrain prefabs.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::domain::level::refs::texture_name_for_guid;

use super::{list_pathnames, read_pathname_text};

const TERRAIN_PREFAB_PREFIX: &str = "e2dTerrain";
const TERRAIN_SCRIPT_GUID: &str = "dec592636f66e19d4a958df992538a81";

#[derive(Default)]
struct TerrainTextureSet {
    fill: Option<String>,
    splat0: Option<String>,
    splat1: Option<String>,
}

fn terrain_texture_sets() -> &'static HashMap<String, TerrainTextureSet> {
    static MAP: OnceLock<HashMap<String, TerrainTextureSet>> = OnceLock::new();
    MAP.get_or_init(build_terrain_texture_sets)
}

fn terrain_aliases() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(build_terrain_aliases)
}

fn build_terrain_texture_sets() -> HashMap<String, TerrainTextureSet> {
    let paths = list_pathnames("Assets/Prefab/", ".prefab");
    let parse = |asset_path: String| {
        let prefab_name = asset_path
            .strip_prefix("Assets/Prefab/")
            .and_then(|n| n.strip_suffix(".prefab"))?;
        if !prefab_name.starts_with(TERRAIN_PREFAB_PREFIX) {
            return None;
        }

        let Some(text) = read_pathname_text(&asset_path) else {
            log::warn!("Missing embedded terrain prefab: {asset_path}");
            return None;
        };
        let Some(texture_set) = parse_terrain_texture_set(&text) else {
            log::warn!("Failed to parse embedded terrain prefab: {asset_path}");
            return None;
        };
        Some((prefab_name.to_string(), texture_set))
    };
    let entries = crate::parallel::filter_map(paths, parse);
    entries.into_iter().collect()
}

fn build_terrain_aliases() -> HashMap<String, String> {
    let mut aliases = HashMap::new();

    for name in terrain_texture_sets().keys() {
        aliases.insert(name.clone(), name.clone());
        aliases
            .entry(canonicalize_name(name))
            .or_insert_with(|| name.clone());
    }

    if terrain_texture_sets().contains_key("e2dTerrainDark") {
        aliases.insert(
            canonicalize_name("e2dTerrainDark cave"),
            "e2dTerrainDark".to_string(),
        );
        aliases.insert(
            canonicalize_name("e2dTerrainDark_cave"),
            "e2dTerrainDark".to_string(),
        );
    }

    aliases
}

fn parse_terrain_texture_set(raw: &str) -> Option<TerrainTextureSet> {
    let doc = raw.split("--- ").skip(1).find(|doc| {
        doc.lines().any(|line| {
            line.trim()
                == format!("m_Script: {{fileID: 11500000, guid: {TERRAIN_SCRIPT_GUID}, type: 3}}")
        })
    })?;

    let fill = doc
        .lines()
        .find_map(|line| line.trim().strip_prefix("FillTexture: "))
        .and_then(texture_name_from_reference)
        .map(str::to_string);

    let mut curve_textures = Vec::new();
    let mut in_curve_textures = false;
    let mut current_curve_texture: Option<String> = None;
    for line in doc.lines() {
        let trimmed = line.trim();
        if trimmed == "CurveTextures:" {
            in_curve_textures = true;
            continue;
        }
        if !in_curve_textures {
            continue;
        }
        if trimmed.starts_with("PlasticEdges:") {
            if let Some(curve_texture) = current_curve_texture.take() {
                curve_textures.push(curve_texture);
            }
            break;
        }
        if let Some(reference) = trimmed.strip_prefix("- texture: ") {
            if let Some(curve_texture) = current_curve_texture.take() {
                curve_textures.push(curve_texture);
            }
            if let Some(texture_name) = texture_name_from_reference(reference) {
                current_curve_texture = Some(texture_name.to_string());
            }
            continue;
        }
    }

    if let Some(curve_texture) = current_curve_texture.take() {
        curve_textures.push(curve_texture);
    }

    Some(TerrainTextureSet {
        fill,
        splat0: curve_textures.first().cloned(),
        splat1: curve_textures.get(1).cloned(),
    })
}

fn texture_name_from_reference(reference: &str) -> Option<&'static str> {
    let guid = extract_guid(reference)?;
    texture_name_for_guid(guid)
}

fn extract_guid(text: &str) -> Option<&str> {
    let start = text.find("guid: ")? + "guid: ".len();
    let rest = &text[start..];
    let end = rest.find(|c| [',', '}'].contains(&c)).unwrap_or(rest.len());
    Some(rest[..end].trim())
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

fn canonicalize_name(name: &str) -> String {
    name.to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}

fn transition_target_prefab(name: &str) -> Option<String> {
    let (position, marker) = [" _ to ", " - to "]
        .into_iter()
        .find_map(|marker| name.find(marker).map(|position| (position, marker)))?;
    let source = name[..position].trim();
    let target = name[position + marker.len()..]
        .trim_start()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .next()?;
    (!source.is_empty() && !target.is_empty()).then(|| format!("{source}_{target}"))
}

fn resolve_terrain_prefab_key(name: &str) -> Option<&'static str> {
    let aliases = terrain_aliases();

    if let Some(target) = transition_target_prefab(name)
        && let Some(resolved) = aliases
            .get(&target)
            .or_else(|| aliases.get(&canonicalize_name(&target)))
    {
        return Some(resolved.as_str());
    }

    let normalized = normalize_terrain(name);
    aliases
        .get(&normalized)
        .or_else(|| aliases.get(&canonicalize_name(&normalized)))
        .map(String::as_str)
}

/// Get the fill texture filename for a terrain object name.
pub fn get_terrain_fill_texture(terrain_name: &str) -> Option<&'static str> {
    let key = resolve_terrain_prefab_key(terrain_name)?;
    terrain_texture_sets().get(key)?.fill.as_deref()
}

/// Get Splat0 (surface/grass) texture filename.
pub fn get_terrain_splat0(terrain_name: &str) -> Option<&'static str> {
    let key = resolve_terrain_prefab_key(terrain_name)?;
    terrain_texture_sets().get(key)?.splat0.as_deref()
}

/// Get Splat1 (outline) texture filename.
pub fn get_terrain_splat1(terrain_name: &str) -> Option<&'static str> {
    let key = resolve_terrain_prefab_key(terrain_name)?;
    terrain_texture_sets().get(key)?.splat1.as_deref()
}

/// Get Splat1 (outline) texture filename for a terrain object name.
pub fn get_terrain_splat1_for_level(_level_key: &str, terrain_name: &str) -> Option<&'static str> {
    get_terrain_splat1(terrain_name)
}
