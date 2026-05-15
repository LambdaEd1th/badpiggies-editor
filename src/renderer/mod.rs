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

use std::sync::Arc;

use eframe::egui;

use crate::data::assets;
use crate::domain::types::*;

use clouds::*;
use dark_overlay::DarkOverlayKey;
use dark_overlay::LitAreaPolygon;
use particles::*;

mod handles;
mod preview;
mod show;
mod types;

pub use types::*;
pub(crate) use handles::MIN_OBJECT_SCALE;
pub(crate) use show::ATLAS_FILES;

/// Goal flag texture (needs repeat wrap for UV scroll).
pub(super) const GOAL_FLAG_TEXTURE: &str = "Props_Goal_Area_01.png";

/// Glow/starburst atlas.
pub(super) const GLOW_ATLAS: &str = "Particles_Sheet_01.png";


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

/// State for an active level-bounds drag operation.
struct BoundsDragState {
    handle: BoundsHandle,
    start_mouse: Vec2,
    original_limits: [f32; 4],
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
    /// Runtime preview state for play/build/pause-sensitive effects.
    preview_playback_state: PreviewPlaybackState,
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
    /// Residual rotation/scale preview kept until the level is rebuilt.
    /// This prevents a 1-frame snap-back to the stale opaque batch on release.
    pending_transform_preview: Option<ObjectIndex>,
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
    /// Attached particle emitters for rocket, turbo, and magnet effects.
    attached_effect_emitters: Vec<particles::AttachedEffectEmitter>,
    /// Active attached-effect particles.
    attached_effect_particles: Vec<particles::AttachedEffectParticle>,
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

