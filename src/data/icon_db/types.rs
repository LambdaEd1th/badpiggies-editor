//! Types shared between icon_db submodules.

use std::collections::HashMap;

pub type Mat2x3 = (f32, f32, f32, f32, f32, f32);

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

/// Per-part info: z_offset + layers.
pub struct PartInfo {
    pub z_offset: f32,
    pub layers: Vec<IconLayer>,
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
    pub(super) uv_x_norm: f32,
    pub(super) uv_y_norm: f32,
    pub(super) uv_w_norm: f32,
    pub(super) uv_h_norm: f32,
}

#[derive(Debug, Clone)]
pub(super) struct GameObjectInfo {
    pub(super) name: String,
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

#[derive(Default)]
pub(super) struct ParsedPrefab {
    pub(super) part_type: Option<i32>,
    pub(super) custom_part_index: Option<i32>,
    pub(super) z_offset: f32,
    pub(super) game_objects: HashMap<String, GameObjectInfo>,
    pub(super) transforms: HashMap<String, TransformInfo>,
    pub(super) sprites: HashMap<String, SpriteComponent>,
    pub(super) renderers: HashMap<String, RendererInfo>,
}
