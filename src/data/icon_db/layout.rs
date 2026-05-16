//! Bake a parsed part prefab into world-space `IconLayer`s via a hierarchy walk.

use std::collections::HashMap;

use super::math::{mat_apply, mat_compose, make_local_trs, round_six};
use super::parse::atlas_for_material_guid;
use super::types::{
    IconLayer, Mat2x3, ParsedPrefab, RendererInfo, RuntimeSpriteMeta, SpriteComponent,
};

const WORLD_SCALE: f32 = 10.0 / 768.0;

pub(super) fn build_part_layers(
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
