//! Level renderer — draws terrain, sprites, background on the egui canvas.

pub mod background;
pub mod bg_shader;
mod clouds;
mod compound_data;
mod compound_overrides;
pub mod compounds;
pub mod dark_overlay;
pub mod dark_shader;
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

use crate::assets;
use crate::types::*;

use clouds::*;
use dark_overlay::LitAreaPolygon;
use particles::*;

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

/// State for an active object drag operation.
struct DragState {
    index: ObjectIndex,
    start_mouse: Vec2,
    original_pos: Vec3,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorMode {
    /// Default: click to select, drag objects to move.
    Select,
    /// Drag a rectangle to select all objects inside it.
    BoxSelect,
    /// Freehand draw to create terrain curves.
    DrawTerrain,
    /// All primary-drag pans the view (no object interaction).
    Pan,
}

impl Default for CursorMode {
    fn default() -> Self {
        Self::Select
    }
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
    /// Asset base directory (for loading textures).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub asset_base: Option<String>,
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
    /// Completed terrain node drag result.
    pub node_drag_result: Option<NodeDragResult>,
    /// Completed terrain node edit action (add/delete).
    pub node_edit_action: Option<NodeEditAction>,
    /// Completed box-selection result.
    pub box_select_result: Option<BoxSelectResult>,
    /// Active box-selection start position (screen coords).
    box_select_start: Option<egui::Pos2>,
    /// Completed freehand terrain draw result.
    pub draw_terrain_result: Option<DrawTerrainResult>,
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
    /// Shared wgpu dark overlay shader pipeline + resources.
    dark_resources: Option<Arc<dark_shader::DarkResources>>,
    /// Pre-built GPU meshes for dark overlay (fan-triangulated lit-area polygons).
    dark_gpu_meshes: Option<Arc<dark_shader::DarkGpuMeshes>>,
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
    /// Reusable scratch mesh buffer for terrain CPU transform (avoids per-frame allocation).
    terrain_scratch_mesh: egui::Mesh,
    /// Cached dark overlay mesh (layer 1: dark complement).
    dark_overlay_mesh: Option<egui::Mesh>,
    /// Cached dark overlay border ring mesh (layer 2).
    dark_overlay_ring: Option<egui::Mesh>,
    /// Camera/viewport state when dark overlay was last built.
    dark_overlay_key: (f32, f32, f32, f32, f32),
}

impl LevelRenderer {
    /// Set the level-refs key (derived from filename) for prefab name overrides.
    pub fn set_level_key(&mut self, filename: &str) {
        self.level_key = crate::level_refs::level_key_from_filename(filename);
    }

    /// Whether the current level is a dark level.
    pub fn is_dark_level(&self) -> bool {
        self.dark_level
    }

    /// Show the level canvas. Returns the index of a newly-clicked object, if any.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        selected: &BTreeSet<ObjectIndex>,
        cursor_mode: CursorMode,
        tr: &'static crate::locale::I18n,
    ) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
        let rect = response.rect;
        let canvas_center = rect.center().to_vec2();

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
        if self.bounds_dragging.is_some() {
            let icon = match self.bounds_dragging.as_ref().unwrap().handle {
                BoundsHandle::Move => egui::CursorIcon::Grabbing,
                BoundsHandle::Left | BoundsHandle::Right => egui::CursorIcon::ResizeHorizontal,
                BoundsHandle::Top | BoundsHandle::Bottom => egui::CursorIcon::ResizeVertical,
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            };
            ui.ctx().set_cursor_icon(icon);
        }

        // Request continuous repaint for animations
        ui.ctx().request_repaint();

        // Lazy-load atlas textures (only attempt once per atlas)
        self.lazy_load_textures(ui.ctx());
    }
}
