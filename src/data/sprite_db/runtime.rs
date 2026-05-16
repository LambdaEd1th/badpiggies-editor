//! Sprites.bytes + spritemapping.bytes loader for sprite_db.

use std::collections::HashMap;

use super::read_embedded_text;
use super::types::{RuntimeSpriteMeta, UvRect};

const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";

pub(super) fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
    let mut sprite_data = HashMap::new();

    let Some(text) = read_embedded_text(SPRITES_BYTES_ASSET) else {
        log::error!(
            "Failed to read embedded {} for sprite database",
            SPRITES_BYTES_ASSET
        );
        return HashMap::new();
    };

    for line in text.lines() {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.len() < 14 {
            continue;
        }
        let Some(width) = fields[11].parse().ok() else {
            continue;
        };
        let Some(height) = fields[12].parse().ok() else {
            continue;
        };
        sprite_data.insert(
            fields[0].to_string(),
            RuntimeSpriteMeta {
                material_id: fields[2].to_string(),
                width,
                height,
                uv: UvRect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
            },
        );
    }

    let Some(text) = read_embedded_text(SPRITE_MAPPING_ASSET) else {
        log::error!(
            "Failed to read embedded {} for sprite database",
            SPRITE_MAPPING_ASSET
        );
        return HashMap::new();
    };

    for line in text.lines() {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.len() < 5 {
            continue;
        }
        let Some(entry) = sprite_data.get_mut(fields[0]) else {
            continue;
        };
        let Some(x) = fields[1].parse().ok() else {
            continue;
        };
        let Some(y) = fields[2].parse().ok() else {
            continue;
        };
        let Some(w) = fields[3].parse().ok() else {
            continue;
        };
        let Some(h) = fields[4].parse().ok() else {
            continue;
        };
        entry.uv = UvRect { x, y, w, h };
    }

    sprite_data
        .into_iter()
        .filter(|(_, entry)| entry.uv.w > 0.0 && entry.uv.h > 0.0)
        .collect()
}
