//! Background theme data — parses embedded Unity background prefabs at runtime.
//!
//! Each theme has multiple sprite entries organized into parallax layers.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;

/// Parallax layer with a speed factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgLayer {
    Sky,        // speed 0.8
    Camera,     // speed 1.0
    Further,    // speed 0.7
    Far,        // speed 0.6
    Near,       // speed 0.4
    Ground,     // speed 0.0
    Foreground, // speed -0.4
}

impl BgLayer {
    pub fn parallax_speed(&self) -> f32 {
        match self {
            BgLayer::Sky => 0.8,
            BgLayer::Camera => 1.0,
            BgLayer::Further => 0.7,
            BgLayer::Far => 0.6,
            BgLayer::Near => 0.4,
            BgLayer::Ground => 0.0,
            BgLayer::Foreground => -0.4,
        }
    }

    /// Render order (lower = drawn first = further back).
    pub fn order(&self) -> i32 {
        match self {
            BgLayer::Sky => 0,
            BgLayer::Camera => 1,
            BgLayer::Further => 2,
            BgLayer::Far => 3,
            BgLayer::Near => 4,
            BgLayer::Ground => 5,
            BgLayer::Foreground => 20,
        }
    }
}

/// A background sprite ready for rendering.
#[derive(Debug, Clone)]
pub struct BgSprite {
    pub name: String,
    pub atlas: Option<String>,
    pub fill_color: Option<[u8; 3]>,
    pub sky_texture: Option<String>,
    pub uv_x: f32,
    pub uv_y: f32,
    pub grid_w: f32,
    pub grid_h: f32,
    pub sprite_w: f32,
    pub sprite_h: f32,
    pub subdiv: f32,
    pub border: f32,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub layer: BgLayer,
    pub local_x: f32,
    pub local_y: f32,
    pub parent_group: String,
    pub tint: [f32; 4],
    pub alpha_blend: bool,
}

/// Full theme data with sprites organized by layer.
#[derive(Debug, Clone)]
pub struct BgTheme {
    pub sprites: Vec<BgSprite>,
    pub group_defaults: HashMap<String, [f32; 3]>,
    /// PositionSerializer child root-order → group name mapping.
    /// When present, per-level `childLocalPositions` arrays are applied as
    /// group position overrides (EP6 background prefabs).
    pub child_order: Vec<String>,
}

const BG_SPRITE_SCRIPT_GUID: &str = "b011dfa16a4475b746a1372ea41fdf05";
const BG_TEXTURELOADER_ASSET: &str = "unity/resources/textureloader.prefab";
const BG_THEME_PREFABS: &[(&str, &str)] = &[
    ("Cave", "unity/background/background_cave_01_set 1.prefab"),
    (
        "Morning",
        "unity/background/background_forest_01_set 1.prefab",
    ),
    ("Halloween", "unity/background/background_halloween.prefab"),
    ("Jungle", "unity/background/background_jungle_01_set.prefab"),
    ("Maya", "unity/background/background_mm_01_set.prefab"),
    (
        "MayaCave",
        "unity/background/background_mm_cave_01_set.prefab",
    ),
    (
        "MayaCaveDark",
        "unity/background/background_mm_cave_01_set_dark.prefab",
    ),
    (
        "MayaCave2Dark",
        "unity/background/background_mm_cave_02_set_dark.prefab",
    ),
    (
        "MayaHigh",
        "unity/background/background_mm_high_01_set.prefab",
    ),
    (
        "MayaTemple",
        "unity/background/background_mm_temple_01_set_01.prefab",
    ),
    ("Night", "unity/background/background_night_01_set 1.prefab"),
    (
        "Plateau",
        "unity/background/background_plateau_01_set.prefab",
    ),
];

#[derive(Debug, Clone)]
struct GameObjectInfo {
    name: String,
    tag: String,
    active: bool,
}

#[derive(Debug, Clone)]
struct TransformInfo {
    game_object_id: String,
    local_pos: [f32; 3],
    local_scale: [f32; 3],
    parent_id: Option<String>,
    children: Vec<String>,
    root_order: i32,
}

#[derive(Debug, Clone)]
struct SpriteComponent {
    sprite_width: f32,
    sprite_height: f32,
    uv_x: f32,
    uv_y: f32,
    width: f32,
    height: f32,
    subdiv: f32,
    border: f32,
}

#[derive(Debug, Clone)]
struct ParsedPrefab {
    root_transform_id: String,
    game_objects: HashMap<String, GameObjectInfo>,
    transforms: HashMap<String, TransformInfo>,
    renderers: HashMap<String, String>,
    sprites: HashMap<String, SpriteComponent>,
}

#[derive(Debug, Clone)]
struct GroupContext {
    name: String,
    layer: BgLayer,
    origin: [f32; 3],
}

fn read_embedded_text(path: &str) -> Option<String> {
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

fn guid_prefix(guid: &str) -> &str {
    guid.get(..8).unwrap_or(guid)
}

fn is_enabled(doc: &str) -> bool {
    field_value(doc, "m_Enabled: ") != Some("0")
}

fn parse_prefab(raw: &str) -> Option<ParsedPrefab> {
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

fn load_textureloader_materials() -> HashMap<String, String> {
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

fn classify_group_layer(tag: &str, group_name: &str) -> BgLayer {
    match tag {
        "ParallaxLayerSky" => BgLayer::Sky,
        "ParallaxLayerFixedFollowCamera" => BgLayer::Camera,
        "ParallaxLayerFurther" => BgLayer::Further,
        "ParallaxLayerFar" => BgLayer::Far,
        "ParallaxLayerNear" => BgLayer::Near,
        "ParallaxLayerForeground" => BgLayer::Foreground,
        "Ground" => BgLayer::Ground,
        _ => {
            let lower = group_name.to_ascii_lowercase();
            if lower.contains("sky") {
                BgLayer::Sky
            } else if lower.contains("further") {
                BgLayer::Further
            } else if lower.contains("foreground") || lower.starts_with("fglayer") {
                BgLayer::Foreground
            } else if lower.contains("far") {
                BgLayer::Far
            } else if lower.contains("near") {
                BgLayer::Near
            } else if lower.contains("cloud") || lower.contains("moon") || lower.contains("castle")
            {
                BgLayer::Camera
            } else {
                BgLayer::Ground
            }
        }
    }
}

fn supplemental_atlas_for_material(guid: &str) -> Option<&'static str> {
    match guid {
        "42e57a40" => Some("Background_Maya_Sheet_03.png"),
        "38ea809d" => Some("Background_Maya_Sheet_02.png"),
        "0de59521" => Some("Background_Maya_Sheet_02.png"),
        "c650b83a" => Some("Background_Maya_Sheet_04.png"),
        "d2458d0c" => Some("Background_Maya_Sheet_05.png"),
        "ac6e41ef" => Some("Background_Maya_Sheet_04.png"),
        "8429542c" => Some("Background_Maya_Sheet_04.png"),
        "ac9d3653" => Some("Background_Maya_Sheet_04.png"),
        "543a0873" => Some("Background_Maya_Sheet_03.png"),
        "ad0893eb" => Some("Background_Maya_Sheet_02.png"),
        "18df2da6" => Some("Background_Maya_Sheet_03.png"),
        "a79aee02" => Some("Background_Maya_Sheet_03.png"),
        "141823ce" => Some("Background_Maya_Sheet_03.png"),
        _ => None,
    }
}

fn fill_color_override(theme: &str, sprite_name: &str, parent_group: &str) -> Option<[u8; 3]> {
    match (theme, sprite_name, parent_group) {
        ("Cave", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("Cave", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("Jungle", "Background_Far_fill", "BGLayerFar") => Some([0x54, 0xaa, 0x44]),
        ("Jungle", "Background_Near_fill", "BGLayerNear") => Some([0x33, 0x88, 0x44]),
        ("Jungle", "Dummy", "Ocean") => Some([0x44, 0xaa, 0x99]),
        ("Maya", "Background_Far_fill", "BGLayerFar") => Some([0xcd, 0xab, 0x74]),
        ("Maya", "Background_Far_fill2", "BGLayerFurther") => Some([0xdd, 0xdd, 0xdd]),
        ("Maya", "Dummy", "Ocean") => Some([0x14, 0xba, 0xdc]),
        ("MayaCave", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("MayaCave", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("MayaCaveDark", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("MayaCaveDark", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("MayaCave2Dark", "Background_Sky_Fill1", "Background_Sky_Fill1") => {
            Some([0x04, 0x0b, 0x12])
        }
        ("MayaCave2Dark", "Grass_fill", "GroundLayer") => Some([0x42, 0x42, 0x29]),
        ("MayaTemple", "Background_Sky", "Background_Sky") => Some([0xfd, 0xf8, 0x7b]),
        ("Morning", "Background_Far_fill", "BGLayerFar") => Some([0x6d, 0x7e, 0x96]),
        ("Morning", "Background_Near_fill", "BGLayerNear") => Some([0x3f, 0x4b, 0x5b]),
        ("Morning", "Dummy", "Ocean") => Some([0x4f, 0x5f, 0x82]),
        ("Morning", "Fill", "BGLayerForeground") => Some([0x11, 0x11, 0x11]),
        ("Plateau", "Background_Far_fill", "BGLayerFar") => Some([0xcc, 0xaa, 0x21]),
        ("Plateau", "Background_Near_fill", "BGLayerNear") => Some([0x88, 0x77, 0x21]),
        ("Plateau", "Fill", "FGLayer") => Some([0x21, 0x44, 0x44]),
        ("Plateau", "Grass_fill", "GrassLayer") => Some([0x33, 0x77, 0x66]),
        _ => None,
    }
}

fn alpha_blend_override(
    theme: &str,
    sprite_name: &str,
    parent_group: &str,
    layer: BgLayer,
) -> bool {
    matches!(
        (theme, sprite_name, parent_group, layer),
        (
            "Morning",
            "Background_Jungle_02",
            "BGLayerFar",
            BgLayer::Far
        ) | ("Jungle", "Background_Jungle_02", "BGLayerFar", BgLayer::Far)
            | ("Maya", "Background_Maya_01", "BGLayerFar", BgLayer::Far)
            | ("Night", "Moon", _, BgLayer::Camera)
            | ("Halloween", _, _, BgLayer::Camera)
    )
}

fn uses_own_group_context(theme: &str, sprite_name: &str, parent_group: &str) -> bool {
    matches!(
        (theme, sprite_name, parent_group),
        ("MayaCave2Dark", "Background_Sky_Fill1", "Background_Sky")
    )
}

fn asset_filename(asset_name: &str) -> String {
    format!("{asset_name}.png")
}

fn is_sky_texture_asset(asset_name: &str) -> bool {
    asset_name.contains("Sky_Texture") || asset_name.contains("Backgrounds_sky")
}

struct BgSpriteBuildInput<'a> {
    theme_name: &'a str,
    group: &'a GroupContext,
    game_object: &'a GameObjectInfo,
    world_pos: [f32; 3],
    world_scale: [f32; 3],
    sprite_component: &'a SpriteComponent,
    material_guid: &'a str,
    textureloader_materials: &'a HashMap<String, String>,
}

fn build_bg_sprite(input: BgSpriteBuildInput<'_>) -> Option<BgSprite> {
    let BgSpriteBuildInput {
        theme_name,
        group,
        game_object,
        world_pos,
        world_scale,
        sprite_component,
        material_guid,
        textureloader_materials,
    } = input;
    let fill_color = fill_color_override(theme_name, &game_object.name, &group.name);
    let (atlas, sky_texture) = if fill_color.is_some() {
        (None, None)
    } else if let Some(asset_name) = textureloader_materials.get(material_guid) {
        if is_sky_texture_asset(asset_name) {
            (None, Some(asset_filename(asset_name)))
        } else {
            (Some(asset_filename(asset_name)), None)
        }
    } else if let Some(atlas_name) = supplemental_atlas_for_material(material_guid) {
        (Some(atlas_name.to_string()), None)
    } else {
        log::warn!(
            "Missing background material mapping for theme={}, sprite={}, guid={}",
            theme_name,
            game_object.name,
            material_guid
        );
        return None;
    };

    let is_group_root = game_object.name == group.name;
    Some(BgSprite {
        name: game_object.name.clone(),
        atlas,
        fill_color,
        sky_texture,
        uv_x: sprite_component.uv_x,
        uv_y: sprite_component.uv_y,
        grid_w: sprite_component.width,
        grid_h: sprite_component.height,
        sprite_w: sprite_component.sprite_width,
        sprite_h: sprite_component.sprite_height,
        subdiv: sprite_component.subdiv,
        border: sprite_component.border,
        world_x: world_pos[0],
        world_y: world_pos[1],
        world_z: world_pos[2],
        scale_x: world_scale[0],
        scale_y: world_scale[1],
        layer: group.layer,
        local_x: if is_group_root {
            world_pos[0]
        } else {
            world_pos[0] - group.origin[0]
        },
        local_y: if is_group_root {
            world_pos[1]
        } else {
            world_pos[1] - group.origin[1]
        },
        parent_group: group.name.clone(),
        tint: [1.0, 1.0, 1.0, 1.0],
        alpha_blend: alpha_blend_override(theme_name, &game_object.name, &group.name, group.layer),
    })
}

fn combine_world_pos(
    parent_pos: [f32; 3],
    parent_scale: [f32; 3],
    local_pos: [f32; 3],
) -> [f32; 3] {
    [
        parent_pos[0] + parent_scale[0] * local_pos[0],
        parent_pos[1] + parent_scale[1] * local_pos[1],
        parent_pos[2] + parent_scale[2] * local_pos[2],
    ]
}

fn combine_world_scale(parent_scale: [f32; 3], local_scale: [f32; 3]) -> [f32; 3] {
    [
        parent_scale[0] * local_scale[0],
        parent_scale[1] * local_scale[1],
        parent_scale[2] * local_scale[2],
    ]
}

struct BgTraverseCtx<'a> {
    theme_name: &'a str,
    prefab: &'a ParsedPrefab,
    textureloader_materials: &'a HashMap<String, String>,
}

struct BgTraverseOutput<'a> {
    group_defaults: &'a mut HashMap<String, [f32; 3]>,
    child_order: &'a mut Vec<String>,
    sprites: &'a mut Vec<BgSprite>,
}

fn traverse_group(
    ctx: &BgTraverseCtx<'_>,
    transform_id: &str,
    parent_pos: [f32; 3],
    parent_scale: [f32; 3],
    group: Option<GroupContext>,
    out: &mut BgTraverseOutput<'_>,
) {
    let Some(transform) = ctx.prefab.transforms.get(transform_id) else {
        return;
    };
    let Some(game_object) = ctx.prefab.game_objects.get(&transform.game_object_id) else {
        return;
    };
    if !game_object.active {
        return;
    }

    let world_pos = combine_world_pos(parent_pos, parent_scale, transform.local_pos);
    let world_scale = combine_world_scale(parent_scale, transform.local_scale);
    let group = match group {
        Some(group) => group,
        None => {
            out.group_defaults
                .insert(game_object.name.clone(), world_pos);
            out.child_order.push(game_object.name.clone());
            GroupContext {
                name: game_object.name.clone(),
                layer: classify_group_layer(&game_object.tag, &game_object.name),
                origin: world_pos,
            }
        }
    };

    let group = if uses_own_group_context(ctx.theme_name, &game_object.name, &group.name) {
        out.group_defaults
            .entry(game_object.name.clone())
            .or_insert(world_pos);
        GroupContext {
            name: game_object.name.clone(),
            layer: classify_group_layer(&game_object.tag, &game_object.name),
            origin: world_pos,
        }
    } else {
        group
    };

    if let Some(sprite_component) = ctx.prefab.sprites.get(&transform.game_object_id)
        && let Some(material_guid) = ctx.prefab.renderers.get(&transform.game_object_id)
        && let Some(sprite) = build_bg_sprite(BgSpriteBuildInput {
            theme_name: ctx.theme_name,
            group: &group,
            game_object,
            world_pos,
            world_scale,
            sprite_component,
            material_guid,
            textureloader_materials: ctx.textureloader_materials,
        })
    {
        out.sprites.push(sprite);
    }

    let mut children = transform.children.clone();
    children.sort_by_key(|child_id| {
        ctx.prefab
            .transforms
            .get(child_id)
            .map(|child| child.root_order)
            .unwrap_or_default()
    });
    for child_id in children {
        traverse_group(
            ctx,
            &child_id,
            world_pos,
            world_scale,
            Some(group.clone()),
            out,
        );
    }
}

fn build_theme(
    theme_name: &str,
    prefab_path: &str,
    textureloader_materials: &HashMap<String, String>,
) -> Option<BgTheme> {
    let raw = read_embedded_text(prefab_path)?;
    let prefab = parse_prefab(&raw)?;
    let root_transform = prefab.transforms.get(&prefab.root_transform_id)?;

    let mut group_defaults = HashMap::new();
    let mut child_order = Vec::new();
    let mut sprites = Vec::new();
    let ctx = BgTraverseCtx {
        theme_name,
        prefab: &prefab,
        textureloader_materials,
    };

    let mut children = root_transform.children.clone();
    children.sort_by_key(|child_id| {
        prefab
            .transforms
            .get(child_id)
            .map(|child| child.root_order)
            .unwrap_or_default()
    });
    {
        let mut out = BgTraverseOutput {
            group_defaults: &mut group_defaults,
            child_order: &mut child_order,
            sprites: &mut sprites,
        };
        for child_id in children {
            traverse_group(
                &ctx,
                &child_id,
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                None,
                &mut out,
            );
        }
    }

    sprites.sort_by(|a, b| {
        a.layer
            .order()
            .cmp(&b.layer.order())
            .then_with(|| b.world_z.partial_cmp(&a.world_z).unwrap_or(Ordering::Equal))
    });

    Some(BgTheme {
        sprites,
        group_defaults,
        child_order,
    })
}

static BG_THEMES: OnceLock<HashMap<String, BgTheme>> = OnceLock::new();

fn build_themes() -> HashMap<String, BgTheme> {
    let textureloader_materials = load_textureloader_materials();
    let mut themes = HashMap::with_capacity(BG_THEME_PREFABS.len());
    for (theme_name, prefab_path) in BG_THEME_PREFABS {
        match build_theme(theme_name, prefab_path, &textureloader_materials) {
            Some(theme) => {
                themes.insert((*theme_name).to_string(), theme);
            }
            None => {
                log::error!(
                    "Failed to build embedded background theme {} from {}",
                    theme_name,
                    prefab_path
                );
            }
        }
    }
    themes
}

/// Get background theme data by name.
pub fn get_theme(name: &str) -> Option<&'static BgTheme> {
    let themes = BG_THEMES.get_or_init(build_themes);
    themes.get(name)
}

/// Parsed BG overrides: group/sprite name → partial position override.
#[derive(Default)]
pub struct BgOverrides {
    pub groups: HashMap<String, [Option<f32>; 3]>,
    pub sprites: HashMap<String, [Option<f32>; 3]>,
}

/// Parse BG override text from a BackgroundObject's PrefabOverrideData.
pub fn parse_bg_overrides(raw: &str) -> BgOverrides {
    let mut result = BgOverrides::default();
    let mut current_group = String::new();
    let mut current_sprite = String::new();
    // "group" or "sprite" — what m_LocalPosition applies to
    let mut parsing_for = "";

    for line in raw.lines() {
        let stripped = line.trim_end_matches('\r');
        let depth = stripped.len() - stripped.trim_start_matches('\t').len();
        let content = stripped.trim();
        if content.is_empty() {
            continue;
        }

        if depth == 1 && content.starts_with("GameObject ") {
            current_group = content[11..].trim().to_string();
            current_sprite.clear();
            parsing_for = "";
        } else if depth == 2 && content.starts_with("GameObject ") {
            current_sprite = content[11..].trim().to_string();
            parsing_for = "";
        } else if depth == 2 && content == "Component UnityEngine.Transform" {
            parsing_for = "group";
        } else if depth == 3 && content == "Component UnityEngine.Transform" {
            parsing_for = "sprite";
        } else if content.starts_with("Float ")
            && let Some(rest) = content.strip_prefix("Float ")
        {
            // Parse "x = 1.23" or "y = -4.56"
            let parts: Vec<&str> = rest.splitn(2, '=').collect();
            if parts.len() == 2 {
                let axis = parts[0].trim();
                if let Ok(val) = parts[1].trim().parse::<f32>() {
                    let (target_name, target_map) = match parsing_for {
                        "group" => (&current_group, &mut result.groups),
                        "sprite" => (&current_sprite, &mut result.sprites),
                        _ => continue,
                    };
                    if !target_name.is_empty() {
                        let entry = target_map
                            .entry(target_name.clone())
                            .or_insert([None, None, None]);
                        match axis {
                            "x" => entry[0] = Some(val),
                            "y" => entry[1] = Some(val),
                            "z" => entry[2] = Some(val),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    result
}

/// Apply BG overrides to a theme's sprites, returning modified copies with updated positions.
pub fn apply_bg_overrides(theme: &BgTheme, overrides: &BgOverrides) -> Vec<BgSprite> {
    if overrides.groups.is_empty() && overrides.sprites.is_empty() {
        return theme.sprites.clone();
    }
    theme
        .sprites
        .iter()
        .map(|s| {
            let defaults = match theme.group_defaults.get(&s.parent_group) {
                Some(d) => d,
                None => return s.clone(),
            };
            let group_ovr = overrides.groups.get(&s.parent_group);
            let sprite_ovr = overrides.sprites.get(&s.name);
            if group_ovr.is_none() && sprite_ovr.is_none() {
                return s.clone();
            }
            let gx = group_ovr.and_then(|o| o[0]).unwrap_or(defaults[0]);
            let gy = group_ovr.and_then(|o| o[1]).unwrap_or(defaults[1]);
            let gz = group_ovr.and_then(|o| o[2]).unwrap_or(defaults[2]);
            // When a sprite IS its own group parent (name == parentGroup),
            // its default localY already equals the group default position
            // (both represent the same Transform.m_LocalPosition). Using
            // localY here would double-count the offset. Treat it as 0.
            let is_group_root = s.name == s.parent_group;
            let lx = sprite_ovr.and_then(|o| o[0]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_x
            });
            let ly = sprite_ovr.and_then(|o| o[1]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_y
            });
            let new_x = gx + lx;
            let new_y = gy + ly;
            let sprite_local_z = s.world_z - defaults[2];
            let new_z = gz + sprite_local_z;
            let mut out = s.clone();
            out.world_x = new_x;
            out.world_y = new_y;
            out.world_z = new_z;
            out
        })
        .collect()
}

/// Parse PositionSerializer `childLocalPositions` into group overrides.
///
/// EP6 background prefabs use a `PositionSerializer` component with an array
/// of child positions (indexed by `m_RootOrder`).  The `child_order` slice
/// maps each array index to the corresponding background group name so we
/// can produce the same `BgOverrides` struct that `apply_bg_overrides` expects.
pub fn parse_position_serializer_overrides(raw: &str, child_order: &[String]) -> BgOverrides {
    let mut result = BgOverrides {
        groups: HashMap::new(),
        sprites: HashMap::new(),
    };

    if !raw.contains("PositionSerializer") || child_order.is_empty() {
        return result;
    }

    let mut current_element: Option<usize> = None;
    let mut current_pos: [Option<f32>; 3] = [None, None, None];

    for line in raw.lines() {
        let content = line.trim();
        if let Some(rest) = content.strip_prefix("Element ") {
            // Flush previous element
            if let Some(idx) = current_element
                && idx < child_order.len()
                && !child_order[idx].is_empty()
            {
                result.groups.insert(child_order[idx].clone(), current_pos);
            }
            current_element = rest.trim().parse::<usize>().ok();
            current_pos = [None, None, None];
        } else if let Some(rest) = content.strip_prefix("Float ")
            && let Some((axis, val_str)) = rest.split_once('=')
            && let Ok(val) = val_str.trim().parse::<f32>()
        {
            match axis.trim() {
                "x" => current_pos[0] = Some(val),
                "y" => current_pos[1] = Some(val),
                "z" => current_pos[2] = Some(val),
                _ => {}
            }
        }
    }
    // Flush last element
    if let Some(idx) = current_element
        && idx < child_order.len()
        && !child_order[idx].is_empty()
    {
        result.groups.insert(child_order[idx].clone(), current_pos);
    }

    result
}

/// All known background atlas filenames.
pub fn bg_atlas_files() -> &'static [&'static str] {
    &[
        "Background_Cave_Sheet_01.png",
        "Background_Halloween_Sheet_01.png",
        "Background_Jungle_Sheet_01.png",
        "Background_Jungle_Sheet_02.png",
        "Background_Maya_Sheet_01.png",
        "Background_Maya_Sheet_02.png",
        "Background_Maya_Sheet_03.png",
        "Background_Maya_Sheet_04.png",
        "Background_Maya_Sheet_05.png",
        "Background_Morning_Sheet_01.png",
        "Background_Morning_Sheet_02.png",
        "Background_Night_Sheet_01.png",
        "Background_Plateaus_Sheet_01.png",
    ]
}

/// All known sky texture filenames.
pub fn sky_texture_files() -> &'static [&'static str] {
    &[
        "Halloween_Sky_Texture.png",
        "Jungle_Sky_Texture.png",
        "Maya_Backgrounds_sky.png",
        "Morning_Sky_Texture.png",
        "Night_Sky_Texture.png",
        "Plateau_Sky_Texture.png",
    ]
}

#[cfg(test)]
mod tests {
    use super::get_theme;

    #[test]
    fn maya_cave2dark_sky_fill_uses_own_group_and_fill_color() {
        let Some(theme) = get_theme("MayaCave2Dark") else {
            panic!("missing MayaCave2Dark theme");
        };
        let Some(sprite) = theme
            .sprites
            .iter()
            .find(|sprite| sprite.name == "Background_Sky_Fill1")
        else {
            panic!("missing Background_Sky_Fill1");
        };

        assert_eq!(sprite.parent_group, "Background_Sky_Fill1");
        assert_eq!(sprite.fill_color, Some([0x04, 0x0b, 0x12]));
        assert!(sprite.atlas.is_none());
    }

    #[test]
    fn maya_cave2dark_fg_uses_sheet_02() {
        let Some(theme) = get_theme("MayaCave2Dark") else {
            panic!("missing MayaCave2Dark theme");
        };

        for sprite in theme.sprites.iter().filter(|sprite| {
            sprite.parent_group == "FGLayer"
                && matches!(sprite.name.as_str(), "Fill2" | "Pillars01")
        }) {
            assert_eq!(
                sprite.atlas.as_deref(),
                Some("Background_Maya_Sheet_02.png"),
                "unexpected atlas for {}",
                sprite.name
            );
        }
    }

    #[test]
    fn maya_temple_uses_expected_maya_sheets() {
        let Some(theme) = get_theme("MayaTemple") else {
            panic!("missing MayaTemple theme");
        };

        for sprite in theme.sprites.iter().filter(|sprite| {
            sprite.parent_group == "FGLayer"
                && matches!(
                    sprite.name.as_str(),
                    "Background_Maya_Temple_FG"
                        | "Background_Maya_Temple_FG_Fill"
                        | "Background_Maya_Temple_Near_Base"
                        | "Background_Maya_Temple_Near_Top"
                )
        }) {
            assert_eq!(
                sprite.atlas.as_deref(),
                Some("Background_Maya_Sheet_05.png"),
                "unexpected FG atlas for {}",
                sprite.name
            );
        }

        for sprite in theme.sprites.iter().filter(|sprite| {
            sprite.parent_group == "BGLayerNearBottom"
                && matches!(
                    sprite.name.as_str(),
                    "Background_Maya_Temple_Near_01"
                        | "Background_Maya_Temple_Near_02"
                        | "Background_Maya_Temple_Near_03"
                        | "Background_Maya_Temple_Near_04"
                )
        }) {
            assert_eq!(
                sprite.atlas.as_deref(),
                Some("Background_Maya_Sheet_04.png"),
                "unexpected near-bottom atlas for {}",
                sprite.name
            );
        }
    }
}
