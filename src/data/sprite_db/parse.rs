//! Unity prefab YAML parser → ParsedPrefab.

use std::collections::HashMap;

use super::WORLD_SCALE;
use super::types::{
    GameObjectInfo, ParsedPrefab, RendererInfo, RuntimeSpriteComponent, TransformInfo,
    UnmanagedSpriteComponent, UvRect,
};

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
            23 => parse_renderer(doc, &mut parsed.renderers),
            114 => parse_mono_behaviour(doc, &mut parsed),
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
    let mut active = true;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_IsActive:") {
            active = value.trim() != "0";
        }
    }

    game_objects.insert(file_id.to_string(), GameObjectInfo { active });
}

fn parse_transform(doc: &str, file_id: &str, transforms: &mut HashMap<String, TransformInfo>) {
    let mut game_object_id = None;
    let mut father = String::from("0");
    let mut children = Vec::new();
    let mut in_children = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
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
            father,
            children,
        },
    );
}

fn parse_renderer(doc: &str, renderers: &mut HashMap<String, RendererInfo>) {
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
        game_object_id,
        RendererInfo {
            material_guid,
            enabled,
        },
    );
}

fn parse_mono_behaviour(doc: &str, parsed: &mut ParsedPrefab) {
    let mut game_object_id = None;

    let mut sprite_id = None;
    let mut scale_x = None;
    let mut scale_y = None;

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
        if let Some(value) = trimmed.strip_prefix("m_id:") {
            sprite_id = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleX:") {
            scale_x = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleY:") {
            scale_y = value.trim().parse().ok();
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
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };

    if let (Some(sprite_id), Some(scale_x), Some(scale_y)) = (sprite_id, scale_x, scale_y) {
        parsed.runtime_sprites.push(RuntimeSpriteComponent {
            game_object_id,
            sprite_id,
            scale_x,
            scale_y,
        });
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
