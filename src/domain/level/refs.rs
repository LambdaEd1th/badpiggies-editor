//! Level refs database.
//!
//! Terrain texture refs are rebuilt at runtime from embedded Unity loader prefabs.
//! Prefab names are rebuilt at runtime from embedded loader prefabs and prefab
//! YAMLs by resolving each loader `m_prefabs` entry's GameObject fileID.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::data::assets;
use crate::diagnostics::error::{AppError, AppResult};

const LOADER_MANIFEST_ASSET: &str = "unity/levels/loader-manifest.txt";
const LOADER_ASSET_PREFIX: &str = "unity/levels/";
const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_ASSET_PREFIX: &str = "unity/prefabs/";

#[derive(Debug, Clone)]
struct LoaderReference {
    guid: Option<String>,
    ref_type: i32,
}

#[derive(Debug, Clone)]
struct ParsedLoaderPrefab {
    level_key: String,
    prefab_file_ids: Vec<String>,
    references: Vec<LoaderReference>,
}

/// Parsed lookup: level_key → (ref_index → texture_filename)
type RefsMap = HashMap<String, HashMap<i32, String>>;
/// Parsed lookup: level_key → (prefab_index → corrected_name)
type PrefabsMap = HashMap<String, HashMap<i16, String>>;

struct LevelRefsData {
    refs: RefsMap,
    prefabs: PrefabsMap,
}

fn data() -> &'static LevelRefsData {
    static INSTANCE: OnceLock<LevelRefsData> = OnceLock::new();
    INSTANCE.get_or_init(|| match try_load_level_refs() {
        Ok(data) => data,
        Err(error) => {
            log::error!("Failed to load level refs: {error}");
            LevelRefsData {
                refs: HashMap::new(),
                prefabs: HashMap::new(),
            }
        }
    })
}

fn try_load_level_refs() -> AppResult<LevelRefsData> {
    let loaders = load_embedded_loaders()?;
    let prefab_names = load_prefab_names_by_file_id()?;
    let refs = build_runtime_level_refs(&loaders);
    let prefabs = build_runtime_prefab_names(&loaders, &prefab_names);

    Ok(LevelRefsData { refs, prefabs })
}

fn load_embedded_loaders() -> AppResult<Vec<ParsedLoaderPrefab>> {
    let manifest = read_embedded_text(LOADER_MANIFEST_ASSET)?;
    let mut loaders = Vec::new();

    for relative_path in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let asset_path = format!("{LOADER_ASSET_PREFIX}{relative_path}");
        let text = read_embedded_text(&asset_path)?;
        let Some(loader) = parse_loader_prefab(&text) else {
            continue;
        };
        loaders.push(loader);
    }

    Ok(loaders)
}

fn build_runtime_level_refs(loaders: &[ParsedLoaderPrefab]) -> RefsMap {
    let mut refs = HashMap::new();

    for loader in loaders {
        let mut level_refs = HashMap::new();
        for (index, reference) in loader.references.iter().enumerate() {
            if reference.ref_type != 3 {
                continue;
            }

            let Some(guid) = reference.guid.as_deref() else {
                continue;
            };
            let Some(texture_name) = resolve_texture_name(&loader.level_key, guid) else {
                continue;
            };

            level_refs.insert(index as i32, texture_name.to_string());
        }

        refs.insert(loader.level_key.clone(), level_refs);
    }

    refs
}

fn load_prefab_names_by_file_id() -> AppResult<HashMap<String, String>> {
    let manifest = read_embedded_text(PREFAB_MANIFEST_ASSET)?;
    let mut names_by_file_id = HashMap::new();

    for relative_path in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let asset_path = format!("{PREFAB_ASSET_PREFIX}{relative_path}");
        let text = read_embedded_text(&asset_path)?;
        let prefab_name = Path::new(relative_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(relative_path)
            .to_string();

        for file_id in parse_prefab_game_object_ids(&text) {
            names_by_file_id
                .entry(file_id)
                .or_insert_with(|| prefab_name.clone());
        }
    }

    Ok(names_by_file_id)
}

fn parse_prefab_game_object_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("--- !u!1 &"))
        .map(|file_id| file_id.trim().to_string())
        .collect()
}

fn build_runtime_prefab_names(
    loaders: &[ParsedLoaderPrefab],
    prefab_names: &HashMap<String, String>,
) -> PrefabsMap {
    let mut prefabs = HashMap::new();

    for loader in loaders {
        let mut level_prefabs = HashMap::new();
        for (index, file_id) in loader.prefab_file_ids.iter().enumerate() {
            let Ok(index) = i16::try_from(index) else {
                continue;
            };
            let Some(name) = prefab_names.get(file_id) else {
                continue;
            };
            level_prefabs.insert(index, name.clone());
        }
        prefabs.insert(loader.level_key.clone(), level_prefabs);
    }

    prefabs
}

fn read_embedded_text(path: &str) -> AppResult<String> {
    let bytes = assets::read_asset(path).ok_or_else(|| {
        AppError::invalid_data_key1("error_level_refs_missing_asset", path.to_string())
    })?;
    String::from_utf8(bytes.into_owned()).map_err(|error| {
        AppError::invalid_data_key1("error_level_refs_invalid_utf8", format!("{path}: {error}"))
    })
}

fn parse_loader_prefab(text: &str) -> Option<ParsedLoaderPrefab> {
    let mut asset_name = None;
    let mut prefab_file_ids = Vec::new();
    let mut references = Vec::new();

    #[derive(Clone, Copy)]
    enum LoaderSection {
        None,
        Prefabs,
        References,
    }

    let mut section = LoaderSection::None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_prefix("assetName:") {
            asset_name = Some(name.trim().to_string());
        }

        if trimmed == "m_prefabs:" {
            section = LoaderSection::Prefabs;
            continue;
        }

        if trimmed == "m_references:" {
            section = LoaderSection::References;
            continue;
        }

        match section {
            LoaderSection::None => {}
            LoaderSection::Prefabs => {
                if trimmed.starts_with("- ") {
                    if let Some(file_id) = extract_loader_field(trimmed, "fileID: ") {
                        prefab_file_ids.push(file_id.to_string());
                    }
                    continue;
                }

                if trimmed.starts_with("m_") || trimmed.starts_with("assetBundle:") {
                    section = LoaderSection::None;
                }
            }
            LoaderSection::References => {
                if trimmed.starts_with("- ") {
                    references.push(parse_loader_reference(trimmed));
                    continue;
                }

                if trimmed.starts_with("m_") || trimmed.starts_with("assetBundle:") {
                    section = LoaderSection::None;
                }
            }
        }
    }

    asset_name.map(|level_key| ParsedLoaderPrefab {
        level_key,
        prefab_file_ids,
        references,
    })
}

fn parse_loader_reference(line: &str) -> LoaderReference {
    LoaderReference {
        guid: extract_loader_field(line, "guid: ").map(ToOwned::to_owned),
        ref_type: extract_loader_field(line, "type: ")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_default(),
    }
}

fn extract_loader_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let start = line.find(field)? + field.len();
    let rest = &line[start..];
    let end = rest
        .find(',')
        .or_else(|| rest.find('}'))
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn resolve_texture_name(level_key: &str, guid: &str) -> Option<&'static str> {
    override_texture_name_for_level(level_key, guid).or_else(|| default_texture_name_for_guid(guid))
}

fn default_texture_name_for_guid(guid: &str) -> Option<&'static str> {
    match guid {
        "0595a4242277e8bf4509f40a650321da" => Some("Ground_Ice_Outline.png"),
        "099f1ac79d8437a04cee2da4945ce8df" => Some("Ground_Grass_Maya_Texture.png"),
        "106bfafde9fb258f408b316319b09705" => Some("Ground_Rocks_Texture_02.png"),
        "112abcc05e24258c4169e63993930203" => Some("Ground_Grass_Texture_3.png"),
        "1261cd9f37a4aaa6420caadf1b1c83b5" => Some("Ground_Maya_cave_texture_bg.png"),
        "3ed677047877dc834d24835296ed719b" => Some("Ground_Rocks_Texture_05.png"),
        "413dd7c251004ea54c009643f3907841" => Some("Ground_Temple_cave_dark.png"),
        "45852d194a2aa49342a35ed790392320" => Some("Ground_Temple_Dark_Texture.png"),
        "65c5fa7e53072cae4d1032d660fe281e" => Some("Ground_Snow_Texture.png"),
        "6ddb5c22ed9534b248104e4ab9bfaa4e" => Some("Ground_Rocks_Outline_Texture_03.png"),
        "755ba0d622586ab5450f29e7c9da610f" => Some("Ground_Cave_Texture.png"),
        "7913446d9ac5efad454f59abc28c84ac" => Some("Ground_Temple_Tile_Texture.png"),
        "7f139bbf61097b904decc3330606eb10" => Some("Ground_Temple_cave.png"),
        "7f8a0efb020205bf4f9d4ede0e6ad445" => Some("Ground_Rocks_Outline_Texture.png"),
        "88e44c05da33a0b449016bd36686ac9c" => Some("Ground_Temple_Dark_Texture_02.png"),
        "8d268126cfbbe69b4159e555b69bcc4f" => Some("Ground_Maya_cave_texture_bg.png"),
        "8e6d64dad7ef5ea54fbb77d86cb9a9d7" => Some("Ground_Maya_cave_texture.png"),
        "977533b9ad59c2ae434820adb5b74076" => Some("Ground_Rocks_Outline_Texture_06.png"),
        "9d4aad58526c159f4299c3ad34644a92" => Some("Ground_Rocks_Outline_Texture_02.png"),
        "9e649cc7317c24aa4527db7d09a07ddf" => Some("Ground_Grass_Texture.png"),
        "a435b589ae7331a44ec02c7af798fd77" => Some("Ground_Rocks_Texture_06.png"),
        "a5c8346ecb8cf1bb4ee7e76263e8bbc3" => Some("Ground_Grass_Texture_02.png"),
        "a5da0d35e72e39af4feec543d8e35dc6" => Some("Ground_Rocks_Texture.png"),
        "a9d4884632d3bdb547183911b83c8e0f" => Some("Ground_Rocks_Texture_04.png"),
        "b1166fc3b0bb6eab4ec3d24c4e170eee" => Some("Ground_Halloween_Cream_Texture.png"),
        "b175d459066da29e40ad914f4a0f580a" => Some("Ground_Temple_cave_dark.png"),
        "b2515ce4a02cc8b444525f7d9fcc2a5c" => Some("Ground_Halloween_Texture_BG.png"),
        "c6da515a2512b3ab4281bd3747666ae3" => Some("Ground_Ice_Texture.png"),
        "c7054eac4cb471a04cd4ffad6fb86ec9" => Some("Ground_Rocks_Outline_Texture_04.png"),
        "d2f621392c0b8bb74760c6f819a08e48" => Some("Ground_Halloween_Outline_Texture.png"),
        "d3e2c0cc9fb321af40057511249fa805" => Some("Ground_Halloween_Texture.png"),
        "d87e8f561e24ac9245b3a51e0e611a08" => Some("Ground_Rocks_Outline_Texture_06.png"),
        "f02b58597a8fa8b549c6d1e3fec2651a" => Some("Ground_Temple_Rock_Texture.png"),
        "f2ac7f939745a094490b25021eef34be" => Some("Ground_Rocks_Outline_Texture_05.png"),
        _ => None,
    }
}

fn override_texture_name_for_level(level_key: &str, guid: &str) -> Option<&'static str> {
    match (level_key, guid) {
        ("Episode_6_Ice Sandbox_data", "7913446d9ac5efad454f59abc28c84ac") => {
            Some("Ground_Temple_Rock_Texture.png")
        }
        ("Episode_6_Ice Sandbox_data", "88e44c05da33a0b449016bd36686ac9c") => {
            Some("Ground_Temple_cave.png")
        }
        ("Episode_6_Tower Sandbox_data", "45852d194a2aa49342a35ed790392320") => {
            Some("Ground_Rocks_Texture_06.png")
        }
        ("Episode_6_Tower Sandbox_data", "7913446d9ac5efad454f59abc28c84ac") => {
            Some("Ground_Temple_Rock_Texture.png")
        }
        ("Episode_6_Tower Sandbox_data", "88e44c05da33a0b449016bd36686ac9c") => {
            Some("Ground_Temple_cave.png")
        }
        ("Episode_6_Tower Sandbox_data", "f02b58597a8fa8b549c6d1e3fec2651a") => {
            Some("Ground_Temple_Tile_Texture.png")
        }
        _ => None,
    }
}

/// Derive the level-refs key from a filename (strip `.bytes` extension).
pub fn level_key_from_filename(filename: &str) -> String {
    filename
        .strip_suffix(".bytes")
        .unwrap_or(filename)
        .to_string()
}

/// Look up a terrain texture filename by level key and reference index.
pub fn get_level_ref(level_key: &str, ref_index: i32) -> Option<&'static str> {
    data()
        .refs
        .get(level_key)
        .and_then(|m| m.get(&ref_index))
        .map(|s| s.as_str())
}

/// Get the resolved prefab name for a loader prefab index.
pub fn get_prefab_override(level_key: &str, prefab_index: i16) -> Option<&'static str> {
    data()
        .prefabs
        .get(level_key)
        .and_then(|m| m.get(&prefab_index))
        .map(|s| s.as_str())
}

#[cfg(test)]
mod tests {
    use super::{get_level_ref, get_prefab_override, level_key_from_filename};

    #[test]
    fn level_key_from_filename_strips_bytes_suffix() {
        assert_eq!(
            level_key_from_filename("Level_14_data.bytes"),
            "Level_14_data"
        );
    }

    #[test]
    fn runtime_loader_refs_match_basic_level() {
        assert_eq!(
            get_level_ref("Level_14_data", 9),
            Some("Ground_Rocks_Texture.png")
        );
        assert_eq!(
            get_level_ref("Level_14_data", 10),
            Some("Ground_Grass_Texture.png")
        );
        assert_eq!(
            get_level_ref("Level_14_data", 11),
            Some("Ground_Rocks_Outline_Texture.png")
        );
    }

    #[test]
    fn runtime_loader_refs_keep_episode_6_sandbox_overrides() {
        assert_eq!(
            get_level_ref("Episode_6_Ice Sandbox_data", 7),
            Some("Ground_Temple_cave.png")
        );
        assert_eq!(
            get_level_ref("Episode_6_Ice Sandbox_data", 161),
            Some("Ground_Temple_Rock_Texture.png")
        );
    }

    #[test]
    fn runtime_loader_prefabs_resolve_names() {
        assert_eq!(get_prefab_override("Level_51_data", 17), Some("Grass_09_0"));
        assert_eq!(get_prefab_override("Level_51_data", 19), Some("Grass_11_0"));
        assert_eq!(
            get_prefab_override("Level_Sandbox_06_data", 15),
            Some("Bush_03_0")
        );
    }
}
