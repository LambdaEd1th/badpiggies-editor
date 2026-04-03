//! Level renderer — draws terrain, sprites, background on the egui canvas.

pub mod background;
pub mod bg_shader;
pub mod compounds;
pub mod edge_shader;
pub mod fill_shader;
pub mod grid;
pub mod opaque_shader;
pub mod sprite_shader;
pub mod sprites;
pub mod terrain;

use std::sync::Arc;

use eframe::egui;

use crate::assets;
use crate::types::*;

/// Point-in-triangle test using barycentric coordinates (sign of cross products).
fn point_in_triangle(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> bool {
    let d1 = (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d2 = (p.x - c.x) * (b.y - c.y) - (b.x - c.x) * (p.y - c.y);
    let d3 = (p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

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
const GOAL_FLAG_TEXTURE: &str = "Props_Goal_Area_01.png";

/// Glow/starburst atlas.
const GLOW_ATLAS: &str = "Particles_Sheet_01.png";

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
    /// Current mouse position in world coordinates (if hovering canvas).
    pub mouse_world: Option<Vec2>,
    /// Elapsed time for animations (seconds).
    pub time: f64,
    /// Active drag state for object manipulation.
    dragging: Option<DragState>,
    /// Completed drag result: (object index, position delta in world units).
    pub drag_result: Option<(ObjectIndex, Vec2)>,
    /// Residual camera offset for dragged sprite, kept until drag_result is consumed
    /// (prevents 1-frame snap-back when opaque batch hasn't been rebuilt yet).
    pending_drag_offset: Option<(ObjectIndex, f32, f32)>,
    /// Whether to show background layers.
    pub show_bg: bool,
    /// Whether to show the physics ground line.
    pub show_ground: bool,
    /// Whether to show the world grid overlay.
    pub show_grid: bool,
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
    /// Reusable scratch mesh buffer for terrain CPU transform (avoids per-frame allocation).
    terrain_scratch_mesh: egui::Mesh,
}

/// An individual cloud sprite that drifts horizontally and wraps.
struct CloudInstance {
    x: f32,
    y: f32,
    center_x: f32,
    limits: f32,
    velocity: f32,
    opacity: f32,
    /// Sprite name for UV lookup.
    sprite_name: String,
    /// Atlas path for texture.
    atlas: String,
    /// Scale multipliers (from config).
    scale_x: f32,
    scale_y: f32,
}

/// Cloud config per set (mirrors Unity CloudSetPlateau etc.)
struct CloudConfig {
    max_clouds: usize,
    velocity: f32,
    limits: f32,
    height: f32,
    sprites: &'static [CloudSpriteInfo],
}
struct CloudSpriteInfo {
    name: &'static str,
    atlas: &'static str,
    scale_x: f32,
    scale_y: f32,
}

const CLOUD_CONFIGS: &[(&str, CloudConfig)] = &[
    (
        "CloudPlateauSet",
        CloudConfig {
            max_clouds: 8,
            velocity: 0.2,
            limits: 93.24,
            height: 5.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
            ],
        },
    ),
    (
        "CloudJungleSet",
        CloudConfig {
            max_clouds: 8,
            velocity: 0.2,
            limits: 93.24,
            height: 10.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 0.87,
                    scale_y: 0.6525,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 1.0875,
                    scale_y: 0.87,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 1.0,
                    scale_y: 0.8,
                },
            ],
        },
    ),
    (
        "CloudNightSet",
        CloudConfig {
            max_clouds: 5,
            velocity: 0.2,
            limits: 40.0,
            height: 2.5,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
            ],
        },
    ),
    (
        "CloudHalloweenSet",
        CloudConfig {
            max_clouds: 5,
            velocity: 0.5,
            limits: 93.0,
            height: 4.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_2",
                    atlas: "Background_Halloween_Sheet_01.png",
                    scale_x: 1.5,
                    scale_y: 1.5,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_2",
                    atlas: "Background_Halloween_Sheet_01.png",
                    scale_x: 1.2,
                    scale_y: 1.2,
                },
            ],
        },
    ),
    (
        "CloudLPASet",
        CloudConfig {
            max_clouds: 10,
            velocity: 0.1,
            limits: 250.0,
            height: 0.84,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_3",
                    atlas: "Background_Maya_Sheet_02.png",
                    scale_x: 1.0,
                    scale_y: 1.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_2",
                    atlas: "Background_Maya_Sheet_02.png",
                    scale_x: 1.0,
                    scale_y: 1.0,
                },
            ],
        },
    ),
];

/// A single Zzz particle.
struct ZzzParticle {
    x: f32,
    y: f32,
    vy: f32,
    age: f32,
    lifetime: f32,
    start_size: f32,
    wobble_phase: f32,
    wobble_freq: f32,
    rot: f32,
    rot_speed: f32,
}

/// Wind area zone definition.
struct WindAreaDef {
    center_x: f32,
    center_y: f32,
    half_w: f32,
    half_h: f32,
}

/// Leaf UV frame within 16×16 Particles_Sheet_01 atlas.
/// Row 2 from top → UV Y = 13/16, columns 4/5/6.
const LEAF_TILES: f32 = 16.0;
const LEAF_ROW_UV: f32 = 13.0 / 16.0; // (16 - 2 - 1) / 16
const LEAF_COLS: [u8; 3] = [4, 5, 6];

/// A single wind leaf particle.
struct WindParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    lifetime: f32,
    rot: f32,
    rot_speed: f32,
    y_phase: f32,
    size: f32,
    /// Which leaf column (0..3 index into LEAF_COLS) for UV frame selection.
    leaf_col: u8,
}

/// Fan state machine (mirrors Fan.cs Update).
#[derive(Clone, Copy, PartialEq)]
enum FanState {
    Inactive,
    DelayedStart,
    SpinUp,
    Spinning,
    SpinDown,
}

/// Persistent fan animation state.
struct FanEmitter {
    /// Index into sprite_data for propeller scaling.
    sprite_index: usize,
    /// Current state.
    state: FanState,
    /// Time counter in current state.
    counter: f32,
    /// Normalized force (0..1).
    force: f32,
    /// Whether particle emission is on.
    emitting: bool,
    /// Propeller rotation angle (rad).
    angle: f32,
    /// Fan world position.
    world_x: f32,
    world_y: f32,
    /// Fan rotation in radians.
    rot: f32,
    /// Burst emission timer (0..1) cycling at 1 Hz.
    burst_time: f32,
    // Timing config from override or defaults
    start_time: f32,
    on_time: f32,
    off_time: f32,
    delayed_start: f32,
    always_on: bool,
}

/// Fan wind particle.
struct FanParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    lifetime: f32,
    start_size: f32,
    rot: f32,
    rot_speed: f32,
}

impl LevelRenderer {
    pub fn new(render_state: Option<&egui_wgpu::RenderState>) -> Self {
        // Initialize wgpu edge shader pipeline if render state available
        let (wgpu_device, wgpu_queue, edge_resources) = match render_state {
            Some(rs) => {
                log::info!(
                    "wgpu render state available, target_format={:?}",
                    rs.target_format
                );
                let resources = edge_shader::init_edge_resources(&rs.device, rs.target_format);
                (
                    Some(rs.device.clone()),
                    Some(rs.queue.clone()),
                    Some(Arc::new(resources)),
                )
            }
            None => {
                log::warn!("No wgpu render state — edge shader disabled");
                (None, None, None)
            }
        };
        let opaque_resources = render_state.map(|rs| {
            Arc::new(opaque_shader::init_opaque_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        let bg_resources = render_state
            .map(|rs| Arc::new(bg_shader::init_bg_resources(&rs.device, rs.target_format)));
        let sprite_resources = render_state.map(|rs| {
            Arc::new(sprite_shader::init_sprite_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        let fill_resources = render_state.map(|rs| {
            Arc::new(fill_shader::init_fill_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        Self {
            camera: Camera::default(),
            world_positions: Vec::new(),
            terrain_data: Vec::new(),
            sprite_data: Vec::new(),
            bg_theme: None,
            bg_override_text: None,
            bg_layer_cache: None,
            construction_grid: None,
            show_grid_overlay: true,
            level_key: String::new(),
            tex_cache: assets::TextureCache::new(),
            asset_base: None,
            panning: false,
            clicked_object: None,
            mouse_world: None,
            time: 0.0,
            dragging: None,
            drag_result: None,
            pending_drag_offset: None,
            show_bg: true,
            show_ground: false,
            show_grid: true,
            fan_emitters: Vec::new(),
            fan_particles: Vec::new(),
            wind_areas: Vec::new(),
            wind_particles: Vec::new(),
            wind_spawn_accum: Vec::new(),
            zzz_particles: Vec::new(),
            zzz_emit_accum: Vec::new(),
            bird_positions: Vec::new(),
            cloud_instances: Vec::new(),
            wgpu_device,
            wgpu_queue,
            edge_resources,
            edge_gpu_meshes: Arc::new(Vec::new()),
            edge_gpu_mesh_index: Vec::new(),
            bg_resources,
            bg_atlas_cache: bg_shader::BgAtlasCache::new(),
            bg_slot_counter: 0,
            opaque_resources,
            opaque_atlas: None,
            opaque_batch: None,
            opaque_sprite_map: Vec::new(),
            sprite_resources,
            sprite_atlas_cache: sprite_shader::SpriteAtlasCache::new(),
            sprite_slot_counter: 0,
            fill_resources,
            fill_texture_cache: fill_shader::FillTextureCache::new(),
            fill_gpu_meshes: Vec::new(),
            fill_slot_counter: 0,
            terrain_scratch_mesh: egui::Mesh::default(),
        }
    }

    /// Set the level-refs key (derived from filename) for prefab name overrides.
    pub fn set_level_key(&mut self, filename: &str) {
        self.level_key = crate::level_refs::level_key_from_filename(filename);
    }

    /// Rebuild cached data when a new level is loaded.
    pub fn set_level(&mut self, level: &LevelData) {
        // Drop old GPU resources (wgpu resources are reference-counted)
        self.edge_gpu_meshes = Arc::new(Vec::new());
        self.opaque_batch = None;
        self.opaque_sprite_map.clear();
        self.pending_drag_offset = None;

        self.world_positions.clear();
        self.terrain_data.clear();
        self.sprite_data.clear();
        self.fan_emitters.clear();
        self.fan_particles.clear();
        self.wind_areas.clear();
        self.wind_particles.clear();
        self.wind_spawn_accum.clear();
        self.zzz_particles.clear();
        self.zzz_emit_accum.clear();
        self.bird_positions.clear();
        self.cloud_instances.clear();

        // Collect all object names for BG theme detection
        let names: Vec<String> = level
            .objects
            .iter()
            .map(|o| match o {
                LevelObject::Prefab(p) => p.name.clone(),
                LevelObject::Parent(p) => p.name.clone(),
            })
            .collect();
        self.bg_theme = assets::detect_bg_theme(&names);

        // Find BackgroundObject override text for BG position adjustments
        self.bg_override_text = find_bg_override_text(&level.objects);

        // Build background layer cache (pre-compute tile grouping/singletons)
        self.bg_layer_cache = self.bg_theme.and_then(|theme| {
            background::build_bg_layer_cache(theme, self.bg_override_text.as_deref())
        });

        // Compute world positions and build draw data
        for (idx, obj) in level.objects.iter().enumerate() {
            let world_pos = compute_world_position(level, idx);
            self.world_positions.push((idx, world_pos));

            match obj {
                LevelObject::Prefab(prefab) => {
                    if prefab.terrain_data.is_some() {
                        self.terrain_data.push(terrain::build_terrain(
                            prefab,
                            world_pos,
                            &self.level_key,
                            idx,
                        ));
                    }
                    // Resolve correct sprite name via level-refs override
                    let resolved_name = crate::level_refs::get_prefab_override(
                        &self.level_key,
                        prefab.prefab_index,
                    );
                    self.sprite_data.push(sprites::build_sprite(
                        prefab,
                        world_pos,
                        idx,
                        resolved_name,
                    ));
                }
                LevelObject::Parent(_) => {}
            }
        }

        // Update terrain sprites' half_size from terrain bounding boxes
        for td in &self.terrain_data {
            // Collect world-space points from curve or fill mesh
            let pts: &[(f32, f32)] = if td.curve_world_verts.len() >= 2 {
                &td.curve_world_verts
            } else {
                continue;
            };
            let mut min_x = f32::MAX;
            let mut max_x = f32::MIN;
            let mut min_y = f32::MAX;
            let mut max_y = f32::MIN;
            for &(wx, wy) in pts {
                min_x = min_x.min(wx);
                max_x = max_x.max(wx);
                min_y = min_y.min(wy);
                max_y = max_y.max(wy);
            }
            // Also include fill mesh vertices for a more complete bounding box
            if let Some(ref fm) = td.fill_mesh {
                for v in &fm.vertices {
                    min_x = min_x.min(v.pos.x);
                    max_x = max_x.max(v.pos.x);
                    min_y = min_y.min(v.pos.y);
                    max_y = max_y.max(v.pos.y);
                }
            }
            let cx = (min_x + max_x) * 0.5;
            let cy = (min_y + max_y) * 0.5;
            let hw = (max_x - min_x) * 0.5;
            let hh = (max_y - min_y) * 0.5;
            if let Some(sprite) = self
                .sprite_data
                .iter_mut()
                .find(|s| s.index == td.object_index)
            {
                sprite.world_pos.x = cx;
                sprite.world_pos.y = cy;
                sprite.half_size = (hw.max(0.5), hh.max(0.5));
            }
        }

        // Sort terrain by world Z back-to-front (larger Z = farther = drawn first).
        // This mirrors Unity's orthographic Z-depth: decorative terrain (Z≈5) draws
        // behind ground background (Z≈2) and collider terrain (Z≈0) automatically.
        self.terrain_data.sort_by(|a, b| {
            b.world_z
                .partial_cmp(&a.world_z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Sort sprites back-to-front by Z position (higher Z = farther = rendered first).
        // In the game, smaller Z = closer to camera = renders on top. egui has no depth
        // buffer, so painter's order determines layering.
        self.sprite_data.sort_by(|a, b| {
            b.world_pos
                .z
                .partial_cmp(&a.world_pos.z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Upload terrain edge meshes to GPU (wgpu path)
        // Track which terrains get GPU-uploaded edge meshes (terrain_index → gpu_mesh_index)
        self.edge_gpu_mesh_index = vec![None; self.terrain_data.len()];
        if let (Some(device), Some(queue), Some(resources)) =
            (&self.wgpu_device, &self.wgpu_queue, &self.edge_resources)
        {
            let mut gpu_meshes = Vec::new();
            for (ti, td) in self.terrain_data.iter().enumerate() {
                if td.edge_vertices.is_empty() || td.edge_ctrl_pixels.is_none() {
                    continue;
                }
                let ctrl = td.edge_ctrl_pixels.as_ref().unwrap();
                // Load splat textures from embedded assets
                let splat0 = td
                    .edge_splat0
                    .as_ref()
                    .and_then(|name| load_raw_rgba(&format!("ground/{}", name)));
                let splat1 = td
                    .edge_splat1
                    .as_ref()
                    .and_then(|name| load_raw_rgba(&format!("ground/{}", name)));
                log::info!(
                    "terrain[{}] edge: verts={} indices={} ctrl={}B splat0={} splat1={} splatParamsX={:.4}",
                    ti,
                    td.edge_vertices.len(),
                    td.edge_indices.len(),
                    ctrl.len(),
                    td.edge_splat0.as_deref().unwrap_or("NONE"),
                    td.edge_splat1.as_deref().unwrap_or("NONE"),
                    td.edge_splat_params_x,
                );
                let gpu_mesh = edge_shader::upload_edge_mesh(
                    device,
                    queue,
                    resources,
                    &edge_shader::EdgeMeshInput {
                        vertices: &td.edge_vertices,
                        indices: &td.edge_indices,
                        control_pixels: ctrl,
                        control_node_count: td.edge_node_count,
                        splat0_pixels: splat0.as_ref().map(|(px, _, _)| px.as_slice()),
                        splat0_w: splat0.as_ref().map(|(_, w, _)| *w).unwrap_or(0),
                        splat0_h: splat0.as_ref().map(|(_, _, h)| *h).unwrap_or(0),
                        splat1_pixels: splat1.as_ref().map(|(px, _, _)| px.as_slice()),
                        splat1_w: splat1.as_ref().map(|(_, w, _)| *w).unwrap_or(0),
                        splat1_h: splat1.as_ref().map(|(_, _, h)| *h).unwrap_or(0),
                        splat_params_x: td.edge_splat_params_x,
                        decorative: td.decorative,
                    },
                );
                if gpu_mesh.has_both_splats {
                    self.edge_gpu_mesh_index[ti] = Some(gpu_meshes.len());
                    log::info!("  → terrain[{}] GPU edge active (both splats loaded)", ti);
                } else {
                    log::warn!("  → terrain[{}] GPU edge fallback (missing splat)", ti);
                }
                gpu_meshes.push(gpu_mesh);
            }
            self.edge_gpu_meshes = Arc::new(gpu_meshes);
        }

        // Build GPU vertex/index buffers for terrain fill meshes
        self.fill_gpu_meshes = Vec::new();
        if let Some(ref device) = self.wgpu_device {
            for td in &self.terrain_data {
                if let Some(ref fill) = td.fill_mesh {
                    let vertices: Vec<fill_shader::FillVertex> = fill
                        .vertices
                        .iter()
                        .map(|v| fill_shader::FillVertex {
                            pos: [v.pos.x, v.pos.y],
                            uv: [v.uv.x, v.uv.y],
                        })
                        .collect();
                    let gpu = fill_shader::build_fill_gpu_mesh(device, &vertices, &fill.indices);
                    self.fill_gpu_meshes.push(Some(Arc::new(gpu)));
                } else {
                    self.fill_gpu_meshes.push(None);
                }
            }
        }

        // Build opaque sprite batch for Props sprites (wgpu path)
        self.opaque_batch = None;
        self.opaque_sprite_map = vec![None; self.sprite_data.len()];
        if let (Some(device), Some(queue), Some(resources)) =
            (&self.wgpu_device, &self.wgpu_queue, &self.opaque_resources)
        {
            // Lazy-load Props atlas texture as wgpu resource
            if self.opaque_atlas.is_none()
                && let Some(atlas) = opaque_shader::load_props_atlas(device, queue)
            {
                log::info!("Opaque atlas loaded: {}×{}", atlas.width, atlas.height);
                self.opaque_atlas = Some(Arc::new(atlas));
            }
            if let Some(ref atlas) = self.opaque_atlas {
                let mut vertices = Vec::new();
                for (i, sprite) in self.sprite_data.iter().enumerate() {
                    if sprite.atlas.as_deref() != Some("Props_Generic_Sheet_01.png") {
                        continue;
                    }
                    // Emissive / collectible sprites keep their original material
                    // and are NOT tinted by GenericProps theme variants.
                    if assets::skip_props_tint(&sprite.name) {
                        continue;
                    }
                    if let Some(uv) = &sprite.uv {
                        let idx = (vertices.len() / 4) as u32;
                        self.opaque_sprite_map[i] = Some(idx);
                        let quad = opaque_shader::build_quad(
                            opaque_shader::QuadGeometry {
                                cx: sprite.world_pos.x,
                                cy: sprite.world_pos.y,
                                half_w: sprite.half_size.0,
                                half_h: sprite.half_size.1,
                                rotation: sprite.rotation,
                                scale_x: sprite.scale.0,
                                scale_y: sprite.scale.1,
                            },
                            uv,
                            atlas.width as f32,
                            atlas.height as f32,
                        );
                        vertices.extend_from_slice(&quad);
                    }
                }
                if !vertices.is_empty() {
                    log::info!("Built opaque sprite batch: {} quads", vertices.len() / 4);
                    self.opaque_batch = Some(Arc::new(opaque_shader::build_opaque_sprites(
                        device, resources, atlas, &vertices,
                    )));
                }
            }
        }

        // Build fan emitter state machines + wind area definitions
        for (i, sprite) in self.sprite_data.iter().enumerate() {
            if sprite.name == "Fan" {
                let ovr = compounds::parse_fan_override_public(sprite.override_text.as_deref());
                let on_time = ovr.on_time.unwrap_or(4.0);
                let delayed_start = ovr.delayed_start.unwrap_or(1.0) + on_time;
                let always_on = ovr.always_on.unwrap_or(true);
                let init_state = if always_on {
                    FanState::SpinUp
                } else if delayed_start > 0.0 {
                    FanState::DelayedStart
                } else {
                    FanState::SpinUp
                };
                self.fan_emitters.push(FanEmitter {
                    sprite_index: i,
                    state: init_state,
                    counter: 0.0,
                    force: 0.0,
                    emitting: init_state == FanState::SpinUp,
                    angle: 0.0,
                    world_x: sprite.world_pos.x,
                    world_y: sprite.world_pos.y,
                    rot: sprite.rotation,
                    burst_time: pseudo_random(i as u32 * 997), // stagger bursts across fans
                    start_time: ovr.start_time.unwrap_or(2.0),
                    on_time,
                    off_time: ovr.off_time.unwrap_or(2.0),
                    delayed_start,
                    always_on,
                });
            }
            // Collect WindArea zones
            if sprite.name.starts_with("WindArea") {
                let sx = sprite.scale.0.abs().max(1.0);
                let sy = sprite.scale.1.abs().max(1.0);
                self.wind_areas.push(WindAreaDef {
                    center_x: sprite.world_pos.x,
                    center_y: sprite.world_pos.y,
                    half_w: 20.0 * sx,
                    half_h: 7.5 * sy,
                });
            }
            // Collect Bird positions for Zzz particles
            if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
                self.bird_positions.push(Vec2 {
                    x: sprite.world_pos.x,
                    y: sprite.world_pos.y,
                });
            }
        }
        self.wind_spawn_accum = vec![0.0; self.wind_areas.len()];
        // Pre-spawn some wind particles
        for area_idx in 0..self.wind_areas.len() {
            let count = ((self.wind_areas[area_idx].half_w * self.wind_areas[area_idx].half_h * 2.0
                / 300.0) as usize)
                .max(3);
            for _ in 0..count {
                spawn_wind_particle(&self.wind_areas[area_idx], &mut self.wind_particles);
                if let Some(p) = self.wind_particles.last_mut() {
                    let frac = pseudo_random(p.x as u32) * 0.8;
                    p.age = p.lifetime * frac;
                    p.x += p.vx * p.age;
                    p.y += p.vy * p.age;
                }
            }
        }

        self.zzz_emit_accum = vec![0.0; self.bird_positions.len()];

        // Spawn cloud instances from CloudSet level objects
        for (idx, obj) in level.objects.iter().enumerate() {
            let obj_name = match obj {
                LevelObject::Prefab(p) => &p.name,
                LevelObject::Parent(p) => &p.name,
            };
            for &(config_name, ref config) in CLOUD_CONFIGS {
                if obj_name != config_name {
                    continue;
                }
                let pos = compute_world_position(level, idx);
                let cx = pos.x;
                let cy = pos.y;
                let mut seed: u32 = (cx * 1000.0) as u32 ^ (cy * 1000.0) as u32;
                for i in 0..config.max_clouds {
                    seed = (seed.wrapping_mul(1103515245)).wrapping_add(12345);
                    let info = &config.sprites[(seed as usize) % config.sprites.len()];
                    seed = seed.wrapping_mul(7) ^ (i as u32);
                    let x = cx + ((seed % 10000) as f32 / 5000.0 - 1.0) * config.limits;
                    seed = seed.wrapping_mul(13) ^ (i as u32 + 1);
                    let y = cy + ((seed % 10000) as f32 / 5000.0 - 1.0) * config.height;
                    seed = seed.wrapping_mul(17) ^ (i as u32 + 2);
                    let opacity = 0.8 + (seed % 200) as f32 / 1000.0;
                    self.cloud_instances.push(CloudInstance {
                        x,
                        y,
                        center_x: cx,
                        limits: config.limits,
                        velocity: config.velocity,
                        opacity,
                        sprite_name: info.name.to_string(),
                        atlas: info.atlas.to_string(),
                        scale_x: info.scale_x,
                        scale_y: info.scale_y,
                    });
                }
            }
        }

        // Parse construction grid from LevelManager override data
        self.construction_grid = grid::parse_construction_grid(level);

        // Fit camera to level bounds
        self.fit_to_level();
    }

    /// Fit camera to show all objects.
    pub fn fit_to_level(&mut self) {
        if self.world_positions.is_empty() {
            return;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for &(_, pos) in &self.world_positions {
            min_x = min_x.min(pos.x);
            max_x = max_x.max(pos.x);
            min_y = min_y.min(pos.y);
            max_y = max_y.max(pos.y);
        }

        let padding = 5.0;
        self.camera.center = Vec2 {
            x: (min_x + max_x) / 2.0,
            y: (min_y + max_y) / 2.0,
        };

        let range_x = (max_x - min_x) + padding * 2.0;
        let range_y = (max_y - min_y) + padding * 2.0;
        let range = range_x.max(range_y).max(1.0);
        self.camera.zoom = (600.0 / range).clamp(5.0, 200.0);
    }

    /// Hit test: find the topmost non-terrain sprite under a world position.
    /// Draw a single terrain's edge using CPU fallback (splat textures or flat vertex-color).
    fn draw_terrain_edge_cpu(
        painter: &egui::Painter,
        td: &terrain::TerrainDrawData,
        camera: &Camera,
        canvas_center: egui::Vec2,
        tex_cache: &assets::TextureCache,
        scratch: &mut egui::Mesh,
    ) {
        let mut drew_textured = false;
        if let Some(ref sm) = td.edge_splat0_mesh
            && let Some(ref name) = td.edge_splat0
            && let Some(tex_id) = tex_cache.get(name)
        {
            terrain::transform_mesh_to_screen_into(sm, camera, canvas_center, scratch);
            scratch.texture_id = tex_id;
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
            drew_textured = true;
        }
        if let Some(ref sm) = td.edge_splat1_mesh
            && let Some(ref name) = td.edge_splat1
            && let Some(tex_id) = tex_cache.get(name)
        {
            terrain::transform_mesh_to_screen_into(sm, camera, canvas_center, scratch);
            scratch.texture_id = tex_id;
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
            drew_textured = true;
        }
        if !drew_textured && let Some(ref edge) = td.edge_mesh {
            terrain::transform_mesh_to_screen_into(edge, camera, canvas_center, scratch);
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
        }
    }

    fn hit_test(&self, pos: Vec2, selected: Option<ObjectIndex>) -> Option<ObjectIndex> {
        let mut best: Option<(ObjectIndex, f32)> = None;
        for sprite in self.sprite_data.iter().rev() {
            // Allow terrain through only if it's the currently selected object
            if sprite.is_terrain && selected != Some(sprite.index) {
                continue;
            }
            // Skip objects that are hidden (not rendered) unless selected or parent is selected
            let is_selected = selected == Some(sprite.index)
                || (sprite.is_hidden && sprite.parent.is_some() && selected == sprite.parent);
            if !is_selected && sprite.is_hidden {
                continue;
            }
            let dx = (pos.x - sprite.world_pos.x).abs();
            let dy = (pos.y - sprite.world_pos.y).abs();
            if dx <= sprite.half_size.0 && dy <= sprite.half_size.1 {
                // For terrain, refine hit test using fill mesh triangles
                if sprite.is_terrain && !self.point_in_terrain(sprite.index, pos) {
                    continue;
                }
                let dist = dx * dx + dy * dy;
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((sprite.index, dist));
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Check whether a world-space point lies inside a terrain's fill mesh triangles.
    fn point_in_terrain(&self, index: ObjectIndex, pos: Vec2) -> bool {
        let td = self.terrain_data.iter().find(|t| t.object_index == index);
        let td = match td {
            Some(t) => t,
            None => return true, // no terrain data → fall back to AABB
        };
        let mesh = match td.fill_mesh {
            Some(ref m) => m,
            None => return true, // no fill mesh → fall back to AABB
        };
        let verts = &mesh.vertices;
        let indices = &mesh.indices;
        // Apply drag offset so hit test matches the drawn position
        let (ox, oy) = self.terrain_drag_offset(index);
        for tri in indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if i0 >= verts.len() || i1 >= verts.len() || i2 >= verts.len() {
                continue;
            }
            let a = egui::pos2(verts[i0].pos.x + ox, verts[i0].pos.y + oy);
            let b = egui::pos2(verts[i1].pos.x + ox, verts[i1].pos.y + oy);
            let c = egui::pos2(verts[i2].pos.x + ox, verts[i2].pos.y + oy);
            if point_in_triangle(egui::pos2(pos.x, pos.y), a, b, c) {
                return true;
            }
        }
        false
    }

    /// Returns (dx, dy) drag offset for a given object index, or (0, 0) if not being dragged.
    fn terrain_drag_offset(&self, object_index: ObjectIndex) -> (f32, f32) {
        if let Some(ref drag) = self.dragging {
            if drag.index == object_index
                && let Some(sprite) = self.sprite_data.iter().find(|s| s.index == object_index)
            {
                return (
                    sprite.world_pos.x - drag.original_pos.x,
                    sprite.world_pos.y - drag.original_pos.y,
                );
            }
        } else if let Some((idx, dx, dy)) = self.pending_drag_offset
            && idx == object_index
        {
            return (dx, dy);
        }
        (0.0, 0.0)
    }

    /// Show the level canvas. Returns the index of a newly-clicked object, if any.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        selected: Option<ObjectIndex>,
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

        // Background (sky + ground fill)
        if self.show_bg {
            background::draw_background(&painter, rect, &self.camera, canvas_center, self.bg_theme);

            // Reset per-frame bg shader slot counter
            self.bg_slot_counter = 0;
            // Reset per-frame sprite shader slot counter
            self.sprite_slot_counter = 0;
            // Reset per-frame fill shader slot counter
            self.fill_slot_counter = 0;

            // Parallax background layers: sky (worldZ >= 17.5, before cloud instances)
            if let (Some(theme_name), Some(cache)) = (self.bg_theme, &self.bg_layer_cache) {
                let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
                    (Some(r), Some(d), Some(q)) => Some(background::BgGpuState {
                        resources: r.clone(),
                        atlas_cache: &mut self.bg_atlas_cache,
                        device: d,
                        queue: q,
                        slot_counter: &mut self.bg_slot_counter,
                    }),
                    _ => None,
                };
                background::draw_bg_layers(
                    &DrawCtx {
                        painter: &painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    theme_name,
                    self.time,
                    (17.5, f32::INFINITY),
                    cache,
                    gpu.as_mut(),
                );
            }

            // Cloud sprite instances (drift + wrap) — Z=15, between sky(Z=20) and near(Z≈6)
            let dt = ui.input(|i| i.stable_dt);
            for cloud in &mut self.cloud_instances {
                cloud.x += cloud.velocity * dt;
                if cloud.x > cloud.center_x + cloud.limits {
                    cloud.x = cloud.center_x - cloud.limits;
                } else if cloud.x < cloud.center_x - cloud.limits {
                    cloud.x = cloud.center_x + cloud.limits;
                }
                // Render cloud sprite (camera-layer parallax: offset by camera.center
                // so clouds stay fixed on screen, matching TS cloudGroup.position.x = camX * 1.0)
                if let Some(info) = crate::sprite_db::get_sprite_info(&cloud.sprite_name) {
                    let hw = info.world_w * cloud.scale_x;
                    let hh = info.world_h * cloud.scale_y;
                    let center = self.camera.world_to_screen(
                        Vec2 {
                            x: cloud.x + self.camera.center.x,
                            y: cloud.y,
                        },
                        canvas_center,
                    );
                    let sw = hw * 2.0 * self.camera.zoom;
                    let sh = hh * 2.0 * self.camera.zoom;
                    if center.x + sw < rect.left() - 50.0
                        || center.x - sw > rect.right() + 50.0
                        || center.y + sh < rect.top() - 50.0
                        || center.y - sh > rect.bottom() + 50.0
                    {
                        continue;
                    }
                    let draw_rect = egui::Rect::from_center_size(center, egui::vec2(sw, sh));
                    if let Some(tex_id) = self.tex_cache.get(&cloud.atlas) {
                        // UV Y flip: Unity V=0 at bottom, egui V=0 at top
                        let uv_rect = egui::Rect::from_min_max(
                            egui::pos2(info.uv.x, 1.0 - info.uv.y - info.uv.h),
                            egui::pos2(info.uv.x + info.uv.w, 1.0 - info.uv.y),
                        );
                        let alpha = (cloud.opacity * 255.0) as u8;
                        let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                        let mut mesh = egui::Mesh::with_texture(tex_id);
                        mesh.add_rect_with_uv(draw_rect, uv_rect, tint);
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
            }

            // Parallax background layers: camera through near (5 <= worldZ < 17.5)
            if let (Some(theme_name), Some(cache)) = (self.bg_theme, &self.bg_layer_cache) {
                let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
                    (Some(r), Some(d), Some(q)) => Some(background::BgGpuState {
                        resources: r.clone(),
                        atlas_cache: &mut self.bg_atlas_cache,
                        device: d,
                        queue: q,
                        slot_counter: &mut self.bg_slot_counter,
                    }),
                    _ => None,
                };
                background::draw_bg_layers(
                    &DrawCtx {
                        painter: &painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    theme_name,
                    self.time,
                    (5.0, 17.5),
                    cache,
                    gpu.as_mut(),
                );
            }
        }

        // ── Decorative terrain (Z≈5, behind ground background Z≈2) ──
        // In Unity, decorative terrain sits at Z≈5 (farther from camera), so it's
        // occluded by ground background at Z≈2. We draw it before ground layers.

        // Decorative terrain: interleave fill + edge per terrain (back-to-front)
        // so that front terrain's fill covers back terrain's edge properly.
        for (ti, td) in self.terrain_data.iter().enumerate() {
            if !td.decorative {
                continue;
            }

            let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
            let cam_cx = self.camera.center.x - tdx;
            let cam_cy = self.camera.center.y - tdy;

            // ── Fill ──
            if let Some(ref fill_data) = td.fill_mesh {
                let mut gpu_done = false;
                if let (Some(resources), Some(device), Some(queue)) =
                    (&self.fill_resources, &self.wgpu_device, &self.wgpu_queue)
                    && let Some(Some(gpu_mesh)) = self.fill_gpu_meshes.get(ti)
                    && let Some(tex_name) = &td.fill_texture
                    && let Some(tex_gpu) = self
                        .fill_texture_cache
                        .get_or_load(device, queue, resources, tex_name)
                    && self.fill_slot_counter < fill_shader::max_draw_slots()
                {
                    let slot = self.fill_slot_counter;
                    self.fill_slot_counter += 1;
                    let fc = &fill_data.vertices[0].color;
                    let [r, g, b, a] = [
                        fc.r() as f32 / 255.0,
                        fc.g() as f32 / 255.0,
                        fc.b() as f32 / 255.0,
                        fc.a() as f32 / 255.0,
                    ];
                    let uniforms = fill_shader::FillUniforms {
                        screen_size: [rect.width(), rect.height()],
                        camera_center: [cam_cx, cam_cy],
                        zoom: self.camera.zoom,
                        _pad0: 0.0,
                        _pad1: [0.0; 2],
                        tint_color: [r, g, b, a],
                    };
                    painter.add(fill_shader::make_fill_callback(
                        rect,
                        resources.clone(),
                        tex_gpu,
                        gpu_mesh.clone(),
                        slot,
                        uniforms,
                    ));
                    gpu_done = true;
                }
                if !gpu_done {
                    let drag_cam = Camera {
                        center: Vec2 {
                            x: cam_cx,
                            y: cam_cy,
                        },
                        ..self.camera.clone()
                    };
                    terrain::transform_mesh_to_screen_into(
                        fill_data,
                        &drag_cam,
                        canvas_center,
                        &mut self.terrain_scratch_mesh,
                    );
                    if let Some(ref tex_name) = td.fill_texture
                        && let Some(tex_id) = self.tex_cache.get(tex_name)
                    {
                        self.terrain_scratch_mesh.texture_id = tex_id;
                    }
                    painter.add(egui::Shape::mesh(std::mem::take(
                        &mut self.terrain_scratch_mesh,
                    )));
                }
            }

            // ── Edge (right after fill, before next terrain's fill covers it) ──
            if let Some(gpu_idx) = self.edge_gpu_mesh_index.get(ti).copied().flatten() {
                if let Some(ref resources) = self.edge_resources {
                    painter.add(edge_shader::make_single_edge_paint_callback(
                        rect,
                        resources.clone(),
                        self.edge_gpu_meshes.clone(),
                        edge_shader::EdgeCameraParams {
                            screen_w: rect.width(),
                            screen_h: rect.height(),
                            camera_x: cam_cx,
                            camera_y: cam_cy,
                            zoom: self.camera.zoom,
                        },
                        gpu_idx,
                    ));
                }
            } else {
                let drag_cam = Camera {
                    center: Vec2 {
                        x: cam_cx,
                        y: cam_cy,
                    },
                    ..self.camera.clone()
                };
                Self::draw_terrain_edge_cpu(
                    &painter,
                    td,
                    &drag_cam,
                    canvas_center,
                    &self.tex_cache,
                    &mut self.terrain_scratch_mesh,
                );
            }
        }

        // ── Ground background (eff_z 0..5): beach/grass, behind collider terrain ──
        if self.show_bg
            && let Some(theme_name) = self.bg_theme
        {
            let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
                (Some(r), Some(d), Some(q)) => Some(background::BgGpuState {
                    resources: r.clone(),
                    atlas_cache: &mut self.bg_atlas_cache,
                    device: d,
                    queue: q,
                    slot_counter: &mut self.bg_slot_counter,
                }),
                _ => None,
            };
            if let Some(ref cache) = self.bg_layer_cache {
                background::draw_bg_layers(
                    &DrawCtx {
                        painter: &painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    theme_name,
                    self.time,
                    (0.0, 5.0), // ground behind terrain (beach/grass)
                    cache,
                    gpu.as_mut(),
                );
            }
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
                &self.tex_cache,
            );
        }

        // ── Interaction: drag, pan, click ──
        let is_shift = ui.input(|i| i.modifiers.shift);
        let is_alt = ui.input(|i| i.modifiers.alt);
        self.clicked_object = None;
        self.drag_result = None;

        // Start object drag on primary press (without shift/alt)
        if response.drag_started_by(egui::PointerButton::Primary)
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let world = self.camera.screen_to_world(pointer, canvas_center);
            if let Some(idx) = self.hit_test(world, selected) {
                let orig = self
                    .sprite_data
                    .iter()
                    .find(|s| s.index == idx)
                    .map(|s| s.world_pos)
                    .unwrap_or_default();
                self.dragging = Some(DragState {
                    index: idx,
                    start_mouse: world,
                    original_pos: orig,
                });
                self.clicked_object = Some(idx);
            }
        }

        // Handle pan (middle mouse / shift+drag / alt+drag / primary on empty space)
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && is_shift)
            || (response.dragged_by(egui::PointerButton::Primary) && is_alt)
            || (response.dragged_by(egui::PointerButton::Primary)
                && !is_shift
                && !is_alt
                && self.dragging.is_none())
        {
            let delta = response.drag_delta();
            self.camera.center.x -= delta.x / self.camera.zoom;
            self.camera.center.y += delta.y / self.camera.zoom;
            self.panning = true;
        } else if self.dragging.is_none() {
            self.panning = false;
        }

        // Update sprite position during object drag
        if let Some(ref drag) = self.dragging
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let current = self.camera.screen_to_world(pointer, canvas_center);
            let dx = current.x - drag.start_mouse.x;
            let dy = current.y - drag.start_mouse.y;
            let tidx = drag.index;
            let orig = drag.original_pos;
            for sprite in &mut self.sprite_data {
                if sprite.index == tidx {
                    sprite.world_pos.x = orig.x + dx;
                    sprite.world_pos.y = orig.y + dy;
                    break;
                }
            }
        }

        // End object drag
        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(drag) = self.dragging.take()
        {
            for sprite in &self.sprite_data {
                if sprite.index == drag.index {
                    let dx = sprite.world_pos.x - drag.original_pos.x;
                    let dy = sprite.world_pos.y - drag.original_pos.y;
                    if dx.abs() > 0.001 || dy.abs() > 0.001 {
                        self.drag_result = Some((drag.index, Vec2 { x: dx, y: dy }));
                        // Keep camera offset active until batch is rebuilt
                        self.pending_drag_offset = Some((drag.index, dx, dy));
                    }
                    break;
                }
            }
        }

        // Click-to-select (tap without drag)
        if response.clicked()
            && !self.panning
            && let Some(click_pos) = response.interact_pointer_pos()
        {
            let click_world = self.camera.screen_to_world(click_pos, canvas_center);
            self.clicked_object = self.hit_test(click_world, selected);
        }

        // Handle zoom (scroll wheel, center-preserving)
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(pointer) = response.hover_pos() {
                    let world_before = self.camera.screen_to_world(pointer, canvas_center);
                    let factor = 1.0 + scroll * 0.002;
                    self.camera.zoom = (self.camera.zoom * factor).clamp(2.0, 500.0);
                    let world_after = self.camera.screen_to_world(pointer, canvas_center);
                    self.camera.center.x -= world_after.x - world_before.x;
                    self.camera.center.y -= world_after.y - world_before.y;
                } else {
                    let factor = 1.0 + scroll * 0.002;
                    self.camera.zoom = (self.camera.zoom * factor).clamp(2.0, 500.0);
                }
            }
        }

        // ── Collider terrain (Z≈0, in front of ground background) ──

        // Pre-compute world-space visible rect for early frustum culling
        let world_half_w = rect.width() * 0.5 / self.camera.zoom;
        let world_half_h = rect.height() * 0.5 / self.camera.zoom;
        let visible_min_x = self.camera.center.x - world_half_w;
        let visible_max_x = self.camera.center.x + world_half_w;
        let visible_min_y = self.camera.center.y - world_half_h;
        let visible_max_y = self.camera.center.y + world_half_h;

        // Glow starbursts behind collider terrain
        if let Some(glow_id) = self.tex_cache.get(GLOW_ATLAS) {
            for sprite in &self.sprite_data {
                if !sprites::has_glow(&sprite.name) {
                    continue;
                }
                // Early world-space frustum cull for glow (large radius ~1.5 world units)
                let glow_margin = 2.0;
                if sprite.world_pos.x + glow_margin < visible_min_x
                    || sprite.world_pos.x - glow_margin > visible_max_x
                    || sprite.world_pos.y + glow_margin < visible_min_y
                    || sprite.world_pos.y - glow_margin > visible_max_y
                {
                    continue;
                }
                sprites::draw_glow(
                    &painter,
                    sprite,
                    &self.camera,
                    canvas_center,
                    rect,
                    self.time,
                    glow_id,
                );
            }
        }

        // Collider terrain: interleave fill + edge per terrain (back-to-front)
        // so that front terrain's fill covers back terrain's edge properly.
        for (ti, td) in self.terrain_data.iter().enumerate() {
            if td.decorative {
                continue;
            }

            let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
            let cam_cx = self.camera.center.x - tdx;
            let cam_cy = self.camera.center.y - tdy;

            // ── Fill ──
            if let Some(ref fill_data) = td.fill_mesh {
                let mut gpu_done = false;
                if let (Some(resources), Some(device), Some(queue)) =
                    (&self.fill_resources, &self.wgpu_device, &self.wgpu_queue)
                    && let Some(Some(gpu_mesh)) = self.fill_gpu_meshes.get(ti)
                    && let Some(tex_name) = &td.fill_texture
                    && let Some(tex_gpu) = self
                        .fill_texture_cache
                        .get_or_load(device, queue, resources, tex_name)
                    && self.fill_slot_counter < fill_shader::max_draw_slots()
                {
                    let slot = self.fill_slot_counter;
                    self.fill_slot_counter += 1;
                    let fc = &fill_data.vertices[0].color;
                    let [r, g, b, a] = [
                        fc.r() as f32 / 255.0,
                        fc.g() as f32 / 255.0,
                        fc.b() as f32 / 255.0,
                        fc.a() as f32 / 255.0,
                    ];
                    let uniforms = fill_shader::FillUniforms {
                        screen_size: [rect.width(), rect.height()],
                        camera_center: [cam_cx, cam_cy],
                        zoom: self.camera.zoom,
                        _pad0: 0.0,
                        _pad1: [0.0; 2],
                        tint_color: [r, g, b, a],
                    };
                    painter.add(fill_shader::make_fill_callback(
                        rect,
                        resources.clone(),
                        tex_gpu,
                        gpu_mesh.clone(),
                        slot,
                        uniforms,
                    ));
                    gpu_done = true;
                }
                if !gpu_done {
                    let drag_cam = Camera {
                        center: Vec2 {
                            x: cam_cx,
                            y: cam_cy,
                        },
                        ..self.camera.clone()
                    };
                    terrain::transform_mesh_to_screen_into(
                        fill_data,
                        &drag_cam,
                        canvas_center,
                        &mut self.terrain_scratch_mesh,
                    );
                    if let Some(ref tex_name) = td.fill_texture
                        && let Some(tex_id) = self.tex_cache.get(tex_name)
                    {
                        self.terrain_scratch_mesh.texture_id = tex_id;
                    }
                    painter.add(egui::Shape::mesh(std::mem::take(
                        &mut self.terrain_scratch_mesh,
                    )));
                }
            }

            // ── Edge (right after fill, before next terrain's fill covers it) ──
            if let Some(gpu_idx) = self.edge_gpu_mesh_index.get(ti).copied().flatten() {
                if let Some(ref resources) = self.edge_resources {
                    painter.add(edge_shader::make_single_edge_paint_callback(
                        rect,
                        resources.clone(),
                        self.edge_gpu_meshes.clone(),
                        edge_shader::EdgeCameraParams {
                            screen_w: rect.width(),
                            screen_h: rect.height(),
                            camera_x: cam_cx,
                            camera_y: cam_cy,
                            zoom: self.camera.zoom,
                        },
                        gpu_idx,
                    ));
                }
            } else {
                let drag_cam = Camera {
                    center: Vec2 {
                        x: cam_cx,
                        y: cam_cy,
                    },
                    ..self.camera.clone()
                };
                Self::draw_terrain_edge_cpu(
                    &painter,
                    td,
                    &drag_cam,
                    canvas_center,
                    &self.tex_cache,
                    &mut self.terrain_scratch_mesh,
                );
            }
        }

        // Selection outlines for all terrain
        for td in self.terrain_data.iter() {
            if selected == Some(td.object_index) && td.curve_world_verts.len() >= 2 {
                let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
                let screen_pts: Vec<egui::Pos2> = td
                    .curve_world_verts
                    .iter()
                    .map(|&(wx, wy)| {
                        self.camera.world_to_screen(
                            Vec2 {
                                x: wx + tdx,
                                y: wy + tdy,
                            },
                            canvas_center,
                        )
                    })
                    .collect();
                let stroke = egui::Stroke::new(2.0, egui::Color32::YELLOW);
                for pair in screen_pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
            }
        }

        // ── Fan state machine update ──
        let dt = ui.input(|i| i.stable_dt);
        for emitter in &mut self.fan_emitters {
            const SPINDOWN_TIME: f32 = 2.0;
            match emitter.state {
                FanState::DelayedStart => {
                    emitter.counter += dt;
                    if emitter.counter >= emitter.delayed_start {
                        emitter.state = FanState::SpinUp;
                        emitter.counter = 0.0;
                        emitter.emitting = true;
                    }
                }
                FanState::SpinUp => {
                    emitter.counter += dt;
                    if emitter.counter >= emitter.start_time {
                        emitter.state = FanState::Spinning;
                        emitter.counter = 0.0;
                        emitter.force = 1.0;
                    } else {
                        let t = emitter.counter / emitter.start_time;
                        emitter.force = t * t; // spinupRamp: t²
                    }
                }
                FanState::Spinning => {
                    emitter.force = 1.0;
                    if !emitter.always_on {
                        emitter.counter += dt;
                        if emitter.counter >= emitter.on_time {
                            emitter.emitting = false;
                            emitter.state = FanState::SpinDown;
                            emitter.counter = 0.0;
                        }
                    }
                }
                FanState::SpinDown => {
                    emitter.counter += dt;
                    let t = (emitter.counter / SPINDOWN_TIME).min(1.0);
                    emitter.force = 1.0 - t;
                    if t >= 1.0 {
                        emitter.state = FanState::Inactive;
                        emitter.counter = 0.0;
                        emitter.force = 0.0;
                    }
                }
                FanState::Inactive => {
                    emitter.force = 0.0;
                    emitter.counter += dt;
                    if emitter.counter >= emitter.off_time {
                        emitter.state = FanState::SpinUp;
                        emitter.counter = 0.0;
                        emitter.emitting = true;
                    }
                }
            }
            // Update propeller angle
            const FAN_ROTATION_SPEED: f32 = 600.0 * std::f32::consts::PI / 180.0; // 10.472 rad/s
            emitter.angle += FAN_ROTATION_SPEED * emitter.force * dt;
        }

        // ── Fan particle burst emission ──
        const BURST_TIMES: [f32; 4] = [0.00, 0.25, 0.50, 0.75];
        const FAN_PARTICLE_MAX: usize = 40;
        for ei in 0..self.fan_emitters.len() {
            let prev_t = self.fan_emitters[ei].burst_time;
            self.fan_emitters[ei].burst_time += dt;
            if self.fan_emitters[ei].burst_time >= 1.0 {
                self.fan_emitters[ei].burst_time -= 1.0;
            }
            let new_t = self.fan_emitters[ei].burst_time;
            if !self.fan_emitters[ei].emitting {
                continue;
            }
            for &bt in &BURST_TIMES {
                let fired = (prev_t <= bt && new_t > bt)
                    || (prev_t > new_t && (prev_t <= bt || new_t > bt));
                if fired && self.fan_particles.len() < FAN_PARTICLE_MAX {
                    let e = &self.fan_emitters[ei];
                    let seed = (self.time * 1000.0) as u32
                        + ei as u32 * 773
                        + self.fan_particles.len() as u32 * 419;
                    let r1 = pseudo_random(seed);
                    let r2 = pseudo_random(seed.wrapping_add(1));
                    let r3 = pseudo_random(seed.wrapping_add(2));
                    let r4 = pseudo_random(seed.wrapping_add(3));
                    let ox = (r1 - 0.5) * 0.98; // ±0.49 spread along X
                    let ly = 0.6365_f32; // local Y offset from fan center
                    let cos_r = e.rot.cos();
                    let sin_r = e.rot.sin();
                    let px = e.world_x + ox * cos_r - ly * sin_r;
                    let py = e.world_y + ox * sin_r + ly * cos_r;
                    let local_vy = 3.0 + r2 * 7.0;
                    let local_vx = (r3 - 0.5) * 0.2;
                    let vx = local_vx * cos_r - local_vy * sin_r;
                    let vy = local_vx * sin_r + local_vy * cos_r;
                    self.fan_particles.push(FanParticle {
                        x: px,
                        y: py,
                        vx,
                        vy,
                        age: 0.0,
                        lifetime: 0.7 + r4 * 0.8,
                        start_size: 1.2,
                        rot: pseudo_random(seed.wrapping_add(4)) * std::f32::consts::PI,
                        rot_speed: std::f32::consts::FRAC_PI_4
                            + pseudo_random(seed.wrapping_add(5)) * std::f32::consts::PI * 0.75,
                    });
                }
            }
        }
        // Update fan particles
        let mut fi = 0;
        while fi < self.fan_particles.len() {
            let p = &mut self.fan_particles[fi];
            p.age += dt;
            let t = p.age / p.lifetime;
            if t >= 1.0 {
                self.fan_particles.swap_remove(fi);
                continue;
            }
            p.vy -= 0.5 * dt; // gravity/deceleration
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            let spin_rate = p.rot_speed * (1.0 - t * 0.85);
            p.rot += spin_rate * dt;
            fi += 1;
        }

        // ── Wind leaf particle update ──
        // Spawn new particles
        for a in 0..self.wind_areas.len() {
            self.wind_spawn_accum[a] += dt;
            let area_count = self
                .wind_particles
                .iter()
                .filter(|p| {
                    (p.x - self.wind_areas[a].center_x).abs() < self.wind_areas[a].half_w * 1.5
                })
                .count();
            if self.wind_spawn_accum[a] >= 1.0 && area_count < 20 {
                self.wind_spawn_accum[a] -= 1.0;
                spawn_wind_particle(&self.wind_areas[a], &mut self.wind_particles);
            }
        }
        // Update particles
        let mut i = 0;
        while i < self.wind_particles.len() {
            let p = &mut self.wind_particles[i];
            p.age += dt;
            if p.age >= p.lifetime {
                self.wind_particles.swap_remove(i);
                continue;
            }
            let t_frac = p.age / p.lifetime;
            p.x += p.vx * dt;
            let y_osc = ((t_frac + p.y_phase) * std::f32::consts::TAU).sin() * 0.5;
            p.y += (p.vy + y_osc) * dt;
            p.rot += p.rot_speed * dt;
            i += 1;
        }

        // ── Zzz particle update (bird sleeping) ──
        const ZZZ_EMIT_RATE: f32 = 2.0;
        const ZZZ_MAX_PER_BIRD: usize = 5;
        const ZZZ_SIZE_PEAK_T: f32 = 0.355244;
        const ZZZ_START_SIZE: f32 = 0.49; // 0.7 * 0.7
        const ZZZ_SPAWN_OFFSET_Y: f32 = 0.5;
        const ZZZ_SPAWN_SPREAD_X: f32 = 0.6;
        const ZZZ_SPAWN_SPREAD_Y: f32 = 0.52;

        // Spawn new Zzz particles
        for bi in 0..self.bird_positions.len() {
            if bi < self.zzz_emit_accum.len() {
                self.zzz_emit_accum[bi] += dt;
                while self.zzz_emit_accum[bi] >= 1.0 / ZZZ_EMIT_RATE
                    && self.zzz_particles.len() < ZZZ_MAX_PER_BIRD * self.bird_positions.len()
                {
                    self.zzz_emit_accum[bi] -= 1.0 / ZZZ_EMIT_RATE;
                    let bx = self.bird_positions[bi].x;
                    let by = self.bird_positions[bi].y;
                    let seed = (self.time * 1000.0) as u32
                        + bi as u32 * 997
                        + self.zzz_particles.len() as u32 * 337;
                    let r1 = pseudo_random(seed);
                    let r2 = pseudo_random(seed.wrapping_add(1));
                    let r3 = pseudo_random(seed.wrapping_add(2));
                    let r4 = pseudo_random(seed.wrapping_add(3));
                    let r5 = pseudo_random(seed.wrapping_add(4));
                    self.zzz_particles.push(ZzzParticle {
                        x: bx + (r1 - 0.5) * 2.0 * ZZZ_SPAWN_SPREAD_X,
                        y: by + ZZZ_SPAWN_OFFSET_Y + (r2 - 0.5) * 2.0 * ZZZ_SPAWN_SPREAD_Y,
                        vy: 0.31 + r3 * 0.18,
                        age: 0.0,
                        lifetime: 1.0 + r4,
                        start_size: ZZZ_START_SIZE,
                        wobble_phase: r5 * std::f32::consts::TAU,
                        wobble_freq: 0.8 + pseudo_random(seed.wrapping_add(5)) * 0.4,
                        rot: 0.0, // Unity startRotation = 0 (constant)
                        rot_speed: pseudo_random(seed.wrapping_add(6)) * 30.0_f32.to_radians(),
                    });
                }
            }
        }
        // Update Zzz particles
        let mut zi = 0;
        while zi < self.zzz_particles.len() {
            let p = &mut self.zzz_particles[zi];
            p.age += dt;
            if p.age >= p.lifetime {
                self.zzz_particles.swap_remove(zi);
                continue;
            }
            // X wobble: velocity curve from VelocityModule, scalar=2, amplitude ~0.7
            let wobble_vx =
                (p.wobble_phase + p.age * p.wobble_freq * std::f32::consts::TAU).sin() * 1.4;
            p.x += wobble_vx * dt;
            p.y += p.vy * dt;
            p.rot += p.rot_speed * dt;
            zi += 1;
        }

        // Draw Zzz particles BEFORE sprites — in Unity emitter is at z=+0.5 (behind bird body)
        // UV: Particles_Sheet_01.png 8×8 grid, row 2, frame 6 (constant frameOverTime=0.75)
        let zzz_tex = self.tex_cache.get(GLOW_ATLAS);
        for p in &self.zzz_particles {
            let life_t = p.age / p.lifetime;
            let size_scale = if life_t < ZZZ_SIZE_PEAK_T {
                let g = life_t / ZZZ_SIZE_PEAK_T;
                g * g * (3.0 - 2.0 * g) // smoothstep grow
            } else {
                let s = (life_t - ZZZ_SIZE_PEAK_T) / (1.0 - ZZZ_SIZE_PEAK_T);
                1.0 - s * s * (3.0 - 2.0 * s) // smoothstep shrink
            };
            let sz = p.start_size * size_scale * self.camera.zoom;
            if sz < 0.5 {
                continue;
            }
            let center = self
                .camera
                .world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
            if !rect.expand(20.0).contains(center) {
                continue;
            }
            let alpha = 255u8;
            if let Some(tex_id) = zzz_tex {
                const ZZZ_U0: f32 = 6.0 / 8.0; // 0.75
                const ZZZ_U1: f32 = 7.0 / 8.0; // 0.875
                const ZZZ_V0: f32 = 1.0 - 6.0 / 8.0; // 0.25
                const ZZZ_V1: f32 = 1.0 - 5.0 / 8.0; // 0.375
                let hw = sz;
                let hh = sz;
                let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                let mut mesh = egui::Mesh::with_texture(tex_id);
                let cos_r = p.rot.cos();
                let sin_r = p.rot.sin();
                let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                    egui::pos2(
                        center.x + dx * cos_r + dy * sin_r,
                        center.y - dx * sin_r + dy * cos_r,
                    )
                };
                let tl = rot(-hw, -hh);
                let tr = rot(hw, -hh);
                let br = rot(hw, hh);
                let bl = rot(-hw, hh);
                let i = mesh.vertices.len() as u32;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: tl,
                    uv: egui::pos2(ZZZ_U0, ZZZ_V0),
                    color: tint,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: tr,
                    uv: egui::pos2(ZZZ_U1, ZZZ_V0),
                    color: tint,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: br,
                    uv: egui::pos2(ZZZ_U1, ZZZ_V1),
                    color: tint,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: bl,
                    uv: egui::pos2(ZZZ_U0, ZZZ_V1),
                    color: tint,
                });
                mesh.indices
                    .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                painter.add(egui::Shape::mesh(mesh));
            }
        }

        // Sprites with goal bobbing + compound sub-sprites (renderOrder=12)
        let t = self.time;
        // Build fan angle lookup (avoids O(sprites × fans) per-frame scan)
        let mut fan_angle_map: Vec<Option<f32>> = vec![None; self.sprite_data.len()];
        for e in &self.fan_emitters {
            if e.sprite_index < fan_angle_map.len() {
                fan_angle_map[e.sprite_index] = Some(e.angle);
            }
        }
        // Collect GPU sprite draws into a single Z-ordered Vec (instead of two
        // separate type-specific Vecs). Consecutive same-type draws are batched
        // into one PaintCallback when emitted, minimising render state resets
        // while preserving correct Z interleaving between opaque (Props) and
        // transparent (non-Props) sprites.
        enum GpuDraw {
            Opaque(opaque_shader::OpaqueBatchDraw),
            Transparent(sprite_shader::SpriteBatchDraw),
        }
        let mut gpu_draws: Vec<GpuDraw> = Vec::new();
        // Deferred bird face draws: must render AFTER the GPU batch callbacks so
        // faces appear on top of GPU-rendered bird bodies.
        struct DeferredBird {
            name: String,
            wx: f32,
            wy: f32,
            sx: f32,
            sy: f32,
            rot: f32,
            bsx: f32,
            bsy: f32,
        }
        let mut deferred_birds: Vec<DeferredBird> = Vec::new();
        for (si, sprite) in self.sprite_data.iter().enumerate() {
            let is_sel = selected == Some(sprite.index)
                || (sprite.is_hidden && sprite.parent.is_some() && selected == sprite.parent);

            // Early world-space frustum cull — skip all rendering work for off-screen sprites.
            // Use generous margin (2 world units) to account for compound sub-sprites, rotation, etc.
            if !is_sel {
                let margin = sprite.half_size.0.max(sprite.half_size.1) + 2.0;
                let sx = sprite.world_pos.x;
                let sy = sprite.world_pos.y;
                if sx + margin < visible_min_x
                    || sx - margin > visible_max_x
                    || sy + margin < visible_min_y
                    || sy - margin > visible_max_y
                {
                    continue;
                }
            }

            let fan_angle = fan_angle_map[si];
            let skip_root = compounds::draw_compound(
                &DrawCtx {
                    painter: &painter,
                    camera: &self.camera,
                    canvas_center,
                    canvas_rect: rect,
                    tex_cache: &self.tex_cache,
                },
                &sprite.name,
                CompoundTransform {
                    world_x: sprite.world_pos.x,
                    world_y: sprite.world_pos.y,
                    scale_x: sprite.scale.0,
                    scale_y: sprite.scale.1,
                    rotation_z: sprite.rotation,
                },
                t,
                sprite.override_text.as_deref(),
            );

            // Draw goal flag mesh BEHIND the GoalArea icon sprite (flag is at Z=0, icon at Z=-0.5)
            if sprite.name_lower.starts_with("goalarea")
                && let Some(flag_tex) = self.tex_cache.get(GOAL_FLAG_TEXTURE)
            {
                sprites::draw_goal_flag(
                    &painter,
                    sprite,
                    &self.camera,
                    canvas_center,
                    rect,
                    t,
                    flag_tex,
                );
            }

            let mut is_gpu_rendered = false;

            if !skip_root {
                let opaque_idx = self.opaque_sprite_map.get(si).copied().flatten();
                // Props sprites: render via GPU opaque shader (exact Unity shader port)
                if let Some(oidx) = opaque_idx
                    && let (Some(_resources), Some(_batch)) =
                        (&self.opaque_resources, &self.opaque_batch)
                {
                    // Compute per-sprite y_offset (goal/dessert bobbing, bird sleep bob)
                    let y_off = if sprite.name_lower.contains("goal")
                        || sprite.name_lower.contains("dessert")
                    {
                        (t * 3.0).sin() as f32 * 0.25
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % sprites::BIRD_SLEEP_DURATION)
                            .max(0.0);
                        background::hermite(sprites::BIRD_SLEEP_POS_Y, bt)
                    } else {
                        0.0
                    };
                    // During drag, the sprite's world_pos is updated but the
                    // GPU vertex buffer still has the baked (original) position.
                    // Offset the camera center to compensate, which shifts the
                    // quad on screen without rebuilding the vertex buffer.
                    // pending_drag_offset keeps this active until the batch is rebuilt.
                    let (cam_x, cam_y) = if let Some(ref drag) = self.dragging {
                        if drag.index == sprite.index {
                            let dx = sprite.world_pos.x - drag.original_pos.x;
                            let dy = sprite.world_pos.y - drag.original_pos.y;
                            (self.camera.center.x - dx, self.camera.center.y - dy)
                        } else {
                            (self.camera.center.x, self.camera.center.y)
                        }
                    } else if let Some((idx, dx, dy)) = self.pending_drag_offset {
                        if idx == sprite.index {
                            (self.camera.center.x - dx, self.camera.center.y - dy)
                        } else {
                            (self.camera.center.x, self.camera.center.y)
                        }
                    } else {
                        (self.camera.center.x, self.camera.center.y)
                    };
                    gpu_draws.push(GpuDraw::Opaque(opaque_shader::OpaqueBatchDraw {
                        sprite_index: oidx,
                        cam_x,
                        cam_y,
                        y_offset: y_off,
                    }));
                    is_gpu_rendered = true;
                }
                // Non-Props sprites: render via GPU transparent sprite shader
                let mut _sprite_gpu_rendered = false;
                if (is_sel || !sprite.is_hidden)
                    && opaque_idx.is_none()
                    && let (Some(atlas_name), Some(uv)) = (&sprite.atlas, &sprite.uv)
                    && let (Some(resources), Some(device), Some(queue)) =
                        (&self.sprite_resources, &self.wgpu_device, &self.wgpu_queue)
                    && let Some(atlas) = self
                        .sprite_atlas_cache
                        .get_or_load(device, queue, resources, atlas_name)
                    && self.sprite_slot_counter < sprite_shader::max_draw_slots()
                {
                    let slot = self.sprite_slot_counter;
                    self.sprite_slot_counter += 1;

                    // Compute animation offsets
                    let y_off = if sprite.name_lower.contains("goal")
                        || sprite.name_lower.contains("dessert")
                    {
                        (t * 3.0).sin() as f32 * 0.25
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % sprites::BIRD_SLEEP_DURATION)
                            .max(0.0);
                        background::hermite(sprites::BIRD_SLEEP_POS_Y, bt)
                    } else {
                        0.0
                    };

                    // Animated half-size (Fan foreshorten, Bird scale)
                    let (hw, hh) = if sprite.name == "Fan" {
                        let angle = fan_angle.unwrap_or((t * 10.472) as f32);
                        let foreshorten = angle.cos().abs().max(0.05);
                        (sprite.half_size.0 * foreshorten, sprite.half_size.1)
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % 4.0).max(0.0);
                        let sx = background::hermite(sprites::BIRD_SLEEP_SCALE_X, bt);
                        let sy = background::hermite(sprites::BIRD_SLEEP_SCALE_Y, bt);
                        (sprite.half_size.0 * sx, sprite.half_size.1 * sy)
                    } else {
                        sprite.half_size
                    };

                    let (uv_min, uv_max) = sprite_shader::compute_uvs(
                        uv,
                        atlas.width as f32,
                        atlas.height as f32,
                        sprite.scale.0 < 0.0,
                        sprite.scale.1 < 0.0,
                    );

                    let uniforms = sprite_shader::SpriteUniforms {
                        screen_size: [rect.width(), rect.height()],
                        camera_center: [self.camera.center.x, self.camera.center.y],
                        zoom: self.camera.zoom,
                        rotation: sprite.rotation,
                        world_center: [sprite.world_pos.x, sprite.world_pos.y + y_off],
                        half_size: [hw, hh],
                        uv_min,
                        uv_max,
                        mode: 0.0,
                        shine_center: 0.0,
                        tint_color: [1.0, 1.0, 1.0, 1.0],
                    };

                    gpu_draws.push(GpuDraw::Transparent(sprite_shader::SpriteBatchDraw {
                        atlas,
                        slot,
                        uniforms,
                    }));
                    _sprite_gpu_rendered = true;
                    is_gpu_rendered = true;
                }
                let gpu_rendered = is_gpu_rendered;
                let tex_id = if gpu_rendered {
                    None
                } else {
                    sprite.atlas.as_ref().and_then(|a| self.tex_cache.get(a))
                };
                let atlas_size = if gpu_rendered {
                    None
                } else {
                    sprite
                        .atlas
                        .as_ref()
                        .and_then(|a| self.tex_cache.texture_size(a))
                };
                sprites::draw_sprite(
                    &DrawCtx {
                        painter: &painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    sprite,
                    sprites::SpriteDrawOpts {
                        is_selected: is_sel,
                        time: t,
                        tex_id,
                        atlas_size,
                        fan_angle,
                        opaque_rendered: gpu_rendered,
                    },
                );
            }

            // Bird face: defer if GPU-rendered so faces draw after batch callback
            if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
                let bt = ((t as f32 + sprite.bird_phase) % sprites::BIRD_SLEEP_DURATION).max(0.0);
                let breath_y = background::hermite(sprites::BIRD_SLEEP_POS_Y, bt);
                let breath_sx = background::hermite(sprites::BIRD_SLEEP_SCALE_X, bt);
                let breath_sy = background::hermite(sprites::BIRD_SLEEP_SCALE_Y, bt);
                if is_gpu_rendered {
                    deferred_birds.push(DeferredBird {
                        name: sprite.name.clone(),
                        wx: sprite.world_pos.x,
                        wy: sprite.world_pos.y + breath_y,
                        sx: sprite.scale.0,
                        sy: sprite.scale.1,
                        rot: sprite.rotation,
                        bsx: breath_sx,
                        bsy: breath_sy,
                    });
                } else {
                    compounds::draw_bird_face(
                        &DrawCtx {
                            painter: &painter,
                            camera: &self.camera,
                            canvas_center,
                            canvas_rect: rect,
                            tex_cache: &self.tex_cache,
                        },
                        &sprite.name,
                        CompoundTransform {
                            world_x: sprite.world_pos.x,
                            world_y: sprite.world_pos.y + breath_y,
                            scale_x: sprite.scale.0,
                            scale_y: sprite.scale.1,
                            rotation_z: sprite.rotation,
                        },
                        breath_sx,
                        breath_sy,
                    );
                }
            }

            if skip_root && is_sel {
                let center = self.camera.world_to_screen(
                    Vec2 {
                        x: sprite.world_pos.x,
                        y: sprite.world_pos.y,
                    },
                    canvas_center,
                );
                let hw = sprite.half_size.0 * self.camera.zoom;
                let hh = sprite.half_size.1 * self.camera.zoom;
                let sel_rect = egui::Rect::from_center_size(
                    center,
                    egui::vec2(hw.max(4.0) * 2.0, hh.max(4.0) * 2.0),
                );
                painter.rect_stroke(
                    sel_rect.expand(2.0),
                    2.0,
                    egui::Stroke::new(2.0, egui::Color32::YELLOW),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Emit GPU sprite callbacks in Z order, batching consecutive same-type
        // draws into one callback to minimise render state resets while keeping
        // correct Z interleaving between opaque (Props) and transparent sprites.
        {
            let mut pending_opaque: Vec<opaque_shader::OpaqueBatchDraw> = Vec::new();
            let mut pending_transparent: Vec<sprite_shader::SpriteBatchDraw> = Vec::new();
            let props_tint = assets::props_tint_color(self.bg_theme);

            for draw in gpu_draws {
                match draw {
                    GpuDraw::Opaque(d) => {
                        // Flush pending transparent batch before starting an opaque run
                        if !pending_transparent.is_empty()
                            && let Some(resources) = &self.sprite_resources
                        {
                            painter.add(sprite_shader::make_sprite_batch_callback(
                                rect,
                                resources.clone(),
                                std::mem::take(&mut pending_transparent),
                            ));
                        }
                        pending_opaque.push(d);
                    }
                    GpuDraw::Transparent(d) => {
                        // Flush pending opaque batch before starting a transparent run
                        if !pending_opaque.is_empty()
                            && let (Some(resources), Some(batch)) =
                                (&self.opaque_resources, &self.opaque_batch)
                        {
                            painter.add(opaque_shader::make_opaque_batch_callback(
                                rect,
                                resources.clone(),
                                batch.clone(),
                                opaque_shader::OpaqueBatchParams {
                                    screen_w: rect.width(),
                                    screen_h: rect.height(),
                                    zoom: self.camera.zoom,
                                    tint_color: props_tint,
                                },
                                std::mem::take(&mut pending_opaque),
                            ));
                        }
                        pending_transparent.push(d);
                    }
                }
            }

            // Flush remaining draws
            if !pending_opaque.is_empty()
                && let (Some(resources), Some(batch)) = (&self.opaque_resources, &self.opaque_batch)
            {
                painter.add(opaque_shader::make_opaque_batch_callback(
                    rect,
                    resources.clone(),
                    batch.clone(),
                    opaque_shader::OpaqueBatchParams {
                        screen_w: rect.width(),
                        screen_h: rect.height(),
                        zoom: self.camera.zoom,
                        tint_color: props_tint,
                    },
                    pending_opaque,
                ));
            }
            if !pending_transparent.is_empty()
                && let Some(resources) = &self.sprite_resources
            {
                painter.add(sprite_shader::make_sprite_batch_callback(
                    rect,
                    resources.clone(),
                    pending_transparent,
                ));
            }
        }

        // Deferred bird faces: draw after GPU batch so faces appear on top of bodies
        for bird in &deferred_birds {
            compounds::draw_bird_face(
                &DrawCtx {
                    painter: &painter,
                    camera: &self.camera,
                    canvas_center,
                    canvas_rect: rect,
                    tex_cache: &self.tex_cache,
                },
                &bird.name,
                CompoundTransform {
                    world_x: bird.wx,
                    world_y: bird.wy,
                    scale_x: bird.sx,
                    scale_y: bird.sy,
                    rotation_z: bird.rot,
                },
                bird.bsx,
                bird.bsy,
            );
        }

        // Draw fan particles (cloud puffs, renderOrder=12)
        // UV: Particles_Sheet_01.png 8×8 grid, col=3, row=0 from top
        let fan_tex = self.tex_cache.get(GLOW_ATLAS);
        for p in &self.fan_particles {
            let t_frac = p.age / p.lifetime;
            // Size envelope from Unity SizeModule curve
            let size_scale = if t_frac < 0.136 {
                t_frac / 0.136 * 0.32
            } else if t_frac < 0.845 {
                0.32 + (t_frac - 0.136) / (0.845 - 0.136) * 0.18
            } else if t_frac < 0.913 {
                0.50 + (t_frac - 0.845) / (0.913 - 0.845) * 0.20
            } else {
                0.70 * (1.0 - (t_frac - 0.913) / (1.0 - 0.913))
            } * 0.2; // SizeModule scalar
            let sz = p.start_size * size_scale;
            let center = self
                .camera
                .world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
            if !rect.expand(30.0).contains(center) {
                continue;
            }
            // Alpha: full opacity, then fade to 0 in the last 15%
            let alpha = if t_frac > 0.85 {
                ((1.0 - t_frac) / 0.15 * 255.0) as u8
            } else {
                255
            };
            let hw = sz * self.camera.zoom;
            let hh = hw;
            if let Some(tex_id) = fan_tex {
                // UV: col=3, row=0 from top → in 8×8 grid: u=3/8, v=7/8 (Unity), v_flip = 1/8..2/8 (egui)
                let u0 = 3.0 / 8.0;
                let u1 = 4.0 / 8.0;
                let v0 = 0.0 / 8.0; // egui: V=0 at top → row 0 from top = 0/8
                let v1 = 1.0 / 8.0;
                let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                let cos_r = p.rot.cos();
                let sin_r = p.rot.sin();
                let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                    egui::pos2(
                        center.x + dx * cos_r + dy * sin_r,
                        center.y - dx * sin_r + dy * cos_r,
                    )
                };
                let tl = rot(-hw, -hh);
                let tr = rot(hw, -hh);
                let br = rot(hw, hh);
                let bl = rot(-hw, hh);
                let mut mesh = egui::Mesh::with_texture(tex_id);
                let i = mesh.vertices.len() as u32;
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: tl,
                    uv: egui::pos2(u0, v0),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: tr,
                    uv: egui::pos2(u1, v0),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: br,
                    uv: egui::pos2(u1, v1),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: bl,
                    uv: egui::pos2(u0, v1),
                    color,
                });
                mesh.indices
                    .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                painter.add(egui::Shape::mesh(mesh));
            } else {
                // Fallback: colored circle
                let puff_color = egui::Color32::from_rgba_unmultiplied(220, 230, 245, alpha);
                let puff_rect =
                    egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));
                painter.rect_filled(puff_rect, hw, puff_color);
            }
        }

        // ── Front-ground + foreground (eff_z < 0): waves/foam/dummy + foreground, after sprites ──
        if self.show_bg
            && let Some(theme_name) = self.bg_theme
        {
            let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
                (Some(r), Some(d), Some(q)) => Some(background::BgGpuState {
                    resources: r.clone(),
                    atlas_cache: &mut self.bg_atlas_cache,
                    device: d,
                    queue: q,
                    slot_counter: &mut self.bg_slot_counter,
                }),
                _ => None,
            };
            if let Some(ref cache) = self.bg_layer_cache {
                background::draw_bg_layers(
                    &DrawCtx {
                        painter: &painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    theme_name,
                    self.time,
                    (f32::NEG_INFINITY, 0.0), // waves/foam/dummy + foreground
                    cache,
                    gpu.as_mut(),
                );
            }
        }

        // Draw wind leaf particles (renderOrder=25, on top of foreground)
        if let Some(leaf_tex) = self.tex_cache.get(GLOW_ATLAS) {
            for p in &self.wind_particles {
                let t_frac = p.age / p.lifetime;
                let alpha = if t_frac < 0.056 {
                    t_frac / 0.056
                } else if t_frac > 0.85 {
                    (1.0 - t_frac) / 0.15
                } else {
                    1.0
                };
                let sz = p.size * self.camera.zoom;
                if sz < 0.5 {
                    continue;
                }
                let center = self
                    .camera
                    .world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
                if !rect.expand(20.0).contains(center) {
                    continue;
                }

                // Leaf UV from 16×16 grid: column = LEAF_COLS[leaf_col], row UV = LEAF_ROW_UV
                let col = LEAF_COLS[p.leaf_col as usize] as f32;
                let u0 = col / LEAF_TILES;
                let u1 = (col + 1.0) / LEAF_TILES;
                // egui texture V=0 at top; Unity V=0 at bottom
                // LEAF_ROW_UV = 13/16 (bottom-up), convert: v_top = 1 - (13/16 + 1/16) = 2/16
                let v0 = 1.0 - LEAF_ROW_UV - 1.0 / LEAF_TILES;
                let v1 = 1.0 - LEAF_ROW_UV;

                let hw = sz * 0.5;
                let hh = sz * 0.5;
                let cos_r = p.rot.cos();
                let sin_r = p.rot.sin();
                let rot_pt = |dx: f32, dy: f32| -> egui::Pos2 {
                    egui::pos2(
                        center.x + dx * cos_r + dy * sin_r,
                        center.y - dx * sin_r + dy * cos_r,
                    )
                };

                let a = (alpha * 255.0) as u8;
                let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, a);

                let mut mesh = egui::Mesh::with_texture(leaf_tex);
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: rot_pt(-hw, -hh),
                    uv: egui::pos2(u0, v0),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: rot_pt(hw, -hh),
                    uv: egui::pos2(u1, v0),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: rot_pt(hw, hh),
                    uv: egui::pos2(u1, v1),
                    color,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: rot_pt(-hw, hh),
                    uv: egui::pos2(u0, v1),
                    color,
                });
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                painter.add(egui::Shape::mesh(mesh));
            }
        }

        // Grid (drawn on top of all scene content)
        if self.show_grid {
            self.draw_grid(&painter, rect, canvas_center);
        }

        // Origin axes
        let origin = self
            .camera
            .world_to_screen(Vec2 { x: 0.0, y: 0.0 }, canvas_center);
        if rect.contains(origin) {
            let axis_len = 30.0;
            painter.line_segment(
                [origin, egui::pos2(origin.x + axis_len, origin.y)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
            );
            painter.line_segment(
                [origin, egui::pos2(origin.x, origin.y - axis_len)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 255, 80)),
            );
        }

        // Physics ground line
        if self.show_ground {
            const PHYSICS_GROUND_Y: f32 = -6.599;
            let ground_left = self.camera.world_to_screen(
                Vec2 {
                    x: -1000.0,
                    y: PHYSICS_GROUND_Y,
                },
                canvas_center,
            );
            let ground_right = self.camera.world_to_screen(
                Vec2 {
                    x: 1000.0,
                    y: PHYSICS_GROUND_Y,
                },
                canvas_center,
            );
            let left_x = ground_left.x.max(rect.left());
            let right_x = ground_right.x.min(rect.right());
            if left_x < right_x {
                let gy = ground_left.y;
                painter.line_segment(
                    [egui::pos2(left_x, gy), egui::pos2(right_x, gy)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 165, 0, 180)),
                );
                painter.text(
                    egui::pos2(rect.left() + 8.0, gy - 14.0),
                    egui::Align2::LEFT_TOP,
                    format!("Y = {:.3}", PHYSICS_GROUND_Y),
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(255, 165, 0, 200),
                );
                painter.text(
                    egui::pos2(right_x - 8.0, gy - 14.0),
                    egui::Align2::RIGHT_TOP,
                    tr.get("menu_physics_ground"),
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(255, 165, 0, 200),
                );
            }
        }

        // Zoom + theme info
        let theme_label = self
            .bg_theme
            .map(|s| s.to_owned())
            .unwrap_or_else(|| tr.get("hud_unknown_theme"));
        painter.text(
            rect.left_top() + egui::vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{}: {:.1}x  {}: {}",
                tr.get("hud_zoom"),
                self.camera.zoom,
                tr.get("hud_theme"),
                theme_label
            ),
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(150, 150, 150),
        );

        // Request continuous repaint for animations
        ui.ctx().request_repaint();

        // Lazy-load atlas textures (only attempt once per atlas)
        {
            // Sprite atlases (sprites/ and props/ subdirs)
            for atlas in ATLAS_FILES {
                if self.tex_cache.get(atlas).is_none() {
                    let sprite_key = format!("sprites/{}", atlas);
                    let props_key = format!("props/{}", atlas);
                    // Props_Generic_Sheet is rendered via wgpu opaque shader (GPU pipeline),
                    // so skip egui texture loading for it entirely.
                    if atlas == &"Props_Generic_Sheet_01.png" {
                        continue;
                    } else if self
                        .tex_cache
                        .load_texture(ui.ctx(), &sprite_key, atlas)
                        .is_none()
                    {
                        self.tex_cache.load_texture(ui.ctx(), &props_key, atlas);
                    }
                }
            }
            // Background atlases (bg/ subdir)
            for atlas in crate::bg_data::bg_atlas_files() {
                if self.tex_cache.get(atlas).is_none() {
                    self.tex_cache
                        .load_texture(ui.ctx(), &format!("bg/{}", atlas), atlas);
                }
            }
            // Sky textures (sky/ subdir)
            for sky in crate::bg_data::sky_texture_files() {
                if self.tex_cache.get(sky).is_none() {
                    self.tex_cache
                        .load_texture(ui.ctx(), &format!("sky/{}", sky), sky);
                }
            }
            // Ground fill textures (ground/ subdir) — loaded with repeat wrap
            for td in &self.terrain_data {
                if let Some(ref tex_name) = td.fill_texture
                    && self.tex_cache.get(tex_name).is_none()
                {
                    self.tex_cache.load_texture_repeat(
                        ui.ctx(),
                        &format!("ground/{}", tex_name),
                        tex_name,
                    );
                }
                // Splat textures for CPU-textured edge fallback
                if let Some(ref tex_name) = td.edge_splat0
                    && self.tex_cache.get(tex_name).is_none()
                {
                    self.tex_cache.load_texture_repeat(
                        ui.ctx(),
                        &format!("ground/{}", tex_name),
                        tex_name,
                    );
                }
                if let Some(ref tex_name) = td.edge_splat1
                    && self.tex_cache.get(tex_name).is_none()
                {
                    self.tex_cache.load_texture_repeat(
                        ui.ctx(),
                        &format!("ground/{}", tex_name),
                        tex_name,
                    );
                }
            }
            // Goal flag texture (props/ subdir) — repeat wrap + flip V for UV scroll
            if self.tex_cache.get(GOAL_FLAG_TEXTURE).is_none() {
                self.tex_cache.load_texture_repeat_flipv(
                    ui.ctx(),
                    &format!("props/{}", GOAL_FLAG_TEXTURE),
                    GOAL_FLAG_TEXTURE,
                );
            }
            // Glow/starburst particle atlas
            if self.tex_cache.get(GLOW_ATLAS).is_none() {
                self.tex_cache.load_texture(
                    ui.ctx(),
                    &format!("particles/{}", GLOW_ATLAS),
                    GLOW_ATLAS,
                );
            }
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: egui::Rect, canvas_center: egui::Vec2) {
        // Adaptive grid: choose spacing so pixel distance stays in 30..120 range
        let target_px = 60.0;
        let base = target_px / self.camera.zoom;
        // Snap to nice values: 0.5, 1, 2, 5, 10, 20, 50, 100...
        let nice = [
            0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
        ];
        let grid_step = nice
            .iter()
            .copied()
            .min_by(|a, b| (a - base).abs().partial_cmp(&(b - base).abs()).unwrap())
            .unwrap_or(5.0);
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25);

        let tl = self.camera.screen_to_world(rect.left_top(), canvas_center);
        let br = self
            .camera
            .screen_to_world(rect.right_bottom(), canvas_center);

        let min_x = tl.x.min(br.x);
        let max_x = tl.x.max(br.x);
        let min_y = tl.y.min(br.y);
        let max_y = tl.y.max(br.y);

        let start_x = (min_x / grid_step).floor() as i32;
        let end_x = (max_x / grid_step).ceil() as i32;
        for ix in start_x..=end_x {
            let wx = ix as f32 * grid_step;
            let top = self
                .camera
                .world_to_screen(Vec2 { x: wx, y: max_y }, canvas_center);
            let bot = self
                .camera
                .world_to_screen(Vec2 { x: wx, y: min_y }, canvas_center);
            painter.line_segment([top, bot], egui::Stroke::new(0.5, color));
        }

        let start_y = (min_y / grid_step).floor() as i32;
        let end_y = (max_y / grid_step).ceil() as i32;
        for iy in start_y..=end_y {
            let wy = iy as f32 * grid_step;
            let left = self
                .camera
                .world_to_screen(Vec2 { x: min_x, y: wy }, canvas_center);
            let right = self
                .camera
                .world_to_screen(Vec2 { x: max_x, y: wy }, canvas_center);
            painter.line_segment([left, right], egui::Stroke::new(0.5, color));
        }
    }
}

/// Simple pseudo-random [0, 1) from u32 seed.
fn pseudo_random(seed: u32) -> f32 {
    let n = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((n >> 16) & 0x7fff) as f32 / 32768.0
}

/// Spawn a wind leaf particle in the given area.
fn spawn_wind_particle(area: &WindAreaDef, particles: &mut Vec<WindParticle>) {
    let seed = particles.len() as u32;
    let x = area.center_x - area.half_w + pseudo_random(seed.wrapping_mul(3)) * area.half_w * 0.3;
    let y = area.center_y - area.half_h
        + pseudo_random(seed.wrapping_mul(7).wrapping_add(1)) * area.half_h * 2.0;
    let size = 0.4 + pseudo_random(seed.wrapping_mul(11).wrapping_add(5)) * 0.3;
    let speed = 6.0 + pseudo_random(seed.wrapping_mul(13).wrapping_add(9)) * 3.0;
    let angle = -0.15 + pseudo_random(seed.wrapping_mul(17).wrapping_add(3)) * 0.3;
    let leaf_col = (pseudo_random(seed.wrapping_mul(31).wrapping_add(13)) * 3.0) as u8;
    particles.push(WindParticle {
        x,
        y,
        vx: speed * angle.cos(),
        vy: speed * angle.sin() * 0.3,
        age: 0.0,
        lifetime: 3.5 + pseudo_random(seed.wrapping_mul(19).wrapping_add(7)) * 2.0,
        rot: 0.0,
        rot_speed: (0.17 + pseudo_random(seed.wrapping_mul(23)) * 2.97)
            * if seed.is_multiple_of(2) { 1.0 } else { -1.0 },
        y_phase: pseudo_random(seed.wrapping_mul(29).wrapping_add(11)),
        size,
        leaf_col: leaf_col.min(2),
    });
}

/// Load a PNG from embedded assets and return raw RGBA pixels + dimensions.
fn load_raw_rgba(asset_key: &str) -> Option<(Vec<u8>, u32, u32)> {
    let data = crate::assets::read_asset(asset_key)?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    // Flip vertically: image crate stores top-to-bottom, but glTexImage2D places
    // row 0 at V=0 (bottom). Flipping matches Three.js flipY=true / Unity convention
    // so that V=1 (outer/surface) maps to the top of the image (green grass).
    let flipped = image::imageops::flip_vertical(&img);
    let w = flipped.width();
    let h = flipped.height();
    Some((flipped.into_raw(), w, h))
}

/// Compute the world position of an object by walking up the parent chain.
/// Binary stores world-space positions (LevelLoader.cs uses transform.position, not localPosition).
fn compute_world_position(level: &LevelData, idx: ObjectIndex) -> Vec3 {
    level.objects[idx].position()
}

/// Search the flat object arena for a BackgroundObject with override data.
fn find_bg_override_text(objects: &[LevelObject]) -> Option<String> {
    for obj in objects {
        if let LevelObject::Prefab(inst) = obj
            && inst.name.contains("Background")
            && let Some(ref od) = inst.override_data
            && od.raw_text.contains("Component UnityEngine.Transform")
        {
            return Some(od.raw_text.clone());
        }
    }
    None
}
