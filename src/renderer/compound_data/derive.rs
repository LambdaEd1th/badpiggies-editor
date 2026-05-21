//! Mechanical derivation of compound SubSprite data from embedded Unity prefab
//! assets. Used by every `LazyLock<SubSprite>` in the parent module so the rendered
//! constants are tied directly to the embedded prefab YAML + sprites.bytes.
//!
//! These helpers panic on failure — they run once per static at first access, so
//! a missing or malformed prefab is a programmer / asset-pack error that should
//! surface immediately rather than silently fall back to defaults.

use super::{SCALE, SubSprite};
use crate::data::assets::{atlas_for_material_guid, read_pathname_text};
use crate::data::sprite_db::{UvRect, runtime_sprite_dimensions, runtime_sprite_pivot};
use crate::domain::prefab_asset::{PrefabAssetComponent, PrefabAssetDocument};

/// Atlas-pixel pivot → world units. The atlas reference is 768 px high in Unity's
/// tk2dSpriteCollection, so pivot in atlas pixels divided by 768 yields world units.
pub(super) const PIVOT_WORLD: f32 = 1.0 / 768.0;

pub(super) fn load_prefab(name: &str) -> PrefabAssetDocument {
    let path = format!("Assets/Prefab/{name}.prefab");
    let text =
        read_pathname_text(&path).unwrap_or_else(|| panic!("missing embedded prefab {path}"));
    PrefabAssetDocument::parse(&text)
        .unwrap_or_else(|| panic!("failed to parse prefab {name}"))
}

struct SpriteBits {
    atlas: &'static str,
    uv: UvRect,
    pixel_w: f32,
    pixel_h: f32,
    scale_x: f32,
    scale_y: f32,
}

fn bits_from(
    sprite: &PrefabAssetComponent,
    renderer: &PrefabAssetComponent,
    label: &str,
) -> SpriteBits {
    let sprite_id = sprite
        .field_str("m_id")
        .unwrap_or_else(|| panic!("missing m_id on {label}"))
        .to_string();
    let scale_x = sprite.field_f32("m_scaleX").unwrap_or(1.0);
    let scale_y = sprite.field_f32("m_scaleY").unwrap_or(1.0);

    let (uv, pixel_w, pixel_h, _) = runtime_sprite_dimensions(&sprite_id)
        .unwrap_or_else(|| panic!("runtime sprite {sprite_id} missing in sprite db ({label})"));

    let material_guid = renderer
        .field_guid("m_Materials")
        .unwrap_or_else(|| panic!("no m_Materials guid on {label}"));
    let atlas = atlas_for_material_guid(material_guid)
        .unwrap_or_else(|| panic!("no atlas mapping for material {material_guid} ({label})"));

    SpriteBits {
        atlas,
        uv,
        pixel_w,
        pixel_h,
        scale_x,
        scale_y,
    }
}

fn bits_by_name(doc: &PrefabAssetDocument, name: &str) -> SpriteBits {
    let sprite = doc
        .component_by_game_object_name(name, "Sprite")
        .unwrap_or_else(|| panic!("missing Sprite component for {name}"));
    let renderer = doc
        .component_by_game_object_name(name, "MeshRenderer")
        .unwrap_or_else(|| panic!("missing MeshRenderer for {name}"));
    bits_from(sprite, renderer, name)
}

fn bits_by_path(doc: &PrefabAssetDocument, path: &str) -> SpriteBits {
    let sprite = doc
        .component_by_path(path, "Sprite")
        .unwrap_or_else(|| panic!("missing Sprite component at {path}"));
    let renderer = doc
        .component_by_path(path, "MeshRenderer")
        .unwrap_or_else(|| panic!("missing MeshRenderer at {path}"));
    bits_from(sprite, renderer, path)
}

/// Sprite pivot (atlas pixels → world units) for the sprite at `path`.
pub(super) fn pivot_world_by_path(doc: &PrefabAssetDocument, path: &str) -> (f32, f32) {
    let sprite_id = doc
        .component_by_path(path, "Sprite")
        .and_then(|c| c.field_str("m_id").map(|s| s.to_string()))
        .unwrap_or_else(|| panic!("no Sprite m_id at {path}"));
    let (px, py) = runtime_sprite_pivot(&sprite_id)
        .unwrap_or_else(|| panic!("no pivot for sprite {sprite_id} (path {path})"));
    (px * PIVOT_WORLD, py * PIVOT_WORLD)
}

/// Derive a SubSprite from a prefab game-object identified by its bare name.
/// Offset is the cumulative local position from the leaf up to (but not including)
/// the prefab root.
pub(super) fn derive_by_name(doc: &PrefabAssetDocument, game_object_name: &str) -> SubSprite {
    let bits = bits_by_name(doc, game_object_name);
    let cum = doc
        .cumulative_local_pos_by_game_object_name(game_object_name)
        .unwrap_or_else(|| panic!("no cumulative pos for {game_object_name}"));
    SubSprite {
        atlas: bits.atlas,
        uv: bits.uv,
        world_w: bits.pixel_w * bits.scale_x * SCALE,
        world_h: bits.pixel_h * bits.scale_y * SCALE,
        offset_x: cum[0],
        offset_y: cum[1],
        flip_x: false,
        flip_y: false,
    }
}

/// Same as `derive_by_name` but for slash-separated paths.
pub(super) fn derive_by_path(doc: &PrefabAssetDocument, path: &str) -> SubSprite {
    let bits = bits_by_path(doc, path);
    let cum = doc
        .cumulative_local_pos_by_path(path)
        .unwrap_or_else(|| panic!("no cumulative pos at {path}"));
    SubSprite {
        atlas: bits.atlas,
        uv: bits.uv,
        world_w: bits.pixel_w * bits.scale_x * SCALE,
        world_h: bits.pixel_h * bits.scale_y * SCALE,
        offset_x: cum[0],
        offset_y: cum[1],
        flip_x: false,
        flip_y: false,
    }
}

/// Derive a SubSprite for a sprite mounted on the prefab root.
pub(super) fn derive_root(doc: &PrefabAssetDocument, root_name: &str) -> SubSprite {
    let bits = bits_by_name(doc, root_name);
    SubSprite {
        atlas: bits.atlas,
        uv: bits.uv,
        world_w: bits.pixel_w * bits.scale_x * SCALE,
        world_h: bits.pixel_h * bits.scale_y * SCALE,
        offset_x: 0.0,
        offset_y: 0.0,
        flip_x: false,
        flip_y: false,
    }
}

/// Derive UV + scaled world dimensions for a nested sprite whose parent transforms
/// carry non-identity scale (e.g. `Balloons` at localScale=0.875). Offset is left
/// at zero — caller handles positioning separately.
pub(super) fn derive_with_cumulative_scale(doc: &PrefabAssetDocument, path: &str) -> SubSprite {
    let bits = bits_by_path(doc, path);
    let cum_scale = doc
        .cumulative_local_scale_by_path(path)
        .unwrap_or_else(|| panic!("no cumulative scale at {path}"));
    SubSprite {
        atlas: bits.atlas,
        uv: bits.uv,
        world_w: bits.pixel_w * bits.scale_x * cum_scale[0] * SCALE,
        world_h: bits.pixel_h * bits.scale_y * cum_scale[1] * SCALE,
        offset_x: 0.0,
        offset_y: 0.0,
        flip_x: false,
        flip_y: false,
    }
}

pub(super) fn distance_xy_by_path(doc: &PrefabAssetDocument, path: &str) -> f32 {
    let [x, y, _] = doc
        .cumulative_local_pos_by_path(path)
        .unwrap_or_else(|| panic!("no cumulative pos at {path}"));
    (x * x + y * y).sqrt()
}

pub(super) fn rope_anchor_points_by_path(
    doc: &PrefabAssetDocument,
    path: &str,
) -> ((f32, f32), (f32, f32)) {
    let rope = doc
        .component_by_path(path, "RopeVisualization")
        .unwrap_or_else(|| panic!("missing RopeVisualization at {path}"));
    let rope_start = rope
        .field_vec3("m_pos1Anchor")
        .unwrap_or_else(|| panic!("missing RopeVisualization m_pos1Anchor at {path}"));
    let rope_end = rope
        .field_vec3("m_pos2Anchor")
        .unwrap_or_else(|| panic!("missing RopeVisualization m_pos2Anchor at {path}"));
    let rope_end_transform_id = rope
        .field_file_id("m_pos2Transform")
        .unwrap_or_else(|| panic!("missing RopeVisualization m_pos2Transform at {path}"));
    let [start_x, start_y, _] = doc
        .transform_point_by_path(path, rope_start)
        .unwrap_or_else(|| panic!("failed to TransformPoint m_pos1Anchor at {path}"));
    let [end_x, end_y, _] = doc
        .transform_point_by_file_id(&rope_end_transform_id, rope_end)
        .unwrap_or_else(|| panic!("failed to TransformPoint m_pos2Anchor at {path}"));
    ((start_x, start_y), (end_x, end_y))
}
