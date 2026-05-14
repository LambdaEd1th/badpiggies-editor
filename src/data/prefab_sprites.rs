//! Unity prefab multi-sprite database.
//!
//! Parses prefab child Sprite and unmanaged atlas components from the decompiled
//! Unity project and exposes baked local quads for prefabs that render as one
//! or more visible child sprites.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;
use crate::data::sprite_db::UvRect;
use crate::domain::types::Vec2;

const SPRITE_SCRIPT_GUID: &str = "eaa85264a31f76994888187c4d3a9fb9";
const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";
const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_DIR_ASSET: &str = "unity/prefabs";
const UNMANAGED_ATLAS: &str = "Props_Generic_Sheet_01.png";
const WORLD_SCALE: f32 = 10.0 / 768.0;

type Mat2x3 = (f32, f32, f32, f32, f32, f32);

#[derive(Debug, Clone)]
pub struct PrefabSpriteLayer {
    pub atlas: String,
    pub uv: UvRect,
    pub z_local: f32,
    /// Vertex order matches Unity mesh creation: BL, TL, TR, BR.
    pub vertices: [Vec2; 4],
}

#[derive(Debug, Clone)]
struct RuntimeSpriteMeta {
    selection_x: i32,
    selection_y: i32,
    selection_w: i32,
    selection_h: i32,
    pivot_x: i32,
    pivot_y: i32,
    uv_x: i32,
    uv_y: i32,
    width: i32,
    height: i32,
    uv: UvRect,
}

#[derive(Debug, Clone)]
struct GameObjectInfo {
    name: String,
    active: bool,
}

#[derive(Debug, Clone)]
struct TransformInfo {
    game_object_id: String,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    scale_x: f32,
    scale_y: f32,
    qx: f32,
    qy: f32,
    qz: f32,
    qw: f32,
    father: String,
    children: Vec<String>,
}

#[derive(Debug, Clone)]
struct SpriteComponent {
    game_object_id: String,
    sprite_id: String,
    scale_x: f32,
    scale_y: f32,
    pivot_x: f32,
    pivot_y: f32,
}

#[derive(Debug, Clone)]
struct RendererInfo {
    game_object_id: String,
    material_guid: String,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct UnmanagedSpriteComponent {
    uv: UvRect,
    world_w: f32,
    world_h: f32,
}

#[derive(Default)]
struct ParsedPrefab {
    game_objects: HashMap<String, GameObjectInfo>,
    transforms: HashMap<String, TransformInfo>,
    sprites: HashMap<String, SpriteComponent>,
    renderers: HashMap<String, RendererInfo>,
    unmanaged_sprites: HashMap<String, UnmanagedSpriteComponent>,
}

fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn runtime_sprites() -> &'static HashMap<String, RuntimeSpriteMeta> {
    static INSTANCE: OnceLock<HashMap<String, RuntimeSpriteMeta>> = OnceLock::new();
    INSTANCE.get_or_init(load_runtime_sprites)
}

fn multi_sprite_prefabs() -> &'static HashMap<String, Vec<PrefabSpriteLayer>> {
    static INSTANCE: OnceLock<HashMap<String, Vec<PrefabSpriteLayer>>> = OnceLock::new();
    INSTANCE.get_or_init(load_multi_sprite_prefabs)
}

pub fn get_multi_sprite_layers(name: &str) -> Option<&'static [PrefabSpriteLayer]> {
    let db = multi_sprite_prefabs();
    db.get(name)
        .or_else(|| name.split(" (").next().and_then(|base| db.get(base)))
        .map(Vec::as_slice)
}

fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
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

fn load_multi_sprite_prefabs() -> HashMap<String, Vec<PrefabSpriteLayer>> {
    let runtime = runtime_sprites();
    let Some(manifest) = read_embedded_text(PREFAB_MANIFEST_ASSET) else {
        log::error!(
            "Failed to read embedded prefab manifest for multi-sprite support: {}",
            PREFAB_MANIFEST_ASSET
        );
        return HashMap::new();
    };

    let mut prefabs = HashMap::new();
    for filename in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !filename.ends_with(".prefab") {
            continue;
        }
        let Some(name) = filename.strip_suffix(".prefab").map(str::to_string) else {
            continue;
        };
        let asset_path = format!("{}/{}", PREFAB_DIR_ASSET, filename);
        let Some(layers) = parse_prefab_layers(&name, &asset_path, runtime) else {
            continue;
        };
        if layers.len() > 1 || (name.starts_with("GoalArea") && !layers.is_empty()) {
            prefabs.insert(name, layers);
        }
    }
    prefabs
}

fn parse_prefab_layers(
    prefab_name: &str,
    asset_path: &str,
    runtime: &HashMap<String, RuntimeSpriteMeta>,
) -> Option<Vec<PrefabSpriteLayer>> {
    let text = read_embedded_text(asset_path)?;
    let parsed = parse_prefab(&text);

    let root_transform_id = parsed
        .transforms
        .iter()
        .find(|(_, transform)| transform.father == "0")
        .map(|(id, _)| id.clone())?;

    let sprite_by_go: HashMap<&str, &SpriteComponent> = parsed
        .sprites
        .values()
        .map(|sprite| (sprite.game_object_id.as_str(), sprite))
        .collect();
    let renderer_by_go: HashMap<&str, &RendererInfo> = parsed
        .renderers
        .values()
        .map(|renderer| (renderer.game_object_id.as_str(), renderer))
        .collect();
    let ctx = PrefabTraverseCtx {
        parsed: &parsed,
        sprite_by_go: &sprite_by_go,
        renderer_by_go: &renderer_by_go,
        runtime,
        root_name: prefab_name,
    };

    let identity = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let mut layers = Vec::new();
    traverse_prefab(&root_transform_id, &ctx, identity, 0.0, true, &mut layers);

    layers.sort_by(|a, b| b.z_local.partial_cmp(&a.z_local).unwrap_or(Ordering::Equal));
    (!layers.is_empty()).then_some(layers)
}

struct PrefabTraverseCtx<'a> {
    parsed: &'a ParsedPrefab,
    sprite_by_go: &'a HashMap<&'a str, &'a SpriteComponent>,
    renderer_by_go: &'a HashMap<&'a str, &'a RendererInfo>,
    runtime: &'a HashMap<String, RuntimeSpriteMeta>,
    root_name: &'a str,
}

fn traverse_prefab(
    transform_id: &str,
    ctx: &PrefabTraverseCtx<'_>,
    parent_mat: Mat2x3,
    parent_z: f32,
    is_root: bool,
    out_layers: &mut Vec<PrefabSpriteLayer>,
) {
    let Some(transform) = ctx.parsed.transforms.get(transform_id) else {
        return;
    };

    let current_mat;
    let current_z;
    if is_root {
        current_mat = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        current_z = 0.0;
    } else {
        let local = make_local_trs(
            [transform.pos_x, transform.pos_y],
            [transform.scale_x, transform.scale_y],
            [transform.qx, transform.qy, transform.qz, transform.qw],
        );
        current_mat = mat_compose(parent_mat, local);
        current_z = parent_z + transform.pos_z;
    }

    let game_object_id = transform.game_object_id.as_str();
    let Some(game_object) = ctx.parsed.game_objects.get(game_object_id) else {
        return;
    };
    if !game_object.active {
        return;
    }

    let skip_goal_glow = ctx.root_name.starts_with("GoalArea") && game_object.name == "Glow";

    if !skip_goal_glow
        && let (Some(sprite), Some(renderer)) = (
            ctx.sprite_by_go.get(game_object_id),
            ctx.renderer_by_go.get(game_object_id),
        ) && renderer.enabled
        && let Some(runtime_sprite) = ctx.runtime.get(&sprite.sprite_id)
        && let Some(atlas) = atlas_for_material_guid(&renderer.material_guid)
    {
        let mesh_w = (sprite.scale_x * runtime_sprite.width as f32) as i32;
        let mesh_h = (sprite.scale_y * runtime_sprite.height as f32) as i32;

        let dx = (runtime_sprite.selection_x + runtime_sprite.selection_w / 2)
            - (runtime_sprite.uv_x + runtime_sprite.width / 2);
        let dy = (runtime_sprite.selection_y + runtime_sprite.selection_h / 2)
            - (runtime_sprite.uv_y + runtime_sprite.height / 2);
        let sprite_pivot_x =
            (sprite.scale_x * (dx + runtime_sprite.pivot_x + sprite.pivot_x as i32) as f32) as i32;
        let sprite_pivot_y =
            (sprite.scale_y * (dy + runtime_sprite.pivot_y + sprite.pivot_y as i32) as f32) as i32;

        let half_w = mesh_w as f32 * WORLD_SCALE;
        let half_h = mesh_h as f32 * WORLD_SCALE;
        let pivot_x = -2.0 * sprite_pivot_x as f32 * WORLD_SCALE;
        let pivot_y = -2.0 * sprite_pivot_y as f32 * WORLD_SCALE;
        let base_vertices = [
            Vec2 {
                x: pivot_x - half_w,
                y: pivot_y - half_h,
            },
            Vec2 {
                x: pivot_x - half_w,
                y: pivot_y + half_h,
            },
            Vec2 {
                x: pivot_x + half_w,
                y: pivot_y + half_h,
            },
            Vec2 {
                x: pivot_x + half_w,
                y: pivot_y - half_h,
            },
        ];

        let vertices = base_vertices.map(|vertex| {
            let (x, y) = mat_apply(current_mat, vertex.x, vertex.y);
            Vec2 { x, y }
        });

        out_layers.push(PrefabSpriteLayer {
            atlas: atlas.to_string(),
            uv: runtime_sprite.uv,
            z_local: current_z,
            vertices,
        });
    }

    if !skip_goal_glow
        && let Some(renderer) = ctx.renderer_by_go.get(game_object_id)
        && renderer.enabled
        && let Some(sprite) = ctx.parsed.unmanaged_sprites.get(game_object_id)
    {
        let base_vertices = [
            Vec2 {
                x: -sprite.world_w,
                y: -sprite.world_h,
            },
            Vec2 {
                x: -sprite.world_w,
                y: sprite.world_h,
            },
            Vec2 {
                x: sprite.world_w,
                y: sprite.world_h,
            },
            Vec2 {
                x: sprite.world_w,
                y: -sprite.world_h,
            },
        ];

        let vertices = base_vertices.map(|vertex| {
            let (x, y) = mat_apply(current_mat, vertex.x, vertex.y);
            Vec2 { x, y }
        });

        out_layers.push(PrefabSpriteLayer {
            atlas: UNMANAGED_ATLAS.to_string(),
            uv: sprite.uv,
            z_local: current_z,
            vertices,
        });
    }

    for child_id in &transform.children {
        traverse_prefab(child_id, ctx, current_mat, current_z, false, out_layers);
    }
}

fn parse_prefab(text: &str) -> ParsedPrefab {
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

fn parse_mono_behaviour(
    doc: &str,
    file_id: &str,
    parsed: &mut ParsedPrefab,
) {
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

fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    let prefix = material_guid.get(..8).unwrap_or(material_guid);
    match prefix {
        "ce5a9931" | "d645821c" | "125eb5b4" | "0e790fab" | "353dd850" => Some("IngameAtlas.png"),
        "211b2b9c" | "aca6a4c6" | "765e60c2" | "4ab535f3" | "4eeb62bc" => Some("IngameAtlas2.png"),
        "2a21c011" | "ad767d84" | "7192b13e" | "a6f51d97" | "7975d66d" => Some("IngameAtlas3.png"),
        _ => None,
    }
}

fn quat_to_z_angle(qx: f32, qy: f32, qz: f32, qw: f32) -> f32 {
    (2.0 * (qw * qz + qx * qy)).atan2(1.0 - 2.0 * (qy * qy + qz * qz))
}

fn make_local_trs(position: [f32; 2], scale: [f32; 2], rotation: [f32; 4]) -> Mat2x3 {
    let angle = quat_to_z_angle(rotation[0], rotation[1], rotation[2], rotation[3]);
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    (
        cos_a * scale[0],
        -sin_a * scale[1],
        sin_a * scale[0],
        cos_a * scale[1],
        position[0],
        position[1],
    )
}

fn mat_compose(m1: Mat2x3, m2: Mat2x3) -> Mat2x3 {
    let (a1, b1, c1, d1, tx1, ty1) = m1;
    let (a2, b2, c2, d2, tx2, ty2) = m2;
    (
        a1 * a2 + b1 * c2,
        a1 * b2 + b1 * d2,
        c1 * a2 + d1 * c2,
        c1 * b2 + d1 * d2,
        a1 * tx2 + b1 * ty2 + tx1,
        c1 * tx2 + d1 * ty2 + ty1,
    )
}

fn mat_apply(m: Mat2x3, x: f32, y: f32) -> (f32, f32) {
    let (a, b, c, d, tx, ty) = m;
    (a * x + b * y + tx, c * x + d * y + ty)
}

#[cfg(test)]
mod tests {
    use super::get_multi_sprite_layers;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn embedded_icon_pig_prefab_has_multiple_layers() {
        let Some(layers) = get_multi_sprite_layers("Icon_Pig_01") else {
            panic!("expected embedded multi-sprite data for Icon_Pig_01");
        };
        assert!(
            layers.len() >= 3,
            "expected Icon_Pig_01 to keep multiple embedded sprite layers"
        );
    }

    #[test]
    fn goal_area_mm_gold_prefab_keeps_runtime_icon_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_MM_Gold") else {
            panic!("expected prefab layers for GoalArea_MM_Gold");
        };
        let Some(icon_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "IngameAtlas2.png")
        else {
            panic!("expected GoalArea_MM_Gold icon layer");
        };
        assert_close(icon_layer.uv.x, 0.5419922);
        assert_close(icon_layer.uv.y, 0.7719727);
        assert_close(icon_layer.uv.w, 0.02929688);
        assert_close(icon_layer.uv.h, 0.06054688);
    }

    #[test]
    fn goal_area_01_prefab_keeps_achievement_icon_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_01") else {
            panic!("expected prefab layers for GoalArea_01");
        };
        let Some(icon_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "Props_Generic_Sheet_01.png")
        else {
            panic!("expected GoalArea_01 achievement icon layer");
        };
        assert_close(icon_layer.uv.x, 0.0);
        assert_close(icon_layer.uv.y, 0.0);
        assert_close(icon_layer.uv.w, 0.125);
        assert_close(icon_layer.uv.h, 0.125);
    }

    #[test]
    fn goal_area_star_level_prefab_keeps_hat_layer() {
        let Some(layers) = get_multi_sprite_layers("GoalArea_StarLevel") else {
            panic!("expected prefab layers for GoalArea_StarLevel");
        };
        let Some(hat_layer) = layers
            .iter()
            .find(|layer| layer.atlas == "Props_Generic_Sheet_01.png")
        else {
            panic!("expected GoalArea_StarLevel hat layer");
        };
        assert_close(hat_layer.uv.x, 0.75);
        assert_close(hat_layer.uv.y, 0.25);
        assert_close(hat_layer.uv.w, 0.125);
        assert_close(hat_layer.uv.h, 0.125);
    }
}
