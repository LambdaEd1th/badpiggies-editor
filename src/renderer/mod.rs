//! Level renderer — draws terrain, sprites, background on the egui canvas.

pub mod background;
pub mod bg_shader;
mod clouds;
mod compound_data;
mod compound_overrides;
pub mod compounds;
pub mod dark_overlay;
pub mod edge_shader;
pub mod fill_shader;
mod goal_flag;
pub mod grid;
mod input;
mod level_setup;
pub mod opaque_shader;
mod overlays;
mod particles;
pub mod sprite_shader;
pub mod sprites;
pub mod terrain;

use std::collections::BTreeSet;
use std::sync::Arc;

use eframe::egui;

use crate::data::assets;
use crate::domain::types::*;

use clouds::*;
use dark_overlay::DarkOverlayKey;
use dark_overlay::LitAreaPolygon;
use particles::*;

const ROTATION_HANDLE_STEM_PX: f32 = 10.0;
const ROTATION_HANDLE_OFFSET_PX: f32 = 26.0;
const ROTATION_HANDLE_RADIUS_PX: f32 = 7.0;
const SCALE_HANDLE_OFFSET_PX: f32 = 10.0;
const SCALE_HANDLE_RADIUS_PX: f32 = 7.0;
const MIN_OBJECT_SCALE: f32 = 0.05;

/// Known atlas filenames and their paths relative to the sprites directory.
const ATLAS_FILES: &[&str] = &[
    "IngameAtlas.png",
    "IngameAtlas2.png",
    "IngameAtlas3.png",
    "Ingame_Characters_Sheet_01.png",
    "Ingame_Sheet_04.png",
    "Props_Generic_Sheet_01.png",
];

/// Goal flag texture (needs repeat wrap for UV scroll).
pub(super) const GOAL_FLAG_TEXTURE: &str = "Props_Goal_Area_01.png";

/// Glow/starburst atlas.
pub(super) const GLOW_ATLAS: &str = "Particles_Sheet_01.png";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScaleHandleKind {
    Horizontal,
    Vertical,
    Corner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScaleHandleTarget {
    index: ObjectIndex,
    kind: ScaleHandleKind,
}

/// Active object transform drag mode.
#[derive(Clone, Copy)]
enum DragMode {
    Move,
    Rotate {
        start_pointer_angle: f32,
        original_rotation: f32,
    },
    Scale {
        handle: ScaleHandleKind,
        start_pointer_local: Vec2,
        original_scale: Vec2,
        original_half_size: (f32, f32),
        original_rotation: f32,
    },
}

/// State for an active object drag operation.
struct DragState {
    index: ObjectIndex,
    start_mouse: Vec2,
    original_pos: Vec3,
    mode: DragMode,
}

/// State for an active terrain node drag operation.
struct NodeDragState {
    /// Which terrain object.
    object_index: ObjectIndex,
    /// Which node within the terrain.
    node_index: usize,
    /// Mouse position when drag started (world coords).
    start_mouse: Vec2,
    /// Original node position (world coords).
    original_pos: Vec2,
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

/// State for an active level-bounds drag operation.
struct BoundsDragState {
    handle: BoundsHandle,
    start_mouse: Vec2,
    original_limits: [f32; 4],
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

/// Renderer state for drawing levels.
pub struct LevelRenderer {
    pub camera: Camera,
    /// Cached object positions for rendering.
    world_positions: Vec<(ObjectIndex, Vec3)>,
    /// Pre-built terrain draw data (world-space meshes).
    terrain_data: Vec<terrain::TerrainDrawData>,
    /// Pre-built sprite draw data.
    sprite_data: Vec<sprites::SpriteDrawData>,
    /// Detected background theme name.
    bg_theme: Option<&'static str>,
    /// Background override text from BackgroundObject.
    bg_override_text: Option<String>,
    /// Cached background layer grouping/tile data (rebuilt at level load).
    bg_layer_cache: Option<background::BgLayerCache>,
    /// Parsed construction grid.
    construction_grid: Option<grid::ConstructionGrid>,
    /// Whether to show the construction grid overlay.
    pub show_grid_overlay: bool,
    /// Level-refs key derived from filename (for prefab name overrides).
    level_key: String,
    /// Atlas texture cache.
    tex_cache: assets::TextureCache,
    /// Is panning active?
    panning: bool,
    /// Object index clicked this frame (if any).
    pub clicked_object: Option<ObjectIndex>,
    /// Whether the click had Cmd/Ctrl held.
    pub clicked_with_cmd: bool,
    /// Current mouse position in world coordinates (if hovering canvas).
    pub mouse_world: Option<Vec2>,
    /// Elapsed time for animations (seconds).
    pub time: f64,
    /// Active drag state for object manipulation.
    dragging: Option<DragState>,
    /// Active terrain node drag state.
    node_dragging: Option<NodeDragState>,
    /// Completed drag result: (object index, position delta in world units).
    pub drag_result: Option<(ObjectIndex, Vec2)>,
    /// Completed rotation drag result: (object index, z-rotation delta in degrees).
    pub rotation_drag_result: Option<(ObjectIndex, f32)>,
    /// Completed scale drag result: (object index, absolute x/y scale).
    pub scale_drag_result: Option<(ObjectIndex, Vec2)>,
    /// Completed terrain node drag result.
    pub node_drag_result: Option<NodeDragResult>,
    /// Completed terrain node edit action (add/delete).
    pub node_edit_action: Option<NodeEditAction>,
    /// Completed box-selection result.
    pub box_select_result: Option<BoxSelectResult>,
    /// Object-oriented context-menu action requested this frame.
    pub context_action: Option<CanvasContextAction>,
    /// Object that should become selected due to a viewport context-menu click.
    pub context_selected_object: Option<ObjectIndex>,
    /// World position where the current canvas context menu was opened.
    context_menu_world_pos: Option<Vec2>,
    /// Object selection snapshot for the current canvas context menu.
    context_menu_indices: Vec<ObjectIndex>,
    /// Terrain node snapshot for the current canvas context menu.
    context_menu_node: Option<(ObjectIndex, usize)>,
    /// True when a canvas secondary click was consumed by another tool this frame.
    suppress_context_menu_this_frame: bool,
    /// Active box-selection start position (screen coords).
    box_select_start: Option<egui::Pos2>,
    /// Completed freehand terrain draw result.
    pub draw_terrain_result: Option<DrawTerrainResult>,
    /// Currently armed terrain preset shape, if any.
    terrain_preset_shape: Option<TerrainPresetShape>,
    /// World-space drag start for the armed terrain preset shape.
    terrain_preset_drag_start: Option<Vec2>,
    /// Node count used for ellipse and perfect-circle preset generation.
    terrain_round_segments: usize,
    /// Active freehand terrain draw points (world coords).
    draw_terrain_points: Vec<Vec2>,
    /// Whether a freehand draw is currently active.
    draw_terrain_active: bool,
    /// Active bounds drag state.
    bounds_dragging: Option<BoundsDragState>,
    /// Completed bounds drag result (consumed by app layer).
    pub bounds_drag_result: Option<BoundsDragResult>,
    /// Which bounds handle is currently hovered (for cursor icon).
    pub bounds_hovered_handle: Option<BoundsHandle>,
    /// Residual camera offset for dragged sprite, kept until drag_result is consumed
    /// (prevents 1-frame snap-back when opaque batch hasn't been rebuilt yet).
    pending_drag_offset: Option<(ObjectIndex, f32, f32)>,
    /// Whether to show background layers.
    pub show_bg: bool,
    /// Whether to show the physics ground line.
    pub show_ground: bool,
    /// Whether to show the world grid overlay.
    pub show_grid: bool,
    /// Whether the current level is a dark level (m_darkLevel).
    dark_level: bool,
    /// Whether to show the dark overlay (togglable in UI).
    pub show_dark_overlay: bool,
    /// Parsed camera limits from LevelManager (topLeft + size).
    pub camera_limits: Option<[f32; 4]>,
    /// Whether to show the level bounds border.
    pub show_level_bounds: bool,
    /// Whether to show terrain fill triangulation wireframe.
    pub show_terrain_tris: bool,
    /// Pre-computed lit area polygons (world-space vertices for each LitArea).
    lit_area_polygons: Vec<LitAreaPolygon>,
    /// Fan state machines for propeller animation.
    fan_emitters: Vec<FanEmitter>,
    /// Fan wind particles.
    fan_particles: Vec<FanParticle>,
    /// Wind area definitions for leaf particle spawning.
    wind_areas: Vec<WindAreaDef>,
    /// Active wind leaf particles.
    wind_particles: Vec<WindParticle>,
    /// Per-area spawn accumulators.
    wind_spawn_accum: Vec<f32>,
    /// Bird sleeping Zzz particles.
    zzz_particles: Vec<ZzzParticle>,
    /// Per-bird Zzz emit accumulator.
    zzz_emit_accum: Vec<f32>,
    /// Bird positions for Zzz spawning.
    bird_positions: Vec<Vec2>,
    /// Individual cloud sprite instances.
    cloud_instances: Vec<CloudInstance>,
    /// wgpu device for GPU resource creation.
    wgpu_device: Option<eframe::wgpu::Device>,
    /// wgpu queue for texture/buffer uploads.
    wgpu_queue: Option<eframe::wgpu::Queue>,
    /// Shared wgpu edge shader pipeline + resources.
    edge_resources: Option<Arc<edge_shader::EdgeResources>>,
    /// GPU-uploaded terrain edge meshes (one per terrain object).
    edge_gpu_meshes: Arc<Vec<edge_shader::EdgeGpuMesh>>,
    /// Set of terrain_data indices that have GPU-uploaded edge meshes.
    edge_gpu_mesh_index: Vec<Option<usize>>,
    /// Shared wgpu background shader pipeline + resources.
    bg_resources: Option<Arc<bg_shader::BgResources>>,
    /// Cached GPU background atlas textures (loaded lazily per theme).
    bg_atlas_cache: bg_shader::BgAtlasCache,
    /// Per-frame background draw slot counter.
    bg_slot_counter: u32,
    /// Shared wgpu opaque sprite shader pipeline + resources.
    opaque_resources: Option<Arc<opaque_shader::OpaqueResources>>,
    /// GPU Props atlas texture (loaded once, reused across levels).
    opaque_atlas: Option<Arc<opaque_shader::OpaqueAtlas>>,
    /// GPU vertex batch for all Props sprites in current level.
    opaque_batch: Option<Arc<opaque_shader::OpaqueSpriteBatch>>,
    /// Maps sprite_data index → opaque batch sprite index (None = not opaque).
    opaque_sprite_map: Vec<Option<u32>>,
    /// Shared wgpu transparent sprite shader pipeline + resources.
    sprite_resources: Option<Arc<sprite_shader::SpriteResources>>,
    /// Cached GPU sprite atlas textures (loaded lazily per atlas).
    sprite_atlas_cache: sprite_shader::SpriteAtlasCache,
    /// Per-frame sprite shader draw slot counter.
    sprite_slot_counter: u32,
    /// Shared wgpu terrain fill shader pipeline + resources.
    fill_resources: Option<Arc<fill_shader::FillResources>>,
    /// Cached GPU fill textures (loaded lazily per ground texture).
    fill_texture_cache: fill_shader::FillTextureCache,
    /// Pre-built GPU vertex/index buffers for terrain fill meshes.
    fill_gpu_meshes: Vec<Option<Arc<fill_shader::FillGpuMesh>>>,
    /// Per-frame fill shader draw slot counter.
    fill_slot_counter: u32,
    /// Index of the terrain node currently hovered by the mouse: (object, node_index).
    /// Currently hovered terrain node (object_index, node_index).
    pub hovered_terrain_node: Option<(ObjectIndex, usize)>,
    /// Selected sprite whose rotation handle is currently hovered.
    pub hovered_rotation_handle: Option<ObjectIndex>,
    /// Selected sprite scale handle currently hovered.
    hovered_scale_handle: Option<ScaleHandleTarget>,
    /// Reusable scratch mesh buffer for terrain CPU transform (avoids per-frame allocation).
    terrain_scratch_mesh: egui::Mesh,
    /// True when the user clicked the canvas without hitting any object.
    pub clicked_empty: bool,
    /// Cached dark overlay mesh (layer 1: dark complement).
    dark_overlay_mesh: Option<egui::Mesh>,
    /// Cached dark overlay light fill mesh (layer 2: faintly darkened lit area).
    dark_overlay_light: Option<egui::Mesh>,
    /// Cached dark overlay border ring mesh (layer 3).
    dark_overlay_ring: Option<egui::Mesh>,
    /// Camera/viewport state when dark overlay was last built.
    dark_overlay_key: DarkOverlayKey,
    /// Most recent camera/viewport state seen by the dark overlay draw path.
    dark_overlay_live_key: DarkOverlayKey,
    /// Number of consecutive frames the dark overlay viewport key has stayed stable.
    dark_overlay_stable_frames: u8,
}

impl LevelRenderer {
    fn selected_transform_sprite<'a>(
        &'a self,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<&'a sprites::SpriteDrawData> {
        if selected.len() != 1 {
            return None;
        }
        let index = selected.iter().next().copied()?;
        self.sprite_data
            .iter()
            .find(|sprite| sprite.index == index && !sprite.is_terrain)
    }

    fn rotation_handle_positions(
        &self,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) -> (egui::Pos2, egui::Pos2) {
        let center = self.camera.world_to_screen(
            Vec2 {
                x: sprite.world_pos.x,
                y: sprite.world_pos.y,
            },
            canvas_center,
        );
        let half_height = sprite.half_size.1 * self.camera.zoom;
        let cos_r = sprite.rotation.cos();
        let sin_r = sprite.rotation.sin();
        let rotate = |dy: f32| egui::pos2(center.x + dy * sin_r, center.y + dy * cos_r);
        (
            rotate(-(half_height + ROTATION_HANDLE_STEM_PX)),
            rotate(-(half_height + ROTATION_HANDLE_OFFSET_PX)),
        )
    }

    fn rotation_handle_hit(
        &self,
        pointer: egui::Pos2,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let sprite = self.selected_transform_sprite(selected)?;
        let (_, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let dx = pointer.x - handle_center.x;
        let dy = pointer.y - handle_center.y;
        ((dx * dx + dy * dy).sqrt() <= ROTATION_HANDLE_RADIUS_PX + 4.0).then_some(sprite.index)
    }

    fn scale_handle_positions(
        &self,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) -> [(ScaleHandleKind, egui::Pos2, egui::Pos2); 3] {
        let center = self.camera.world_to_screen(
            Vec2 {
                x: sprite.world_pos.x,
                y: sprite.world_pos.y,
            },
            canvas_center,
        );
        let half_width = sprite.half_size.0 * self.camera.zoom;
        let half_height = sprite.half_size.1 * self.camera.zoom;
        let cos_r = sprite.rotation.cos();
        let sin_r = sprite.rotation.sin();
        let rotate = |dx: f32, dy: f32| {
            egui::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };
        [
            (
                ScaleHandleKind::Horizontal,
                rotate(half_width, 0.0),
                rotate(half_width + SCALE_HANDLE_OFFSET_PX, 0.0),
            ),
            (
                ScaleHandleKind::Vertical,
                rotate(0.0, half_height),
                rotate(0.0, half_height + SCALE_HANDLE_OFFSET_PX),
            ),
            (
                ScaleHandleKind::Corner,
                rotate(half_width, half_height),
                rotate(
                    half_width + SCALE_HANDLE_OFFSET_PX,
                    half_height + SCALE_HANDLE_OFFSET_PX,
                ),
            ),
        ]
    }

    fn scale_handle_hit(
        &self,
        pointer: egui::Pos2,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ScaleHandleTarget> {
        let sprite = self.selected_transform_sprite(selected)?;
        self.scale_handle_positions(sprite, canvas_center)
            .into_iter()
            .find_map(|(kind, _, handle_center)| {
                let dx = pointer.x - handle_center.x;
                let dy = pointer.y - handle_center.y;
                ((dx * dx + dy * dy).sqrt() <= SCALE_HANDLE_RADIUS_PX + 4.0).then_some(
                    ScaleHandleTarget {
                        index: sprite.index,
                        kind,
                    },
                )
            })
    }

    fn scale_handle_cursor(handle: ScaleHandleKind) -> egui::CursorIcon {
        match handle {
            ScaleHandleKind::Horizontal => egui::CursorIcon::ResizeHorizontal,
            ScaleHandleKind::Vertical => egui::CursorIcon::ResizeVertical,
            ScaleHandleKind::Corner => egui::CursorIcon::ResizeNwSe,
        }
    }

    fn pointer_local(center: egui::Pos2, pointer: egui::Pos2, rotation: f32) -> Vec2 {
        let rel_x = pointer.x - center.x;
        let rel_y = pointer.y - center.y;
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();
        Vec2 {
            x: rel_x * cos_r - rel_y * sin_r,
            y: rel_x * sin_r + rel_y * cos_r,
        }
    }

    fn pointer_angle(center: egui::Pos2, pointer: egui::Pos2) -> f32 {
        (center.y - pointer.y).atan2(pointer.x - center.x)
    }

    fn normalize_angle_delta(mut angle: f32) -> f32 {
        while angle <= -std::f32::consts::PI {
            angle += std::f32::consts::TAU;
        }
        while angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        angle
    }

    fn draw_rotation_handle(
        &self,
        painter: &egui::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) {
        let (stem_start, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let is_active = self.dragging.as_ref().is_some_and(|drag| {
            drag.index == sprite.index && matches!(drag.mode, DragMode::Rotate { .. })
        });
        let fill = if is_active || self.hovered_rotation_handle == Some(sprite.index) {
            egui::Color32::from_rgb(255, 235, 120)
        } else {
            egui::Color32::WHITE
        };
        painter.line_segment(
            [stem_start, handle_center],
            egui::Stroke::new(2.0, egui::Color32::YELLOW),
        );
        painter.circle_filled(handle_center, ROTATION_HANDLE_RADIUS_PX, fill);
        painter.circle_stroke(
            handle_center,
            ROTATION_HANDLE_RADIUS_PX,
            egui::Stroke::new(2.0, egui::Color32::BLACK),
        );
    }

    fn draw_scale_handle(
        &self,
        painter: &egui::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) {
        let active_handle = self.dragging.as_ref().and_then(|drag| match drag.mode {
            DragMode::Scale { handle, .. } if drag.index == sprite.index => Some(handle),
            _ => None,
        });
        let hovered_handle = self.hovered_scale_handle.and_then(|target| {
            (target.index == sprite.index).then_some(target.kind)
        });
        for (kind, anchor, handle_center) in self.scale_handle_positions(sprite, canvas_center) {
            let fill = if active_handle == Some(kind) || hovered_handle == Some(kind) {
                egui::Color32::from_rgb(140, 230, 255)
            } else {
                egui::Color32::WHITE
            };
            painter.line_segment(
                [anchor, handle_center],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 220, 255)),
            );
            let size = match kind {
                ScaleHandleKind::Horizontal => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 2.8, SCALE_HANDLE_RADIUS_PX * 1.5)
                }
                ScaleHandleKind::Vertical => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 1.5, SCALE_HANDLE_RADIUS_PX * 2.8)
                }
                ScaleHandleKind::Corner => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 2.0, SCALE_HANDLE_RADIUS_PX * 2.0)
                }
            };
            let handle_rect = egui::Rect::from_center_size(handle_center, size);
            painter.rect_filled(handle_rect, 2.0, fill);
            painter.rect_stroke(
                handle_rect,
                2.0,
                egui::Stroke::new(2.0, egui::Color32::BLACK),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Set the level-refs key (derived from filename) for prefab name overrides.
    pub fn set_level_key(&mut self, filename: &str) {
        self.level_key = crate::domain::level::refs::level_key_from_filename(filename);
    }

    /// Whether the current level is a dark level.
    pub fn is_dark_level(&self) -> bool {
        self.dark_level
    }

    /// Shared transparent sprite shader resources, if the current backend has wgpu.
    pub fn preview_sprite_resources(&self) -> Option<Arc<sprite_shader::SpriteResources>> {
        self.sprite_resources.clone()
    }

    /// Load or fetch a GPU sprite atlas for save preview rendering.
    pub fn preview_sprite_atlas(
        &mut self,
        filename: &str,
    ) -> Option<Arc<sprite_shader::SpriteAtlasGpu>> {
        let (Some(resources), Some(device), Some(queue)) = (
            self.sprite_resources.as_ref(),
            self.wgpu_device.as_ref(),
            self.wgpu_queue.as_ref(),
        ) else {
            return None;
        };
        self.sprite_atlas_cache
            .get_or_load(device, queue, resources, filename)
    }

    /// Show the level canvas. Returns the index of a newly-clicked object, if any.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        selected: &BTreeSet<ObjectIndex>,
        cursor_mode: CursorMode,
        tr: &'static crate::i18n::locale::I18n,
        has_clipboard: bool,
    ) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
        let rect = response.rect;
        let canvas_center = rect.center().to_vec2();
        self.context_action = None;
        self.context_selected_object = None;

        // Advance animation time (use stable_dt = measured frame interval, not predicted)
        self.time += ui.input(|i| i.stable_dt as f64);

        // Track mouse world position
        self.mouse_world = response
            .hover_pos()
            .map(|p| self.camera.screen_to_world(p, canvas_center));

        // Background (sky + ground fill + parallax layers + clouds)
        if self.show_bg {
            let dt = ui.input(|i| i.stable_dt);
            self.draw_background_all(&painter, canvas_center, rect, dt);
        }

        // ── Decorative terrain (Z≈5, behind ground background Z≈2) ──
        // In Unity, decorative terrain sits at Z≈5 (farther from camera), so it's
        // occluded by ground background at Z≈2. We draw it before ground layers.
        self.draw_terrain_pass(&painter, canvas_center, rect, true);

        // ── Ground background (eff_z 0..5): beach/grass, behind collider terrain ──
        if self.show_bg {
            self.draw_bg_z_range(&painter, canvas_center, rect, (0.0, 5.0));
        }

        // Construction grid overlay (renderOrder=9, between ground and collider terrain)
        if self.show_grid_overlay
            && let Some(ref cg) = self.construction_grid
        {
            grid::draw_construction_grid(
                &painter,
                cg,
                &self.camera,
                canvas_center,
                rect,
                &mut self.tex_cache,
                ui.ctx(),
            );
        }

        // ── Interaction: drag, pan, click ──
        self.handle_interaction(ui, &response, canvas_center, rect, selected, cursor_mode);

        // ── Collider terrain (Z≈0, in front of ground background) ──

        // Glow starbursts + goal flags (drawn before collider terrain for Z-order)
        self.draw_pre_terrain_effects(&painter, canvas_center, rect);

        // Collider terrain: interleave fill + edge per terrain (back-to-front)
        self.draw_terrain_pass(&painter, canvas_center, rect, false);

        // Terrain triangulation wireframe overlay
        if self.show_terrain_tris {
            self.draw_terrain_wireframe(&painter, canvas_center);
        }

        // Selection outlines and node handles for terrain
        self.draw_terrain_selection(&painter, canvas_center, selected);

        // ── Particle simulation (fan, wind, zzz) ──
        let dt = ui.input(|i| i.stable_dt);
        self.update_particles(dt);

        // Draw Zzz particles BEFORE sprites — in Unity emitter is at z=+0.5 (behind bird body)
        particles::draw_zzz_particles(
            &self.zzz_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache.get(GLOW_ATLAS),
        );

        // Sprites with goal bobbing + compound sub-sprites (renderOrder=12)
        self.draw_sprites(&painter, canvas_center, rect, selected);

        // Draw fan particles (cloud puffs, renderOrder=12)
        particles::draw_fan_particles(
            &self.fan_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache.get(GLOW_ATLAS),
        );

        // ── Dark level overlay with LitArea cutouts ──
        if self.dark_level && self.show_dark_overlay {
            self.draw_dark_overlay(&painter, canvas_center, rect);
        }

        // ── Front-ground + foreground (eff_z < 0): waves/foam/dummy + foreground, after sprites ──
        if self.show_bg {
            self.draw_bg_z_range(&painter, canvas_center, rect, (f32::NEG_INFINITY, 0.0));
        }

        // Draw wind leaf particles (renderOrder=25, on top of foreground)
        particles::draw_wind_particles(
            &self.wind_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache.get(GLOW_ATLAS),
        );

        // Grid (drawn on top of all scene content)
        if self.show_grid {
            self.draw_grid(&painter, rect, canvas_center);
        }

        // HUD overlays: origin axes, physics ground, level bounds, zoom info
        self.draw_hud(&painter, rect, canvas_center, tr);

        // Tool mode overlays (box-select rect, terrain draw preview)
        self.draw_tool_overlay(&painter, canvas_center, cursor_mode);

        // Set cursor icon for bounds handles
        if let Some(handle) = self.bounds_hovered_handle {
            let icon = match handle {
                BoundsHandle::Move => egui::CursorIcon::Grab,
                BoundsHandle::Left | BoundsHandle::Right => egui::CursorIcon::ResizeHorizontal,
                BoundsHandle::Top | BoundsHandle::Bottom => egui::CursorIcon::ResizeVertical,
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if let Some(bounds_dragging) = self.bounds_dragging.as_ref() {
            let icon = match bounds_dragging.handle {
                BoundsHandle::Move => egui::CursorIcon::Grabbing,
                BoundsHandle::Left | BoundsHandle::Right => egui::CursorIcon::ResizeHorizontal,
                BoundsHandle::Top | BoundsHandle::Bottom => egui::CursorIcon::ResizeVertical,
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            };
            ui.ctx().set_cursor_icon(icon);
        } else if self
            .dragging
            .as_ref()
            .and_then(|drag| match drag.mode {
                DragMode::Scale { handle, .. } => Some(handle),
                _ => None,
            })
            .map(|handle| {
                ui.ctx().set_cursor_icon(Self::scale_handle_cursor(handle));
            })
            .is_some()
        {
        } else if self
            .dragging
            .as_ref()
            .is_some_and(|drag| matches!(drag.mode, DragMode::Rotate { .. }))
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let Some(target) = self.hovered_scale_handle {
            ui.ctx().set_cursor_icon(Self::scale_handle_cursor(target.kind));
        } else if self.hovered_rotation_handle.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        // Request continuous repaint for animations
        ui.ctx().request_repaint();

        // Lazy-load atlas textures (only attempt once per atlas)
        self.lazy_load_textures(ui.ctx());

        let hovered_node = self.hovered_terrain_node;
        let suppress_context_menu = self.suppress_context_menu_this_frame
            || (cursor_mode == CursorMode::DrawTerrain && !self.draw_terrain_points.is_empty());
        let terrain_node_can_delete = |node: Option<(ObjectIndex, usize)>| {
            node.and_then(|(object_index, node_index)| {
                self.terrain_data
                    .iter()
                    .find(|terrain| terrain.object_index == object_index)
                    .map(|terrain| {
                        (
                            object_index,
                            node_index,
                            terrain.curve_world_verts.len() > 2,
                        )
                    })
            })
        };
        let context_menu_node_can_delete = terrain_node_can_delete(self.context_menu_node);
        if !suppress_context_menu && response.secondary_clicked() {
            let hovered_node_can_delete = terrain_node_can_delete(hovered_node);
            let context_world = response
                .interact_pointer_pos()
                .map(|pointer| self.camera.screen_to_world(pointer, canvas_center));
            let context_object = context_world
                .and_then(|world| self.hit_test(world, selected))
                .or_else(|| hovered_node.map(|(object_index, _)| object_index));
            self.context_menu_world_pos = context_world;
            self.context_menu_indices = context_object
                .map(|index| {
                    if selected.contains(&index) {
                        selected.iter().copied().collect()
                    } else {
                        vec![index]
                    }
                })
                .unwrap_or_else(|| selected.iter().copied().collect());
            self.context_menu_node = hovered_node_can_delete
                .map(|(object_index, node_index, _)| (object_index, node_index));
            if let Some(index) = context_object
                && !selected.contains(&index)
            {
                self.context_selected_object = Some(index);
            }
        }
        let context_world = self.context_menu_world_pos;
        let context_indices = self.context_menu_indices.clone();
        let has_context_selection = !context_indices.is_empty();
        let is_mac = cfg!(target_os = "macos");
        let copy_shortcut = if is_mac { "Cmd+C" } else { "Ctrl+C" };
        let cut_shortcut = if is_mac { "Cmd+X" } else { "Ctrl+X" };
        let paste_shortcut = if is_mac { "Cmd+V" } else { "Ctrl+V" };
        let dup_shortcut = if is_mac { "Cmd+D" } else { "Ctrl+D" };
        if !suppress_context_menu {
            response.context_menu(|ui| {
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_copy")).shortcut_text(copy_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Copy(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_cut")).shortcut_text(cut_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Cut(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_clipboard,
                        egui::Button::new(tr.get("menu_paste")).shortcut_text(paste_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Paste {
                        context_indices: context_indices.clone(),
                        world_pos: context_world,
                    });
                    ui.close();
                }
                ui.separator();
                if ui.button(tr.get("menu_add_object")).clicked() {
                    self.context_action = Some(CanvasContextAction::AddObject {
                        world_pos: context_world,
                    });
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_duplicate")).shortcut_text(dup_shortcut),
                    )
                    .clicked()
                {
                    self.context_action =
                        Some(CanvasContextAction::Duplicate(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_delete")).shortcut_text("Del"),
                    )
                    .clicked()
                {
                    self.context_action =
                        Some(CanvasContextAction::Delete(context_indices.clone()));
                    ui.close();
                }

                if let Some((object_index, node_index, can_delete)) = context_menu_node_can_delete {
                    ui.separator();
                    if ui.button(tr.get("context_toggle_node_texture")).clicked() {
                        self.node_edit_action = Some(NodeEditAction::ToggleTexture {
                            object_index,
                            node_index,
                        });
                        ui.close();
                    }
                    if ui
                        .add_enabled(can_delete, egui::Button::new(tr.get("menu_delete")))
                        .clicked()
                    {
                        self.node_edit_action = Some(NodeEditAction::Delete {
                            object_index,
                            node_index,
                        });
                        ui.close();
                    }
                }

                ui.separator();
                if ui.button(tr.get("menu_fit_view")).clicked() {
                    self.fit_to_level();
                    ui.close();
                }
            });
        }
    }
}
