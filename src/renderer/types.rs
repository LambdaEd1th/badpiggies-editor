//! Public-facing renderer types (camera, drawing context, action results).

use std::collections::BTreeSet;

use eframe::egui;

use crate::data::assets;
use crate::domain::types::*;

/// Camera / viewport state for the level canvas.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Center of the viewport in world coordinates.
    pub center: Vec2,
    /// Pixels per world unit.
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Vec2 { x: 0.0, y: 0.0 },
            zoom: 40.0,
        }
    }
}

impl Camera {
    /// Convert world coordinates to screen coordinates.
    pub fn world_to_screen(&self, world: Vec2, canvas_center: egui::Vec2) -> egui::Pos2 {
        egui::pos2(
            canvas_center.x + (world.x - self.center.x) * self.zoom,
            canvas_center.y - (world.y - self.center.y) * self.zoom, // Y flipped
        )
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, screen: egui::Pos2, canvas_center: egui::Vec2) -> Vec2 {
        Vec2 {
            x: self.center.x + (screen.x - canvas_center.x) / self.zoom,
            y: self.center.y - (screen.y - canvas_center.y) / self.zoom,
        }
    }
}

/// Shared drawing context passed to background/compound drawing functions.
pub(crate) struct DrawCtx<'a> {
    pub painter: &'a egui::Painter,
    pub camera: &'a Camera,
    pub canvas_center: egui::Vec2,
    pub canvas_rect: egui::Rect,
    pub tex_cache: &'a assets::TextureCache,
}

/// World-space transform for a compound object.
#[derive(Clone, Copy)]
pub(crate) struct CompoundTransform {
    pub world_x: f32,
    pub world_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_z: f32,
}


/// Result of a completed terrain node drag.
pub struct NodeDragResult {
    /// Which terrain object.
    pub object_index: ObjectIndex,
    /// Which node within the terrain.
    pub node_index: usize,
    /// New node position in local terrain space.
    pub new_local_pos: Vec2,
}

/// Result of a terrain node add or delete action.
pub enum NodeEditAction {
    /// Delete node at given index.
    Delete {
        object_index: ObjectIndex,
        node_index: usize,
    },
    /// Insert a new node after `after_node`, at `local_pos`.
    Insert {
        object_index: ObjectIndex,
        after_node: usize,
        local_pos: Vec2,
    },
    /// Toggle texture index on a node (grass ↔ outline).
    ToggleTexture {
        object_index: ObjectIndex,
        node_index: usize,
    },
}

/// Active cursor/tool mode for canvas interaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorMode {
    /// Default: click to select, drag objects to move.
    #[default]
    Select,
    /// Drag a rectangle to select all objects inside it.
    BoxSelect,
    /// Freehand draw to create terrain curves.
    DrawTerrain,
    /// All primary-drag pans the view (no object interaction).
    Pan,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewPlaybackState {
    #[default]
    Build,
    Play,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainPresetShape {
    Circle,
    Rectangle,
    PerfectCircle,
    Square,
    EquilateralTriangle,
}

/// Result of a completed box-selection drag.
pub struct BoxSelectResult {
    /// Indices of objects whose world positions fall inside the box.
    pub indices: BTreeSet<ObjectIndex>,
}

/// Result of a completed terrain draw (point-by-point).
pub struct DrawTerrainResult {
    /// World-space points placed by the user.
    pub points: Vec<Vec2>,
    /// Whether the curve was closed (last point snapped to first).
    pub closed: bool,
}

/// Which part of the level bounds rectangle is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundsHandle {
    /// Moving the entire rectangle.
    Move,
    /// Resizing from the left edge.
    Left,
    /// Resizing from the right edge.
    Right,
    /// Resizing from the top edge.
    Top,
    /// Resizing from the bottom edge.
    Bottom,
    /// Resizing from the top-left corner.
    TopLeft,
    /// Resizing from the top-right corner.
    TopRight,
    /// Resizing from the bottom-left corner.
    BottomLeft,
    /// Resizing from the bottom-right corner.
    BottomRight,
}

/// Result of a completed level-bounds drag (consumed by app layer).
pub struct BoundsDragResult {
    pub new_limits: [f32; 4],
}

pub enum CanvasContextAction {
    Copy(Vec<ObjectIndex>),
    Cut(Vec<ObjectIndex>),
    AddObject {
        world_pos: Option<Vec2>,
    },
    Paste {
        context_indices: Vec<ObjectIndex>,
        world_pos: Option<Vec2>,
    },
    Duplicate(Vec<ObjectIndex>),
    Delete(Vec<ObjectIndex>),
}
