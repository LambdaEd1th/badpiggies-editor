//! Low-level Unity prefab YAML parser and runtime sprite metadata loader.

use std::collections::HashMap;

use crate::data::assets;
use crate::data::sprite_db::UvRect;

use super::types::{
    GameObjectInfo, ParsedPrefab, RendererInfo, RuntimeSpriteMeta, SpriteComponent,
    TransformInfo, UnmanagedSpriteComponent,
};

const SPRITE_SCRIPT_GUID: &str = "eaa85264a31f76994888187c4d3a9fb9";
const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";
pub(super) const WORLD_SCALE: f32 = 10.0 / 768.0;

pub(super) fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

pub(super) fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    crate::data::assets::atlas_for_material_guid(material_guid)
}

pub(super) fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
    let mut sprite_data: HashMap<String, RuntimeSpriteMeta> = HashMap::new();
    let Some(text) = read_embedded_text(SPRITES_BYTES_ASSET) else {
        log::error!(
            "Failed to read embedded {} for prefab multi-sprite support",
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
            "Failed to read embedded {} for prefab multi-sprite support",
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

pub(super) fn parse_prefab(text: &str) -> ParsedPrefab {
    let mut parsed = ParsedPrefab::default();

    for doc in text.split("--- ").skip(1) {
        let Some(header) = doc.lines().next().map(str::trim) else {
            continue;
        };
        let Some((type_id, file_id)) = parse_doc_header(header) else {
            continue;
        };
        match type_id {
            1 => parse_game_object(doc, &file_id, &mut parsed.game_objects),
            4 => parse_transform(doc, &file_id, &mut parsed.transforms),
            23 => parse_renderer(doc, &file_id, &mut parsed.renderers),
            114 => parse_mono_behaviour(doc, &file_id, &mut parsed),
            _ => {}
        }
    }

    parsed
}

fn parse_doc_header(header: &str) -> Option<(u32, String)> {
    let mut parts = header.split_whitespace();
    let type_part = parts.next()?.strip_prefix("!u!")?;
    let file_part = parts.next()?.strip_prefix('&')?;
    Some((type_part.parse().ok()?, file_part.to_string()))
}

fn parse_game_object(doc: &str, file_id: &str, game_objects: &mut HashMap<String, GameObjectInfo>) {
    let mut name = None;
    let mut active = true;
    for line in doc.lines() {
        let trimmed = line.trim();
        if name.is_none()
            && let Some(value) = trimmed.strip_prefix("m_Name:")
        {
            name = Some(value.trim().to_string());
        }
        if let Some(value) = trimmed.strip_prefix("m_IsActive:") {
            active = value.trim() != "0";
        }
    }
    if let Some(name) = name {
        game_objects.insert(file_id.to_string(), GameObjectInfo { name, active });
    }
}

fn parse_transform(doc: &str, file_id: &str, transforms: &mut HashMap<String, TransformInfo>) {
    let mut game_object_id = None;
    let mut pos_x = 0.0;
    let mut pos_y = 0.0;
    let mut pos_z = 0.0;
    let mut scale_x = 1.0;
    let mut scale_y = 1.0;
    let mut qx = 0.0;
    let mut qy = 0.0;
    let mut qz = 0.0;
    let mut qw = 1.0;
    let mut father = String::from("0");
    let mut children = Vec::new();
    let mut in_children = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_LocalPosition:") {
            pos_x = parse_named_f32(value, "x:").unwrap_or(0.0);
            pos_y = parse_named_f32(value, "y:").unwrap_or(0.0);
            pos_z = parse_named_f32(value, "z:").unwrap_or(0.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_LocalScale:") {
            scale_x = parse_named_f32(value, "x:").unwrap_or(1.0);
            scale_y = parse_named_f32(value, "y:").unwrap_or(1.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_LocalRotation:") {
            qx = parse_named_f32(value, "x:").unwrap_or(0.0);
            qy = parse_named_f32(value, "y:").unwrap_or(0.0);
            qz = parse_named_f32(value, "z:").unwrap_or(0.0);
            qw = parse_named_f32(value, "w:").unwrap_or(1.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_Father:") {
            father = parse_file_id(value).unwrap_or_else(|| String::from("0"));
            in_children = false;
            continue;
        }
        if trimmed.starts_with("m_Children:") {
            in_children = !trimmed.contains("[]");
            continue;
        }
        if in_children {
            if trimmed.starts_with('-') {
                if let Some(child_id) = parse_file_id(trimmed) {
                    children.push(child_id);
                }
                continue;
            }
            if trimmed.starts_with("m_") {
                in_children = false;
            }
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };

    transforms.insert(
        file_id.to_string(),
        TransformInfo {
            game_object_id,
            pos_x,
            pos_y,
            pos_z,
            scale_x,
            scale_y,
            qx,
            qy,
            qz,
            qw,
            father,
            children,
        },
    );
}

fn parse_renderer(doc: &str, file_id: &str, renderers: &mut HashMap<String, RendererInfo>) {
    let mut game_object_id = None;
    let mut enabled = true;
    let mut material_guid = String::new();
    let mut in_materials = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_Enabled:") {
            enabled = value.trim() != "0";
            continue;
        }
        if trimmed.starts_with("m_Materials:") {
            in_materials = true;
            continue;
        }
        if in_materials {
            if let Some(guid) = parse_guid(trimmed) {
                material_guid = guid;
                in_materials = false;
                continue;
            }
            if trimmed.starts_with("m_") {
                in_materials = false;
            }
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };
    renderers.insert(
        file_id.to_string(),
        RendererInfo {
            game_object_id,
            material_guid,
            enabled,
        },
    );
}

fn parse_mono_behaviour(doc: &str, file_id: &str, parsed: &mut ParsedPrefab) {
    let mut game_object_id = None;
    let mut script_guid = None;
    let mut sprite_id = None;
    let mut scale_x = 1.0;
    let mut scale_y = 1.0;
    let mut pivot_x = 0.0;
    let mut pivot_y = 0.0;
    let mut uv_x = None;
    let mut uv_y = None;
    let mut grid_w = None;
    let mut grid_h = None;
    let mut sprite_w = None;
    let mut sprite_h = None;
    let mut subdiv_x = None;
    let mut subdiv_y = None;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_Script:") {
            script_guid = parse_guid(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_id:") {
            sprite_id = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleX:") {
            scale_x = value.trim().parse().unwrap_or(1.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleY:") {
            scale_y = value.trim().parse().unwrap_or(1.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_pivotX:") {
            pivot_x = value.trim().parse().unwrap_or(0.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_pivotY:") {
            pivot_y = value.trim().parse().unwrap_or(0.0);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_UVx:") {
            uv_x = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_UVy:") {
            uv_y = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_width:") {
            grid_w = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_height:") {
            grid_h = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_spriteWidth:") {
            sprite_w = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_spriteHeight:") {
            sprite_h = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_atlasGridSubdivisions:") {
            let parsed_value = value.trim().parse::<f32>().ok();
            subdiv_x = parsed_value;
            subdiv_y = parsed_value;
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_subdivisionsX:") {
            subdiv_x = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_subdivisionsY:") {
            subdiv_y = value.trim().parse::<f32>().ok();
            continue;
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };

    if script_guid.as_deref() == Some(SPRITE_SCRIPT_GUID)
        && let Some(sprite_id) = sprite_id
    {
        parsed.sprites.insert(
            file_id.to_string(),
            SpriteComponent {
                game_object_id,
                sprite_id,
                scale_x,
                scale_y,
                pivot_x,
                pivot_y,
            },
        );
        return;
    }

    let (Some(uv_x), Some(uv_y), Some(grid_w), Some(grid_h), Some(sprite_w), Some(sprite_h)) =
        (uv_x, uv_y, grid_w, grid_h, sprite_w, sprite_h)
    else {
        return;
    };
    let subdiv_x = subdiv_x.unwrap_or(0.0);
    let subdiv_y = subdiv_y.unwrap_or(subdiv_x);
    if subdiv_x <= 0.0 || subdiv_y <= 0.0 {
        return;
    }

    parsed.unmanaged_sprites.insert(
        game_object_id,
        UnmanagedSpriteComponent {
            uv: UvRect {
                x: uv_x / subdiv_x,
                y: uv_y / subdiv_y,
                w: grid_w / subdiv_x,
                h: grid_h / subdiv_y,
            },
            world_w: sprite_w * WORLD_SCALE,
            world_h: sprite_h * WORLD_SCALE,
        },
    );
}

fn parse_file_id(text: &str) -> Option<String> {
    let rest = text[text.find("fileID:")? + "fileID:".len()..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    let file_id = &rest[..end];
    (!file_id.is_empty()).then(|| file_id.to_string())
}

fn parse_guid(text: &str) -> Option<String> {
    let rest = text[text.find("guid:")? + "guid:".len()..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    let guid = &rest[..end];
    (!guid.is_empty()).then(|| guid.to_string())
}

fn parse_named_f32(text: &str, key: &str) -> Option<f32> {
    let rest = text[text.find(key)? + key.len()..].trim_start();
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+' | 'e' | 'E')))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}
