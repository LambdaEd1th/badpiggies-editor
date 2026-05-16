//! Sprites.bytes + spritemapping.bytes loader producing runtime sprite metadata.

use std::collections::HashMap;

use super::read_embedded_text;
use super::types::RuntimeSpriteMeta;

const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";

pub(super) fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
    let mut sprite_data = HashMap::new();

    let Some(text) = read_embedded_text(SPRITES_BYTES_ASSET) else {
        log::error!(
            "Failed to read embedded {} for icon layer database",
            SPRITES_BYTES_ASSET
        );
        return HashMap::new();
    };

    for line in text.lines() {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.len() < 14 {
            continue;
        }
        let Some(selection_x) = fields[3].parse().ok() else {
            continue;
        };
        let Some(selection_y) = fields[4].parse().ok() else {
            continue;
        };
        let Some(selection_w) = fields[5].parse().ok() else {
            continue;
        };
        let Some(selection_h) = fields[6].parse().ok() else {
            continue;
        };
        let Some(pivot_x) = fields[7].parse().ok() else {
            continue;
        };
        let Some(pivot_y) = fields[8].parse().ok() else {
            continue;
        };
        let Some(uv_x) = fields[9].parse().ok() else {
            continue;
        };
        let Some(uv_y) = fields[10].parse().ok() else {
            continue;
        };
        let Some(width) = fields[11].parse().ok() else {
            continue;
        };
        let Some(height) = fields[12].parse().ok() else {
            continue;
        };

        sprite_data.insert(
            fields[0].to_string(),
            RuntimeSpriteMeta {
                selection_x,
                selection_y,
                selection_w,
                selection_h,
                pivot_x,
                pivot_y,
                uv_x,
                uv_y,
                width,
                height,
                uv_x_norm: 0.0,
                uv_y_norm: 0.0,
                uv_w_norm: 0.0,
                uv_h_norm: 0.0,
            },
        );
    }

    let Some(text) = read_embedded_text(SPRITE_MAPPING_ASSET) else {
        log::error!(
            "Failed to read embedded {} for icon layer database",
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
        let Some(uv_x_norm) = fields[1].parse().ok() else {
            continue;
        };
        let Some(uv_y_norm) = fields[2].parse().ok() else {
            continue;
        };
        let Some(uv_w_norm) = fields[3].parse().ok() else {
            continue;
        };
        let Some(uv_h_norm) = fields[4].parse().ok() else {
            continue;
        };
        entry.uv_x_norm = uv_x_norm;
        entry.uv_y_norm = uv_y_norm;
        entry.uv_w_norm = uv_w_norm;
        entry.uv_h_norm = uv_h_norm;
    }

    sprite_data
        .into_iter()
        .filter(|(_, entry)| entry.uv_w_norm > 0.0 && entry.uv_h_norm > 0.0)
        .collect()
}
