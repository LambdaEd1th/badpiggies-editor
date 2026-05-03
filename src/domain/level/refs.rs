//! Level-refs database — per-level terrain texture mappings and prefab name overrides.
//!
//! Embeds `data/level-refs.toml` (generated from level-refs.gen.ts).
//! Provides two lookups:
//! - `get_level_refs(level_key, ref_index)` → terrain texture filename
//! - `resolve_prefab_name(level_key, prefab_index, fallback)` → corrected prefab name

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::diagnostics::error::{AppError, AppResult};

#[derive(Deserialize)]
struct LevelRefsToml {
    refs: HashMap<String, HashMap<String, String>>,
    prefabs: HashMap<String, HashMap<String, String>>,
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
    let toml_str = include_str!("../../../assets/level-refs.toml");
    let raw: LevelRefsToml = toml::from_str(toml_str)
        .map_err(|error| AppError::invalid_data_key1("error_level_refs_parse", error.to_string()))?;

    let refs: RefsMap = raw
        .refs
        .into_iter()
        .map(|(k, v)| {
            let inner: HashMap<i32, String> = v
                .into_iter()
                .filter_map(|(idx_str, val)| idx_str.parse::<i32>().ok().map(|i| (i, val)))
                .collect();
            (k, inner)
        })
        .collect();

    let prefabs: PrefabsMap = raw
        .prefabs
        .into_iter()
        .map(|(k, v)| {
            let inner: HashMap<i16, String> = v
                .into_iter()
                .filter_map(|(idx_str, val)| idx_str.parse::<i16>().ok().map(|i| (i, val)))
                .collect();
            (k, inner)
        })
        .collect();

    Ok(LevelRefsData { refs, prefabs })
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

/// Get the resolved prefab name, returning a static string if override exists.
pub fn get_prefab_override(level_key: &str, prefab_index: i16) -> Option<&'static str> {
    data()
        .prefabs
        .get(level_key)
        .and_then(|m| m.get(&prefab_index))
        .map(|s| s.as_str())
}
