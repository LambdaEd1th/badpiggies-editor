use std::collections::HashMap;

use crate::data::assets;

use super::tables::BG_SPRITE_SCRIPT_GUID;

#[derive(Debug, Clone)]
pub(super) struct GameObjectInfo {
    pub(super) name: String,
    pub(super) tag: String,
    pub(super) active: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TransformInfo {
    pub(super) game_object_id: String,
    pub(super) local_pos: [f32; 3],
    pub(super) local_scale: [f32; 3],
    pub(super) parent_id: Option<String>,
    pub(super) children: Vec<String>,
    pub(super) root_order: i32,
}

#[derive(Debug, Clone)]
pub(super) struct SpriteComponent {
    pub(super) sprite_width: f32,
    pub(super) sprite_height: f32,
    pub(super) uv_x: f32,
    pub(super) uv_y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) subdiv: f32,
    pub(super) border: f32,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedPrefab {
    pub(super) root_transform_id: String,
    pub(super) game_objects: HashMap<String, GameObjectInfo>,
    pub(super) transforms: HashMap<String, TransformInfo>,
    pub(super) renderers: HashMap<String, String>,
    pub(super) sprites: HashMap<String, SpriteComponent>,
}

pub(super) fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn field_value<'a>(doc: &'a str, prefix: &str) -> Option<&'a str> {
    doc.lines()
        .find_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
}

fn parse_doc_header(header: &str) -> Option<(i32, &str)> {
    let rest = header.trim().strip_prefix("!u!")?;
    let (class_id, file_id) = rest.split_once(" &")?;
    Some((class_id.parse().ok()?, file_id.trim()))
}

fn extract_file_id(value: &str) -> Option<String> {
    let start = value.find("fileID: ")? + "fileID: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    Some(tail[..end].trim().to_string())
}

fn extract_guid(value: &str) -> Option<String> {
    let start = value.find("guid: ")? + "guid: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    Some(tail[..end].trim().to_string())
}

fn parse_vec3(value: &str) -> Option<[f32; 3]> {
    let mut out = [0.0; 3];
    let trimmed = value.trim().trim_start_matches('{').trim_end_matches('}');
    let mut seen = [false; 3];
    for part in trimmed.split(',') {
        let (axis, raw) = part.trim().split_once(':')?;
        let index = match axis.trim() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => continue,
        };
        out[index] = raw.trim().parse().ok()?;
        seen[index] = true;
    }
    seen.iter().all(|v| *v).then_some(out)
}

fn parse_children(doc: &str) -> Vec<String> {
    let mut children = Vec::new();
    let mut in_children = false;
    for line in doc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("m_Children:") {
            in_children = true;
            continue;
        }
        if !in_children {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(child_id) = extract_file_id(rest) {
                children.push(child_id);
            }
            continue;
        }
        if trimmed.starts_with("m_Father:") {
            break;
        }
        if !trimmed.is_empty() {
            break;
        }
    }
    children
}

pub(super) fn guid_prefix(guid: &str) -> &str {
    guid.get(..8).unwrap_or(guid)
}

fn is_enabled(doc: &str) -> bool {
    field_value(doc, "m_Enabled: ") != Some("0")
}

pub(super) fn parse_prefab(raw: &str) -> Option<ParsedPrefab> {
    let mut root_game_object_id = None;
    let mut game_objects = HashMap::new();
    let mut transforms = HashMap::new();
    let mut renderers = HashMap::new();
    let mut sprites = HashMap::new();

    for doc in raw.split("--- ").skip(1) {
        let mut lines = doc.lines();
        let Some(header) = lines.next() else {
            continue;
        };
        let Some((class_id, file_id)) = parse_doc_header(header) else {
            continue;
        };

        match class_id {
            1001 => {
                root_game_object_id =
                    field_value(doc, "m_RootGameObject: ").and_then(extract_file_id);
            }
            1 => {
                let name = field_value(doc, "m_Name: ").unwrap_or(file_id).to_string();
                let tag = field_value(doc, "m_TagString: ").unwrap_or("").to_string();
                let active = field_value(doc, "m_IsActive: ") != Some("0");
                game_objects.insert(file_id.to_string(), GameObjectInfo { name, tag, active });
            }
            4 => {
                let Some(game_object_id) =
                    field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                else {
                    continue;
                };
                let local_pos = field_value(doc, "m_LocalPosition: ")
                    .and_then(parse_vec3)
                    .unwrap_or([0.0, 0.0, 0.0]);
                let local_scale = field_value(doc, "m_LocalScale: ")
                    .and_then(parse_vec3)
                    .unwrap_or([1.0, 1.0, 1.0]);
                let parent_id = field_value(doc, "m_Father: ")
                    .and_then(extract_file_id)
                    .filter(|id| id != "0");
                let root_order = field_value(doc, "m_RootOrder: ")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0);
                transforms.insert(
                    file_id.to_string(),
                    TransformInfo {
                        game_object_id,
                        local_pos,
                        local_scale,
                        parent_id,
                        children: parse_children(doc),
                        root_order,
                    },
                );
            }
            23 => {
                if !is_enabled(doc) {
                    continue;
                }
                let Some(game_object_id) =
                    field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                else {
                    continue;
                };
                let Some(material_guid) = doc.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix("- ")
                        .and_then(extract_guid)
                        .or_else(|| {
                            line.trim()
                                .starts_with("m_Materials:")
                                .then_some(None)
                                .flatten()
                        })
                }) else {
                    continue;
                };
                renderers.insert(game_object_id, guid_prefix(&material_guid).to_string());
            }
            114 => {
                if !is_enabled(doc) {
                    continue;
                }
                let Some(script_guid) = field_value(doc, "m_Script: ").and_then(extract_guid)
                else {
                    continue;
                };
                if script_guid != BG_SPRITE_SCRIPT_GUID {
                    continue;
                }
                let Some(game_object_id) =
                    field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                else {
                    continue;
                };
                let parse_f32 =
                    |key| field_value(doc, key).and_then(|value| value.parse::<f32>().ok());
                if parse_f32("m_textureWidth: ").is_none()
                    || parse_f32("m_textureHeight: ").is_none()
                {
                    continue;
                }
                let Some(sprite_width) = parse_f32("m_spriteWidth: ") else {
                    continue;
                };
                let Some(sprite_height) = parse_f32("m_spriteHeight: ") else {
                    continue;
                };
                let Some(uv_x) = parse_f32("m_UVx: ") else {
                    continue;
                };
                let Some(uv_y) = parse_f32("m_UVy: ") else {
                    continue;
                };
                let Some(width) = parse_f32("m_width: ") else {
                    continue;
                };
                let Some(height) = parse_f32("m_height: ") else {
                    continue;
                };
                let Some(subdiv) = parse_f32("m_atlasGridSubdivisions: ") else {
                    continue;
                };
                let border = parse_f32("m_border: ").unwrap_or(0.0);
                sprites.insert(
                    game_object_id,
                    SpriteComponent {
                        sprite_width,
                        sprite_height,
                        uv_x,
                        uv_y,
                        width,
                        height,
                        subdiv,
                        border,
                    },
                );
            }
            _ => {}
        }
    }

    let root_transform_id = root_game_object_id
        .as_ref()
        .and_then(|root_id| {
            transforms.iter().find_map(|(transform_id, transform)| {
                (transform.game_object_id == *root_id).then_some(transform_id.clone())
            })
        })
        .or_else(|| {
            transforms.iter().find_map(|(transform_id, transform)| {
                transform
                    .parent_id
                    .is_none()
                    .then_some(transform_id.clone())
            })
        })?;

    Some(ParsedPrefab {
        root_transform_id,
        game_objects,
        transforms,
        renderers,
        sprites,
    })
}

pub(super) fn load_textureloader_materials() -> HashMap<String, String> {
    use super::tables::BG_TEXTURELOADER_ASSET;
    let Some(raw) = read_embedded_text(BG_TEXTURELOADER_ASSET) else {
        log::error!("Missing embedded background textureloader asset");
        return HashMap::new();
    };

    let mut map = HashMap::new();
    let mut current_guid = None::<String>;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- material:") {
            current_guid = extract_guid(trimmed).map(|guid| guid_prefix(&guid).to_string());
            continue;
        }
        if let Some(asset_name) = trimmed.strip_prefix("assetName: ")
            && let Some(guid) = current_guid.take()
        {
            map.insert(guid, asset_name.trim().to_string());
        }
    }
    map
}

pub(super) fn asset_filename(asset_name: &str) -> String {
    format!("{asset_name}.png")
}

pub(super) fn is_sky_texture_asset(asset_name: &str) -> bool {
    asset_name.contains("Sky_Texture") || asset_name.contains("Backgrounds_sky")
}
