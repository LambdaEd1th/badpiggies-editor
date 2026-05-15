//! Reverse lookup: `.contraption` SHA1-hex filenames -> level scene names.
//!
//! Base level metadata is parsed from embedded Unity episode prefabs at runtime.
//! Only save-specific key derivation stays in Rust:
//! - contraption filename stem = first 10 bytes of `SHA1(level_key)` as uppercase hex
//! - sandbox extra slots use `{scene}_{slot}` for `slot in 1..=40`
//! - race variants use `cr_{scene}_{track}` for `track in 0..=20`

use crate::data::assets;
use serde_yaml::{Mapping, Value};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const LEVELS_PREFAB_DIR: &str = "Prefab/";
const LEVELS_PREFAB_SUFFIX: &str = "Levels.prefab";
const SANDBOX_SLOT_COUNT: u32 = 40;
const RACE_TRACK_COUNT: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LevelCategory {
    Episode,
    Sandbox,
    Race,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LevelEntry {
    label: String,
    scene: String,
    category: LevelCategory,
}

/// Compute the `.contraption` filename stem (20 uppercase hex chars) for a level key.
fn contraption_hash(level_key: &str) -> String {
    let hash = Sha1::digest(level_key.as_bytes());
    hash[..10].iter().map(|b| format!("{b:02X}")).collect()
}

fn build_lookup() -> HashMap<String, (String, String)> {
    let entries = load_base_level_entries();
    let mut map = HashMap::with_capacity(entries.len() + 750);
    let mut sandbox_scenes = HashSet::new();
    let mut race_scenes = HashSet::new();

    for entry in &entries {
        let hash = contraption_hash(&entry.scene);
        map.insert(hash, (entry.label.clone(), entry.scene.clone()));

        match entry.category {
            LevelCategory::Sandbox => {
                sandbox_scenes.insert(entry.scene.clone());
            }
            LevelCategory::Race => {
                race_scenes.insert(entry.scene.clone());
            }
            LevelCategory::Episode => {}
        }
    }

    for scene in sandbox_scenes {
        for slot in 1..=SANDBOX_SLOT_COUNT {
            let key = format!("{scene}_{slot}");
            let hash = contraption_hash(&key);
            map.entry(hash).or_insert_with(|| (String::new(), key));
        }
    }

    for scene in race_scenes {
        for track in 0..=RACE_TRACK_COUNT {
            let key = format!("cr_{scene}_{track}");
            let hash = contraption_hash(&key);
            map.entry(hash).or_insert_with(|| (String::new(), key));
        }
    }

    map
}

fn load_base_level_entries() -> Vec<LevelEntry> {
    let mut entries = Vec::new();

    for filename in assets::list_asset_paths(LEVELS_PREFAB_DIR, LEVELS_PREFAB_SUFFIX) {
        if !filename.starts_with("Episode") {
            continue;
        }

        let asset_key = format!("{LEVELS_PREFAB_DIR}{filename}");
        let Some(text) = assets::read_asset_text(&asset_key) else {
            log::warn!("Missing embedded level prefab for contraption lookup: {}", asset_key);
            continue;
        };

        let parsed = parse_levels_prefab(&text);
        if parsed.is_empty() {
            log::warn!(
                "Failed to parse base level metadata from embedded prefab: {}",
                asset_key
            );
            continue;
        }

        entries.extend(parsed);
    }

    entries
}

fn parse_levels_prefab(text: &str) -> Vec<LevelEntry> {
    for doc in text.split("--- ").skip(1) {
        let mut lines = doc.lines();
        let Some(header) = lines.next().map(str::trim) else {
            continue;
        };
        let Some((type_id, _file_id)) = parse_doc_header(header) else {
            continue;
        };
        if type_id != 114 {
            continue;
        }

        let body = lines.collect::<Vec<_>>().join("\n");
        let Ok(value) = serde_yaml::from_str::<Value>(&body) else {
            continue;
        };
        let Some(("MonoBehaviour", fields)) = doc_root_mapping(&value) else {
            continue;
        };

        let entries = parse_level_entries(fields);
        if !entries.is_empty() {
            return entries;
        }
    }

    Vec::new()
}

fn parse_level_entries(fields: &Mapping) -> Vec<LevelEntry> {
    if let Some(level_infos) = map_get(fields, "m_levelInfos").and_then(Value::as_sequence) {
        let Some(prefix) = map_get(fields, "m_label").and_then(value_as_string) else {
            return Vec::new();
        };

        return level_infos
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let level = item.as_mapping()?;
                let scene = map_get(level, "sceneName")?.as_str()?.to_string();
                Some(LevelEntry {
                    label: format!("{prefix}-{}", index + 1),
                    scene,
                    category: LevelCategory::Episode,
                })
            })
            .collect();
    }

    let Some(levels) = map_get(fields, "m_levels").and_then(Value::as_sequence) else {
        return Vec::new();
    };

    let category = match map_get(fields, "m_label").and_then(value_as_string).as_deref() {
        Some("S") => LevelCategory::Sandbox,
        Some("R") => LevelCategory::Race,
        _ => return Vec::new(),
    };

    levels
        .iter()
        .filter_map(|item| parse_loader_backed_level(item, category))
        .collect()
}

fn parse_loader_backed_level(value: &Value, category: LevelCategory) -> Option<LevelEntry> {
    let level = value.as_mapping()?;
    let label = map_get(level, "m_identifier")?.as_str()?.to_string();
    let loader_path = map_get(level, "m_levelLoaderPath")?.as_str()?;
    let scene = scene_name_from_loader_path(loader_path)?;

    Some(LevelEntry {
        label,
        scene,
        category,
    })
}

fn scene_name_from_loader_path(path: &str) -> Option<String> {
    let filename = path.rsplit('/').next()?;
    filename
        .strip_suffix("_loader.prefab")
        .map(str::to_string)
}

fn parse_doc_header(header: &str) -> Option<(u32, String)> {
    let mut parts = header.split_whitespace();
    let type_id = parts.next()?.strip_prefix("!u!")?.parse().ok()?;
    let file_id = parts.next()?.strip_prefix('&')?.to_string();
    Some((type_id, file_id))
}

fn doc_root_mapping(value: &Value) -> Option<(&str, &Mapping)> {
    let mapping = value.as_mapping()?;
    let (key, fields) = mapping.iter().next()?;
    Some((key.as_str()?, fields.as_mapping()?))
}

fn map_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.iter()
        .find_map(|(candidate, value)| (candidate.as_str() == Some(key)).then_some(value))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|value| value as i64))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

fn value_as_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value_as_i64(value).map(|value| value.to_string()))
}

/// Look up the level name for a `.contraption` filename.
///
/// `filename_stem` is the filename without the `.contraption` extension (20 hex chars).
/// Returns `(display_label, scene_key)`. The label may be empty for sandbox-slot / cake-race
/// variants that don't map to a numbered level.
pub fn contraption_level_name(filename_stem: &str) -> Option<(&'static str, &'static str)> {
    static LOOKUP: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();
    let map = LOOKUP.get_or_init(build_lookup);
    let upper = filename_stem.to_ascii_uppercase();
    map.get(&upper)
        .map(|(label, scene)| (label.as_str(), scene.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup_for_level_key(level_key: &str) -> Option<(String, String)> {
        let stem = contraption_hash(level_key);
        contraption_level_name(&stem).map(|(label, scene)| (label.to_string(), scene.to_string()))
    }

    #[test]
    fn embedded_level_prefabs_cover_expected_base_counts() {
        let entries = load_base_level_entries();
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.category == LevelCategory::Episode)
                .count(),
            255
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.category == LevelCategory::Sandbox)
                .count(),
            14
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.category == LevelCategory::Race)
                .count(),
            8
        );
    }

    #[test]
    fn lookup_resolves_known_episode_sandbox_and_race_levels() {
        assert_eq!(
            lookup_for_level_key("Level_21"),
            Some(("1-1".to_string(), "Level_21".to_string()))
        );
        assert_eq!(
            lookup_for_level_key("scenario_69"),
            Some(("2-1".to_string(), "scenario_69".to_string()))
        );
        assert_eq!(
            lookup_for_level_key("episode_6_level_VI"),
            Some(("6-30".to_string(), "episode_6_level_VI".to_string()))
        );
        assert_eq!(
            lookup_for_level_key("Level_Sandbox_01"),
            Some(("S-S".to_string(), "Level_Sandbox_01".to_string()))
        );
        assert_eq!(
            lookup_for_level_key("Level_Race_01"),
            Some(("R-4".to_string(), "Level_Race_01".to_string()))
        );
    }

    #[test]
    fn lookup_generates_sandbox_and_race_variants() {
        assert_eq!(
            lookup_for_level_key("Level_Sandbox_01_3"),
            Some(("".to_string(), "Level_Sandbox_01_3".to_string()))
        );
        assert_eq!(
            lookup_for_level_key("cr_Level_Race_01_0"),
            Some(("".to_string(), "cr_Level_Race_01_0".to_string()))
        );
    }

    #[test]
    fn lookup_size_matches_expected_dynamic_and_derived_entries() {
        assert_eq!(build_lookup().len(), 1005);
    }

    #[test]
    fn scene_name_derives_from_loader_path() {
        assert_eq!(
            scene_name_from_loader_path(
                "Assets/Resources/Levels/Episode_6_Sandbox/Episode_6_Tower Sandbox_loader.prefab"
            ),
            Some("Episode_6_Tower Sandbox".to_string())
        );
    }
}
