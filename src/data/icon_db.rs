//! Icon layer database — rebuilds multi-layer part icons from embedded Unity raw assets.
//!
//! Each part type/custom index maps to one or more sprite layers, baked from
//! Part_*_NN_SET prefab hierarchies plus Sprites.bytes and spritemapping.bytes.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;

const SPRITE_SCRIPT_GUID: &str = "eaa85264a31f76994888187c4d3a9fb9";
const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";
const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_DIR_ASSET: &str = "unity/prefabs";
const WORLD_SCALE: f32 = 10.0 / 768.0;

type Mat2x3 = (f32, f32, f32, f32, f32, f32);

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
    uv_x_norm: f32,
    uv_y_norm: f32,
    uv_w_norm: f32,
    uv_h_norm: f32,
}

#[derive(Debug, Clone)]
struct GameObjectInfo {
    name: String,
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

#[derive(Default)]
struct ParsedPrefab {
    part_type: Option<i32>,
    custom_part_index: Option<i32>,
    z_offset: f32,
    game_objects: HashMap<String, GameObjectInfo>,
    transforms: HashMap<String, TransformInfo>,
    sprites: HashMap<String, SpriteComponent>,
    renderers: HashMap<String, RendererInfo>,
}

// ── Global singleton ──

/// Per-part info: z_offset + layers.
pub struct PartInfo {
    pub z_offset: f32,
    pub layers: Vec<IconLayer>,
}

static ICON_DB: OnceLock<HashMap<String, PartInfo>> = OnceLock::new();

fn load() -> HashMap<String, PartInfo> {
    let runtime_sprites = load_runtime_sprites();
    if runtime_sprites.is_empty() {
        log::error!("Failed to build runtime sprite metadata for icon layer database");
        return HashMap::new();
    }

    let Some(manifest) = read_embedded_text(PREFAB_MANIFEST_ASSET) else {
        log::error!(
            "Failed to read embedded prefab manifest for icon layer database: {}",
            PREFAB_MANIFEST_ASSET
        );
        return HashMap::new();
    };

    let mut map = HashMap::new();
    for filename in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !filename.starts_with("Part_") || !filename.ends_with("_SET.prefab") {
            continue;
        }
        let Some(default_custom_part_index) = default_custom_part_index(filename) else {
            continue;
        };

        let asset_path = format!("{}/{}", PREFAB_DIR_ASSET, filename);
        let Some(text) = read_embedded_text(&asset_path) else {
            log::warn!(
                "Missing embedded part prefab for icon layers: {}",
                asset_path
            );
            continue;
        };

        let parsed = parse_prefab(&text);
        let Some(part_type) = parsed.part_type else {
            continue;
        };
        let custom_part_index = parsed
            .custom_part_index
            .unwrap_or(default_custom_part_index);
        let layers = build_part_layers(&parsed, &runtime_sprites);
        if layers.is_empty() {
            continue;
        }

        map.insert(
            format!("{part_type}.{custom_part_index}"),
            PartInfo {
                z_offset: parsed.z_offset,
                layers,
            },
        );
    }

    map
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

fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn default_custom_part_index(filename: &str) -> Option<i32> {
    let stem = filename.strip_suffix(".prefab")?;
    let stem = stem.strip_suffix("_SET")?;
    let (_, suffix) = stem.rsplit_once('_')?;
    let variant: i32 = suffix.parse().ok()?;
    Some(variant - 1)
}

fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
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

fn build_part_layers(
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
) -> Vec<IconLayer> {
    let Some(root_transform_id) = parsed
        .transforms
        .iter()
        .find(|(_, transform)| transform.father == "0")
        .map(|(id, _)| id.clone())
    else {
        return Vec::new();
    };

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
    let ctx = IconTraverseCtx {
        parsed,
        sprite_by_go: &sprite_by_go,
        renderer_by_go: &renderer_by_go,
        runtime_sprites,
    };

    let mut layers = Vec::new();
    traverse_part(
        &root_transform_id,
        &ctx,
        (1.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        0.0,
        true,
        &mut layers,
    );
    layers
}

struct IconTraverseCtx<'a> {
    parsed: &'a ParsedPrefab,
    sprite_by_go: &'a HashMap<&'a str, &'a SpriteComponent>,
    renderer_by_go: &'a HashMap<&'a str, &'a RendererInfo>,
    runtime_sprites: &'a HashMap<String, RuntimeSpriteMeta>,
}

fn traverse_part(
    transform_id: &str,
    ctx: &IconTraverseCtx<'_>,
    parent_mat: Mat2x3,
    parent_z: f32,
    is_root: bool,
    out_layers: &mut Vec<IconLayer>,
) {
    let Some(transform) = ctx.parsed.transforms.get(transform_id) else {
        return;
    };

    let (current_mat, current_z) = if is_root {
        ((1.0, 0.0, 0.0, 1.0, 0.0, 0.0), 0.0)
    } else {
        let local = make_local_trs(
            [transform.pos_x, transform.pos_y],
            [transform.scale_x, transform.scale_y],
            [transform.qx, transform.qy, transform.qz, transform.qw],
        );
        (mat_compose(parent_mat, local), parent_z + transform.pos_z)
    };

    let game_object_id = transform.game_object_id.as_str();
    if let (Some(sprite), Some(renderer)) = (
        ctx.sprite_by_go.get(game_object_id),
        ctx.renderer_by_go.get(game_object_id),
    ) && renderer.enabled
        && let Some(runtime_sprite) = ctx.runtime_sprites.get(&sprite.sprite_id)
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
            (pivot_x - half_w, pivot_y - half_h),
            (pivot_x - half_w, pivot_y + half_h),
            (pivot_x + half_w, pivot_y + half_h),
            (pivot_x + half_w, pivot_y - half_h),
        ];
        let baked = base_vertices.map(|(x, y)| mat_apply(current_mat, x, y));
        let go_name = ctx
            .parsed
            .game_objects
            .get(game_object_id)
            .map(|go| go.name.clone())
            .unwrap_or_default();

        out_layers.push(IconLayer {
            go_name,
            atlas: atlas.to_string(),
            uv_x: runtime_sprite.uv_x_norm,
            uv_y: runtime_sprite.uv_y_norm,
            uv_w: runtime_sprite.uv_w_norm,
            uv_h: runtime_sprite.uv_h_norm,
            z_local: round_six(current_z),
            v0_x: round_six(baked[0].0),
            v0_y: round_six(baked[0].1),
            v1_x: round_six(baked[1].0),
            v1_y: round_six(baked[1].1),
            v2_x: round_six(baked[2].0),
            v2_y: round_six(baked[2].1),
            v3_x: round_six(baked[3].0),
            v3_y: round_six(baked[3].1),
        });
    }

    for child_id in &transform.children {
        traverse_part(child_id, ctx, current_mat, current_z, false, out_layers);
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

fn round_six(value: f32) -> f32 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::get_part_info;

    #[test]
    fn balloon_variant_keeps_embedded_face_layer() {
        let Some(info) = get_part_info(1, 7) else {
            panic!("expected embedded icon layers for Balloon custom part 7");
        };

        assert_eq!(info.layers.len(), 2);
        let face = info
            .layers
            .iter()
            .find(|layer| layer.go_name == "Face1")
            .expect("expected Face1 overlay layer");

        assert_eq!(face.atlas, "IngameAtlas3.png");
        assert!(
            (face.z_local - (-0.05)).abs() < 0.000_01,
            "unexpected z_local: {}",
            face.z_local
        );
        assert!(
            (face.v0_x - (-0.299479)).abs() < 0.000_01,
            "unexpected v0_x: {}",
            face.v0_x
        );
        assert!(
            (face.v2_y - 0.295312).abs() < 0.000_01,
            "unexpected v2_y: {}",
            face.v2_y
        );
    }
}
