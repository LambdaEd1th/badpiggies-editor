//! Level setup: loading level data into the renderer, building draw data and GPU resources.

mod set_level;

use std::sync::Arc;

use crate::assets;
use crate::types::{LevelData, LevelObject, ObjectIndex, Vec2, Vec3};

use super::background;
use super::bg_shader;
use super::clouds::{CLOUD_CONFIGS, CloudInstance};
use super::compounds;
use super::dark_overlay::{construction_grid_start_light, parse_dark_level_data};
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
            suppress_context_menu_this_frame: false,
            box_select_start: None,
            draw_terrain_result: None,
            terrain_preset_shape: None,
            terrain_preset_drag_start: None,
            terrain_round_segments: 24,
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
            suppress_context_menu_this_frame: false,
            box_select_start: None,
            draw_terrain_result: None,
            terrain_preset_shape: None,
            terrain_preset_drag_start: None,
            terrain_round_segments: self.terrain_round_segments,
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


}

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
