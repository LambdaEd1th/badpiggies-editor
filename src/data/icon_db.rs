//! Icon layer database — loads multi-layer icon compositing data from icon-layers.toml.
//!
//! Each part type maps to one or more sprite layers from the atlas, with
//! position offsets and scale factors, matching the Unity Icon_* prefabs.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::diagnostics::error::{AppError, AppResult};

/// A single compositing layer within a part icon.
#[derive(Debug, Clone)]
pub struct IconLayer {
    pub go_name: String,
    pub atlas: String,
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    /// Local z-offset within the part hierarchy (accumulated from parent transforms).
    /// Used for global depth sorting across all parts.
    pub z_local: f32,
    /// Baked local quad vertices in part-local world units.
    /// Vertex order matches Unity mesh creation: v0=BL, v1=TL, v2=TR, v3=BR.
    pub v0_x: f32,
    pub v0_y: f32,
    pub v1_x: f32,
    pub v1_y: f32,
    pub v2_x: f32,
    pub v2_y: f32,
    pub v3_x: f32,
    pub v3_y: f32,
}

// ── TOML deserialization ──

#[derive(Deserialize)]
struct IconLayersToml {
    parts: HashMap<String, PartEntry>,
}

#[derive(Deserialize)]
struct PartEntry {
    #[serde(rename = "name")]
    _name: String,
    #[serde(default)]
    z_offset: f32,
    layers: Vec<LayerEntry>,
}

#[derive(Deserialize)]
struct LayerEntry {
    #[serde(default)]
    go_name: String,
    atlas: String,
    uv_x: f32,
    uv_y: f32,
    uv_w: f32,
    uv_h: f32,
    #[serde(default)]
    z_local: f32,
    v0_x: f32,
    v0_y: f32,
    v1_x: f32,
    v1_y: f32,
    v2_x: f32,
    v2_y: f32,
    v3_x: f32,
    v3_y: f32,
}

// ── Global singleton ──

/// Per-part info: z_offset + layers.
pub struct PartInfo {
    pub z_offset: f32,
    pub layers: Vec<IconLayer>,
}

static ICON_DB: OnceLock<HashMap<String, PartInfo>> = OnceLock::new();

fn load() -> HashMap<String, PartInfo> {
    let parsed = match try_load_toml() {
        Ok(parsed) => parsed,
        Err(error) => {
            log::error!("Failed to load icon layer database: {error}");
            return HashMap::new();
        }
    };

    let mut map = HashMap::new();
    for (key, entry) in parsed.parts {
        let layers: Vec<IconLayer> = entry
            .layers
            .into_iter()
            .map(|l| IconLayer {
                go_name: l.go_name,
                atlas: l.atlas,
                uv_x: l.uv_x,
                uv_y: l.uv_y,
                uv_w: l.uv_w,
                uv_h: l.uv_h,
                z_local: l.z_local,
                v0_x: l.v0_x,
                v0_y: l.v0_y,
                v1_x: l.v1_x,
                v1_y: l.v1_y,
                v2_x: l.v2_x,
                v2_y: l.v2_y,
                v3_x: l.v3_x,
                v3_y: l.v3_y,
            })
            .collect();
        map.insert(key, PartInfo { z_offset: entry.z_offset, layers });
    }
    map
}

fn try_load_toml() -> AppResult<IconLayersToml> {
    let Some(toml_bytes) = crate::data::assets::EmbeddedAssets::get("icon-layers.toml") else {
        return Err(AppError::invalid_data_key("error_icon_layers_missing"));
    };
    let toml_str = std::str::from_utf8(&toml_bytes.data).map_err(|error| {
        AppError::invalid_data_key1("error_icon_layers_not_utf8", error.to_string())
    })?;
    toml::from_str(toml_str)
        .map_err(|error| AppError::invalid_data_key1("error_icon_layers_parse", error.to_string()))
}

/// Get the part info (z_offset + layers) for a given part type and custom part index.
/// Falls back to customPartIndex=0 if the exact variant is not found.
pub fn get_part_info(part_type: i32, custom_part_index: i32) -> Option<&'static PartInfo> {
    let db = ICON_DB.get_or_init(load);
    // Try exact match first
    let key = format!("{part_type}.{custom_part_index}");
    if let Some(info) = db.get(&key) {
        return Some(info);
    }
    // Fall back to default variant (customPartIndex=0)
    let key = format!("{part_type}.0");
    db.get(&key)
}
