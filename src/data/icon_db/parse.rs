//! Unity prefab text parser → ParsedPrefab + material → atlas lookup.

use std::collections::HashMap;

use super::types::{
    GameObjectInfo, ParsedPrefab, RendererInfo, SpriteComponent, TransformInfo,
};

const SPRITE_SCRIPT_GUID: &str = "eaa85264a31f76994888187c4d3a9fb9";

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

    for line in doc.lines() {
        let trimmed = line.trim();
        if name.is_none()
            && let Some(value) = trimmed.strip_prefix("m_Name:")
        {
            name = Some(value.trim().to_string());
        }
    }

    if let Some(name) = name {
        game_objects.insert(file_id.to_string(), GameObjectInfo { name });
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
        if let Some(value) = trimmed.strip_prefix("m_partType:") {
            parsed.part_type = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("customPartIndex:") {
            parsed.custom_part_index = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_ZOffset:") {
            parsed.z_offset = value.trim().parse().unwrap_or(0.0);
        }
    }

    if script_guid.as_deref() != Some(SPRITE_SCRIPT_GUID) {
        return;
    }
    let (Some(game_object_id), Some(sprite_id)) = (game_object_id, sprite_id) else {
        return;
    };

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

pub(super) fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    let prefix = material_guid.get(..8).unwrap_or(material_guid);
    match prefix {
        "ce5a9931" | "d645821c" | "125eb5b4" | "0e790fab" | "353dd850" => Some("IngameAtlas.png"),
        "211b2b9c" | "aca6a4c6" | "765e60c2" | "4ab535f3" | "4eeb62bc" => Some("IngameAtlas2.png"),
        "2a21c011" | "ad767d84" | "7192b13e" | "a6f51d97" | "7975d66d" => Some("IngameAtlas3.png"),
        _ => None,
    }
}
