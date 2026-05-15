use std::collections::HashMap;

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

/// Parsed BG overrides: group/sprite name → partial position override.
#[derive(Default)]
pub struct BgOverrides {
    pub groups: HashMap<String, [Option<f32>; 3]>,
    pub sprites: HashMap<String, [Option<f32>; 3]>,
}
