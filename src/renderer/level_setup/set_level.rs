//! Implementation of `LevelRenderer::set_level` (heavy load + GPU upload).

use std::sync::Arc;

use crate::data::assets;
use crate::domain::types::{LevelData, LevelObject, Vec2};

use super::super::LevelRenderer;
use super::super::background;
use super::super::clouds::{CloudInstance, cloud_config};
use super::super::compounds;
use super::super::dark_overlay::{construction_grid_start_light, parse_dark_level_data};
use super::super::edge_shader;
use super::super::fill_shader;
use super::super::grid;
use super::super::opaque_shader;
use super::super::particles::{
    AttachedEffectEmitter, FanEmitter, FanState, build_wind_area_def,
    attached_effect_kind_for_sprite_name, pseudo_random, wind_area_particle_system_count,
};
use super::super::sprites;
use super::super::terrain;
use super::super::PreviewPlaybackState;
use super::{compute_world_position, find_bg_override_text, load_raw_rgba, parse_camera_limits};

impl LevelRenderer {
    pub fn set_level(&mut self, level: &LevelData) {
        // Drop old GPU resources (wgpu resources are reference-counted)
        self.edge_gpu_meshes = Arc::new(Vec::new());
        self.opaque_batch = None;
        self.opaque_sprite_map.clear();
        self.pending_drag_offset = None;
        self.pending_transform_preview = None;

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
        self.attached_effect_emitters.clear();
        self.attached_effect_particles.clear();
        self.cloud_instances.clear();
        self.dark_level = false;
        self.lit_area_polygons.clear();
        self.dark_overlay_mesh = None;
        self.dark_overlay_light = None;
        self.dark_overlay_ring = None;
        self.dark_overlay_key = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        self.dark_overlay_live_key = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        self.dark_overlay_stable_frames = 0;

        // Collect all object names for BG theme detection
        self.bg_override_text = find_bg_override_text(&level.objects);

        let names: Vec<String> = level
            .objects
            .iter()
            .map(|o| match o {
                LevelObject::Prefab(p) => p.name.clone(),
                LevelObject::Parent(p) => p.name.clone(),
            })
            .collect();
        self.bg_theme =
            assets::detect_bg_theme(&self.level_key, &names, self.bg_override_text.as_deref());

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
                    let resolved_name = crate::domain::level::refs::get_prefab_override(
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
                let Some(ctrl) = td.edge_ctrl_pixels.as_ref() else {
                    continue;
                };
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
                    if sprite.is_terrain {
                        continue;
                    }
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
                    target_force: ovr.target_force.unwrap_or(115.0),
                    emitting: init_state == FanState::SpinUp,
                    angle: 0.0,
                    spin_down_start_force: 0.0,
                    spin_down_angle_left: 0.0,
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
                self.wind_areas.push(build_wind_area_def(
                    i,
                    sprite.world_pos.x,
                    sprite.world_pos.y,
                    sprite.world_pos.z,
                    sprite.rotation,
                    sprite.scale.0,
                    sprite.scale.1,
                    sprite.override_text.as_deref(),
                ));
            }
            // Collect Bird positions for Zzz particles
            if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
                self.bird_positions.push(Vec2 {
                    x: sprite.world_pos.x,
                    y: sprite.world_pos.y,
                });
            }
            if let Some(kind) = attached_effect_kind_for_sprite_name(&sprite.name) {
                let system_count = super::super::particles::attached_effect_systems(kind).len();
                self.attached_effect_emitters.push(AttachedEffectEmitter {
                    world_x: sprite.world_pos.x,
                    world_y: sprite.world_pos.y,
                    rot: sprite.rotation,
                    kind,
                    system_time: vec![0.0; system_count],
                    spawn_accum: vec![0.0; system_count],
                });
            }
        }
        self.wind_spawn_accum = vec![0.0; self.wind_areas.len() * wind_area_particle_system_count()];
        self.preview_playback_state = PreviewPlaybackState::Play;
        self.start_runtime_preview();

        self.zzz_emit_accum = vec![0.0; self.bird_positions.len()];

        // Spawn cloud instances from CloudSet level objects
        for (idx, obj) in level.objects.iter().enumerate() {
            let obj_name = match obj {
                LevelObject::Prefab(p) => &p.name,
                LevelObject::Parent(p) => &p.name,
            };
            if let Some(config) = cloud_config(obj_name) {
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
                        sprite_name: info.name.clone(),
                        atlas: info.atlas.clone(),
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

        // Fit camera to level bounds
        self.fit_to_level();
    }
}
