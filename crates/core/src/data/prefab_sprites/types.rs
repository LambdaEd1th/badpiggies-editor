//! Data types for the Unity prefab multi-sprite database.

use std::collections::HashMap;

use crate::data::sprite_db::UvRect;
use crate::domain::types::Vec2;

#[derive(Debug, Clone)]
pub struct PrefabSpriteLayer {
    pub atlas: String,
    pub uv: UvRect,
    pub z_local: f32,
    /// Vertex order matches Unity mesh creation: BL, TL, TR, BR.
    pub vertices: [Vec2; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrefabLocalBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSpriteMeta {
    pub(super) selection_x: i32,
    pub(super) selection_y: i32,
    pub(super) selection_w: i32,
    pub(super) selection_h: i32,
    pub(super) pivot_x: i32,
    pub(super) pivot_y: i32,
    pub(super) uv_x: i32,
    pub(super) uv_y: i32,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) uv: UvRect,
}

#[derive(Debug, Clone)]
pub(super) struct GameObjectInfo {
    pub(super) name: String,
    pub(super) active: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TransformInfo {
    pub(super) game_object_id: String,
    pub(super) pos_x: f32,
    pub(super) pos_y: f32,
    pub(super) pos_z: f32,
    pub(super) scale_x: f32,
    pub(super) scale_y: f32,
    pub(super) qx: f32,
    pub(super) qy: f32,
    pub(super) qz: f32,
    pub(super) qw: f32,
    pub(super) father: String,
    pub(super) children: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct SpriteComponent {
    pub(super) game_object_id: String,
    pub(super) sprite_id: String,
    pub(super) scale_x: f32,
    pub(super) scale_y: f32,
    pub(super) pivot_x: f32,
    pub(super) pivot_y: f32,
}

#[derive(Debug, Clone)]
pub(super) struct RendererInfo {
    pub(super) game_object_id: String,
    pub(super) material_guid: String,
    pub(super) enabled: bool,
}

#[derive(Debug, Clone)]
pub(super) struct UnmanagedSpriteComponent {
    pub(super) uv: UvRect,
    pub(super) world_w: f32,
    pub(super) world_h: f32,
}

#[derive(Default)]
pub(super) struct ParsedPrefab {
    pub(super) game_objects: HashMap<String, GameObjectInfo>,
    pub(super) transforms: HashMap<String, TransformInfo>,
    pub(super) sprites: HashMap<String, SpriteComponent>,
    pub(super) renderers: HashMap<String, RendererInfo>,
    pub(super) unmanaged_sprites: HashMap<String, UnmanagedSpriteComponent>,
}
