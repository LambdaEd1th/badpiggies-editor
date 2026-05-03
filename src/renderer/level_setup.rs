//! Level setup: loading level data into the renderer, building draw data and GPU resources.

use std::sync::Arc;

use crate::assets;
use crate::types::{LevelData, LevelObject, ObjectIndex, Vec2, Vec3};

use super::background;
use super::bg_shader;
use super::clouds::{CLOUD_CONFIGS, CloudInstance};
use super::compounds;
use super::dark_overlay::{construction_grid_start_light, parse_dark_level_data};
use super::dark_shader;
use super::edge_shader;
use super::fill_shader;
use super::grid;
use super::opaque_shader;
use super::particles::{FanEmitter, FanState, WindAreaDef, pseudo_random, spawn_wind_particle};
use super::sprite_shader;
use super::sprites;
use super::terrain;
use super::{Camera, LevelRenderer};

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
        let dark_resources = render_state.map(|rs| {
            Arc::new(dark_shader::init_dark_resources(
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
            clicked_with_cmd: false,
            mouse_world: None,
            time: 0.0,
            dragging: None,
            node_dragging: None,
            drag_result: None,
            node_drag_result: None,
            node_edit_action: None,
            box_select_result: None,
            context_action: None,
            context_selected_object: None,
            context_menu_world_pos: None,
            context_menu_indices: Vec::new(),
            context_menu_node: None,
            box_select_start: None,
            draw_terrain_result: None,
            draw_terrain_points: Vec::new(),
            draw_terrain_active: false,
            bounds_dragging: None,
            bounds_drag_result: None,
            bounds_hovered_handle: None,
            pending_drag_offset: None,
            show_bg: true,
            show_ground: false,
            show_grid: true,
            dark_level: false,
            show_dark_overlay: true,
            camera_limits: None,
            show_level_bounds: false,
            show_terrain_tris: false,
            lit_area_polygons: Vec::new(),
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
            dark_resources,
            dark_gpu_meshes: None,
            hovered_terrain_node: None,
            terrain_scratch_mesh: egui::Mesh::default(),
            clicked_empty: false,
            dark_overlay_mesh: None,
            dark_overlay_light: None,
            dark_overlay_ring: None,
            dark_overlay_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_live_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_stable_frames: 0,
        }
    }

    /// Create a new renderer that shares GPU pipeline resources with this one.
    /// Used when opening a new tab — pipeline/device/queue are Arc-shared,
    /// while per-level caches start empty.
    pub fn clone_for_new_tab(&self) -> Self {
        Self {
            camera: Camera::default(),
            world_positions: Vec::new(),
            terrain_data: Vec::new(),
            sprite_data: Vec::new(),
            bg_theme: None,
            bg_override_text: None,
            bg_layer_cache: None,
            construction_grid: None,
            show_grid_overlay: self.show_grid_overlay,
            level_key: String::new(),
            tex_cache: assets::TextureCache::new(),
            asset_base: self.asset_base.clone(),
            panning: false,
            clicked_object: None,
            clicked_with_cmd: false,
            mouse_world: None,
            time: 0.0,
            dragging: None,
            node_dragging: None,
            drag_result: None,
            node_drag_result: None,
            node_edit_action: None,
            box_select_result: None,
            context_action: None,
            context_selected_object: None,
            context_menu_world_pos: None,
            context_menu_indices: Vec::new(),
            context_menu_node: None,
            box_select_start: None,
            draw_terrain_result: None,
            draw_terrain_points: Vec::new(),
            draw_terrain_active: false,
            bounds_dragging: None,
            bounds_drag_result: None,
            bounds_hovered_handle: None,
            pending_drag_offset: None,
            show_bg: self.show_bg,
            show_ground: self.show_ground,
            show_grid: self.show_grid,
            dark_level: false,
            show_dark_overlay: true,
            camera_limits: None,
            show_level_bounds: self.show_level_bounds,
            show_terrain_tris: self.show_terrain_tris,
            lit_area_polygons: Vec::new(),
            fan_emitters: Vec::new(),
            fan_particles: Vec::new(),
            wind_areas: Vec::new(),
            wind_particles: Vec::new(),
            wind_spawn_accum: Vec::new(),
            zzz_particles: Vec::new(),
            zzz_emit_accum: Vec::new(),
            bird_positions: Vec::new(),
            cloud_instances: Vec::new(),
            wgpu_device: self.wgpu_device.clone(),
            wgpu_queue: self.wgpu_queue.clone(),
            edge_resources: self.edge_resources.clone(),
            edge_gpu_meshes: Arc::new(Vec::new()),
            edge_gpu_mesh_index: Vec::new(),
            bg_resources: self.bg_resources.clone(),
            bg_atlas_cache: bg_shader::BgAtlasCache::new(),
            bg_slot_counter: 0,
            opaque_resources: self.opaque_resources.clone(),
            opaque_atlas: None,
            opaque_batch: None,
            opaque_sprite_map: Vec::new(),
            sprite_resources: self.sprite_resources.clone(),
            sprite_atlas_cache: sprite_shader::SpriteAtlasCache::new(),
            sprite_slot_counter: 0,
            fill_resources: self.fill_resources.clone(),
            fill_texture_cache: fill_shader::FillTextureCache::new(),
            fill_gpu_meshes: Vec::new(),
            fill_slot_counter: 0,
            dark_resources: self.dark_resources.clone(),
            dark_gpu_meshes: None,
            hovered_terrain_node: None,
            terrain_scratch_mesh: egui::Mesh::default(),
            clicked_empty: false,
            dark_overlay_mesh: None,
            dark_overlay_light: None,
            dark_overlay_ring: None,
            dark_overlay_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_live_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_stable_frames: 0,
        }
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
        self.dark_level = false;
        self.lit_area_polygons.clear();
        self.dark_overlay_mesh = None;
        self.dark_overlay_light = None;
        self.dark_overlay_ring = None;
        self.dark_gpu_meshes = None;
        self.dark_overlay_key = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        self.dark_overlay_live_key = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        self.dark_overlay_stable_frames = 0;

        // Collect all object names for BG theme detection
        let names: Vec<String> = level
            .objects
            .iter()
            .map(|o| match o {
                LevelObject::Prefab(p) => p.name.clone(),
                LevelObject::Parent(p) => p.name.clone(),
            })
            .collect();
        self.bg_theme = assets::detect_bg_theme(&self.level_key, &names);

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
            for (_fi, td) in self.terrain_data.iter().enumerate() {
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
                let cz = pos.z;
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
                    seed = seed.wrapping_mul(19) ^ (i as u32 + 3);
                    let z = cz + (seed % 10000) as f32 / 10000.0 * config.far_plane;
                    self.cloud_instances.push(CloudInstance {
                        x,
                        y,
                        z,
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

        // Parse dark level flag and LitArea polygons
        parse_dark_level_data(level, &mut self.dark_level, &mut self.lit_area_polygons);
        if self.dark_level
            && let Some(ref construction_grid) = self.construction_grid
        {
            self.lit_area_polygons
                .push(construction_grid_start_light(construction_grid));
        }

        // Parse camera limits from LevelManager
        self.camera_limits = parse_camera_limits(level);

        self.dark_gpu_meshes = self.wgpu_device.as_ref().and_then(|device| {
            if self.lit_area_polygons.is_empty() {
                None
            } else {
                Some(Arc::new(dark_shader::build_dark_gpu_meshes(
                    device,
                    self.lit_area_polygons.iter().map(|polygon| {
                        let border = if polygon.border_vertices.len() >= 3 {
                            polygon.border_vertices.as_slice()
                        } else {
                            polygon.vertices.as_slice()
                        };
                        (border, polygon.vertices.as_slice())
                    }),
                )))
            }
        });

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
}

/// Load a PNG from embedded assets and return raw RGBA pixels + dimensions.
pub(super) fn load_raw_rgba(asset_key: &str) -> Option<(Vec<u8>, u32, u32)> {
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
///
/// Accepts both `Component UnityEngine.Transform` overrides (EP1-5 style)
/// and `PositionSerializer` / `childLocalPositions` (EP6 style).
fn find_bg_override_text(objects: &[LevelObject]) -> Option<String> {
    for obj in objects {
        if let LevelObject::Prefab(inst) = obj
            && inst.name.contains("Background")
            && let Some(ref od) = inst.override_data
            && (od.raw_text.contains("Component UnityEngine.Transform")
                || od.raw_text.contains("PositionSerializer"))
        {
            return Some(od.raw_text.clone());
        }
    }
    None
}

/// Parse `m_cameraLimits` from LevelManager override data.
/// Returns `[topLeft.x, topLeft.y, size.x, size.y]` or `None` if not overridden.
pub(super) fn parse_camera_limits(level: &LevelData) -> Option<[f32; 4]> {
    for obj in &level.objects {
        if let LevelObject::Prefab(p) = obj
            && p.name == "LevelManager"
            && let Some(ref od) = p.override_data
            && let Some(pos) = od.raw_text.find("m_cameraLimits")
        {
            let after = &od.raw_text[pos..];
            // Parse topLeft x, y and size x, y
            // Format: "Float x = V" / "Float y = V" in order: tl.x, tl.y, sz.x, sz.y
            let mut vals = [0f32; 4];
            let mut search = after;
            for v in vals.iter_mut() {
                // Find whichever of "Float x = " or "Float y = " comes first
                let fx = search.find("Float x = ");
                let fy = search.find("Float y = ");
                let fp = match (fx, fy) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                if let Some(fp) = fp {
                    let eq = &search[fp..];
                    if let Some(eq_pos) = eq.find("= ") {
                        let num_start = &eq[eq_pos + 2..];
                        let end = num_start
                            .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                            .unwrap_or(num_start.len());
                        if let Ok(val) = num_start[..end].parse::<f32>() {
                            *v = val;
                        }
                        search = &num_start[end..];
                    }
                }
            }
            // Only return if size is non-zero
            if vals[2] > 0.0 && vals[3] > 0.0 {
                return Some(vals);
            }
        }
    }
    None
}
