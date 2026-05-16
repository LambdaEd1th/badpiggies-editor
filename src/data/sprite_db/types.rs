//! Types shared between sprite_db submodules.

use std::collections::HashMap;

/// Resolved sprite info ready for rendering.
#[derive(Debug, Clone)]
pub struct SpriteInfo {
    /// Atlas filename (e.g. "IngameAtlas.png").
    pub atlas: String,
    /// Normalized UV rect [0..1].
    pub uv: UvRect,
    /// Half-extent in world units.
    pub world_w: f32,
    pub world_h: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub(super) const UNMANAGED_ATLAS: &str = "Props_Generic_Sheet_01.png";

#[derive(Debug, Clone)]
pub(super) struct RuntimeSpriteMeta {
    pub(super) material_id: String,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) uv: UvRect,
}

#[derive(Debug, Clone)]
pub(super) struct GameObjectInfo {
    pub(super) active: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TransformInfo {
    pub(super) game_object_id: String,
    pub(super) father: String,
    pub(super) children: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RendererInfo {
    pub(super) material_guid: String,
    pub(super) enabled: bool,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSpriteComponent {
    pub(super) game_object_id: String,
    pub(super) sprite_id: String,
    pub(super) scale_x: f32,
    pub(super) scale_y: f32,
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
    pub(super) renderers: HashMap<String, RendererInfo>,
    pub(super) runtime_sprites: Vec<RuntimeSpriteComponent>,
    pub(super) unmanaged_sprites: HashMap<String, UnmanagedSpriteComponent>,
}
