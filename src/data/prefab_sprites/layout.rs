//! Flatten Unity prefab hierarchies into baked sprite quads.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::domain::types::Vec2;

use super::math::{IDENTITY_MAT, Mat2x3, make_local_trs, mat_apply, mat_compose};
use super::parse::{WORLD_SCALE, atlas_for_material_guid, parse_prefab, read_embedded_text};
use super::types::{
    ParsedPrefab, PrefabSpriteLayer, RendererInfo, RuntimeSpriteMeta, SpriteComponent,
};

const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_DIR_ASSET: &str = "unity/prefabs";
const UNMANAGED_ATLAS: &str = "Props_Generic_Sheet_01.png";

pub(super) fn load_multi_sprite_prefabs(
    runtime: &HashMap<String, RuntimeSpriteMeta>,
) -> HashMap<String, Vec<PrefabSpriteLayer>> {
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

    let mut layers = Vec::new();
    traverse_prefab(&root_transform_id, &ctx, IDENTITY_MAT, 0.0, true, &mut layers);

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
        current_mat = IDENTITY_MAT;
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

    let skip_special_glow = game_object.name == "Glow"
        && (ctx.root_name.starts_with("GoalArea")
            || ctx.root_name == "BoxChallenge"
            || ctx.root_name == "DynamicBoxChallenge"
            || ctx.root_name.contains("StarBox"));

    if !skip_special_glow
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

    if !skip_special_glow
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
