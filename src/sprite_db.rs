//! Sprite database — loads sprite atlas UV/sizing data from embedded TOML.
//!
//! At compile time, includes `sprite-data.toml` (generated from extract_sprites.py).
//! At runtime, deserializes into lookup tables for resolving prefab names to
//! atlas + UV rect + world size.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// Resolved sprite info ready for rendering.
#[derive(Debug, Clone)]
pub struct SpriteInfo {
    /// Atlas filename (e.g. "IngameAtlas.png").
    pub atlas: String,
    /// Normalized UV rect [0..1].
    pub uv: UvRect,
    /// Half-extent in world units.
    pub world_w: f32,
    pub world_h: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// ── JSON deserialization structures ──

#[derive(Deserialize)]
struct SpriteDataJson {
    runtime: HashMap<String, RuntimeEntry>,
    unmanaged: HashMap<String, UnmanagedEntry>,
}

#[derive(Deserialize)]
struct RuntimeEntry {
    uv: UvJson,
    width: f32,
    height: f32,
    #[serde(rename = "scaleX")]
    scale_x: f32,
    #[serde(rename = "scaleY")]
    scale_y: f32,
    atlas: String,
}

#[derive(Deserialize)]
struct UvJson {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Deserialize)]
struct UnmanagedEntry {
    #[serde(rename = "uvX")]
    uv_x: f32,
    #[serde(rename = "uvY")]
    uv_y: f32,
    #[serde(rename = "gridW")]
    grid_w: f32,
    #[serde(rename = "gridH")]
    grid_h: f32,
    #[serde(rename = "spriteW")]
    sprite_w: f32,
    #[serde(rename = "spriteH")]
    sprite_h: f32,
    subdiv: f32,
}

/// World-size formula: pixelSize * prefabScale * 10 / 768
const WORLD_SCALE: f32 = 10.0 / 768.0;

static SPRITE_DB: OnceLock<HashMap<String, SpriteInfo>> = OnceLock::new();

fn build_db() -> HashMap<String, SpriteInfo> {
    let toml_str = include_str!("../assets/sprite-data.toml");
    let data: SpriteDataJson = toml::from_str(toml_str).expect("sprite-data.toml parse error");

    let mut map = HashMap::with_capacity(data.runtime.len() + data.unmanaged.len());

    for (name, e) in &data.runtime {
        let sprite_w = e.width * e.scale_x;
        let sprite_h = e.height * e.scale_y;
        map.insert(
            name.clone(),
            SpriteInfo {
                atlas: e.atlas.clone(),
                uv: UvRect {
                    x: e.uv.x,
                    y: e.uv.y,
                    w: e.uv.w,
                    h: e.uv.h,
                },
                world_w: sprite_w * WORLD_SCALE,
                world_h: sprite_h * WORLD_SCALE,
            },
        );
    }

    for (name, e) in &data.unmanaged {
        let uv = UvRect {
            x: e.uv_x / e.subdiv,
            y: e.uv_y / e.subdiv,
            w: e.grid_w / e.subdiv,
            h: e.grid_h / e.subdiv,
        };
        // Only insert if no runtime sprite exists — runtime entries take priority
        map.entry(name.clone()).or_insert(SpriteInfo {
            atlas: "Props_Generic_Sheet_01.png".into(),
            uv,
            world_w: e.sprite_w * WORLD_SCALE,
            world_h: e.sprite_h * WORLD_SCALE,
        });
    }

    map
}

/// Get the sprite database (lazily initialized).
pub fn sprite_db() -> &'static HashMap<String, SpriteInfo> {
    SPRITE_DB.get_or_init(build_db)
}

/// Look up sprite info by name, with normalization fallbacks.
pub fn get_sprite_info(name: &str) -> Option<&'static SpriteInfo> {
    let db = sprite_db();

    // Direct lookup
    if let Some(s) = db.get(name) {
        return Some(s);
    }

    // Strip " (N)" duplicates: "Bottle (2)" → "Bottle"
    if let Some(base) = name.split(" (").next()
        && base != name
        && let Some(s) = db.get(base)
    {
        return Some(s);
    }

    // Strip trailing digits: "StarBox01" → "StarBox"
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != name
        && !trimmed.is_empty()
        && let Some(s) = db.get(trimmed)
    {
        return Some(s);
    }

    // Strip "_001" style suffixes
    if let Some(pos) = name.rfind('_') {
        let suffix = &name[pos + 1..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            let base = &name[..pos];
            if let Some(s) = db.get(base) {
                return Some(s);
            }
        }
    }

    // Common runtime/prefab alias: "Bird_Black" -> "Bird_Black_01"
    let suffixed = format!("{name}_01");
    if let Some(s) = db.get(&suffixed) {
        return Some(s);
    }

    None
}
