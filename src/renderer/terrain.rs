//! Terrain rendering — fill mesh and shader-ready curve edge data.
//!
//! The terrain fill mesh is drawn as indexed triangles tinted with the fill color.

use eframe::egui;

use crate::data::assets;
use crate::domain::types::*;

use super::{LevelRenderer, edge_shader, fill_shader};
use edge_shader::EdgeVertex;

// Note: terrain rendering uses pure Z-depth sorting (matching Unity's orthographic
// camera), not discrete render-order layers. Decorative terrain typically has Z≈5
// (behind ground background Z≈2), collider terrain Z≈0 (in front).

/// A pre-built terrain mesh ready for painting.
pub struct TerrainDrawData {
    /// Fill mesh: screen-space triangles with vertex colors.
    pub fill_mesh: Option<egui::Mesh>,
    /// World Z position for back-to-front sorting (larger Z = farther from camera).
    pub world_z: f32,
    /// Whether this is a decorative (non-collider) terrain.
    pub decorative: bool,
    /// Fill texture name (e.g. "Ground_Rocks_Texture.png"), if resolved.
    pub fill_texture: Option<String>,
    /// Object index for selection matching.
    pub object_index: ObjectIndex,
    /// Curve mesh vertices in world space for selection outline.
    pub curve_world_verts: Vec<(f32, f32)>,
    /// Per-node texture index (0=grass/splat0, 1=outline/splat1).
    pub node_textures: Vec<usize>,
    /// Shader-ready edge vertices (world-space, with UV + gradient).
    pub edge_vertices: Vec<EdgeVertex>,
    /// Shader-ready edge indices (u16).
    pub edge_indices: Vec<u16>,
    /// Decoded control texture RGBA pixels (one pixel per node).
    pub edge_ctrl_pixels: Option<Vec<u8>>,
    /// Decoded control texture size in texels.
    pub edge_ctrl_width: u32,
    pub edge_ctrl_height: u32,
    /// splatParamsX = 1 / curveTextures[0].size.x
    pub edge_splat_params_x: f32,
    /// Splat0 (grass/surface) texture filename.
    pub edge_splat0: Option<String>,
    /// Splat1 (outline) texture filename.
    pub edge_splat1: Option<String>,
}

/// Build terrain draw data for a single prefab instance with terrain data.
pub fn build_terrain(
    prefab: &PrefabInstance,
    world_offset: Vec3,
    level_key: &str,
    object_index: ObjectIndex,
) -> TerrainDrawData {
    let td = match &prefab.terrain_data {
        Some(td) => td,
        None => {
            return TerrainDrawData {
                fill_mesh: None,
                world_z: world_offset.z,
                decorative: false,
                fill_texture: None,
                object_index,
                curve_world_verts: Vec::new(),
                node_textures: Vec::new(),
                edge_vertices: Vec::new(),
                edge_indices: Vec::new(),
                edge_ctrl_pixels: None,
                edge_ctrl_width: 0,
                edge_ctrl_height: 0,
                edge_splat_params_x: 10.0,
                edge_splat0: None,
                edge_splat1: None,
            };
        }
    };

    let decorative = !td.has_collider;

    // Resolve fill texture: loader-derived level refs first, then name-based fallback.
    // Each terrain object's texture is determined by its per-level loader
    // m_references entry, NOT by the background
    // theme.  Different terrain prefab names (e.g. MM_sand, MM_rock,
    // Dark_MM_rock) map to different textures within the same level.
    let fill_texture = crate::domain::level::refs::get_level_ref(level_key, td.fill_texture_index)
        .map(|s| s.to_string())
        .or_else(|| assets::get_terrain_fill_texture(&prefab.name).map(|s| s.to_string()));

    let fill_mesh = build_fill_mesh(td, world_offset);

    // Build shader-ready edge data
    let (edge_vertices, edge_indices) = build_edge_shader_data(td, world_offset);
    let (edge_ctrl_pixels, edge_ctrl_width, edge_ctrl_height) = td
        .control_texture_data
        .as_ref()
        .and_then(|d| decode_control_png(d))
        .map_or((None, 0, 0), |(pixels, width, height)| {
            (Some(pixels), width, height)
        });
    let edge_splat_params_x = if !td.curve_textures.is_empty() && td.curve_textures[0].size.x > 0.0
    {
        1.0 / td.curve_textures[0].size.x
    } else {
        10.0
    };
    // Resolve splat texture names via level-refs then fallback
    let edge_splat0 = if !td.curve_textures.is_empty() {
        crate::domain::level::refs::get_level_ref(level_key, td.curve_textures[0].texture_index)
            .map(|s| s.to_string())
            .or_else(|| assets::get_terrain_splat0(&prefab.name).map(|s| s.to_string()))
    } else {
        assets::get_terrain_splat0(&prefab.name).map(|s| s.to_string())
    };
    let fallback_edge_splat1 =
        assets::get_terrain_splat1_for_level(level_key, &prefab.name).map(|s| s.to_string());
    // Resolve splat1 from Unity loader refs first; if the level reference is
    // missing, fall back to the terrain prefab's authored curve texture.
    let edge_splat1 = if td.curve_textures.len() > 1 {
        crate::domain::level::refs::get_level_ref(level_key, td.curve_textures[1].texture_index)
            .filter(|name| {
                crate::data::assets::read_pathname(&format!("Assets/Texture2D/{}", name)).is_some()
            })
            .map(|s| s.to_string())
            .or_else(|| fallback_edge_splat1.clone())
    } else {
        fallback_edge_splat1.clone()
    };

    // Extract curve outer vertices for selection outline
    let curve_world_verts: Vec<(f32, f32)> = td
        .curve_mesh
        .vertices
        .iter()
        .step_by(2) // curve mesh has 2 verts per cross-section; take outer only
        .map(|v| (v.x + world_offset.x, v.y + world_offset.y))
        .collect();

    // Derive per-node texture index from control texture pixels
    let node_count = td.curve_mesh.vertices.len() / 2;
    let node_textures: Vec<usize> = (0..node_count)
        .map(|i| usize::from(control_selects_splat1(edge_ctrl_pixels.as_deref(), i)))
        .collect();

    TerrainDrawData {
        fill_mesh,
        world_z: world_offset.z,
        decorative,
        fill_texture,
        object_index,
        curve_world_verts,
        node_textures,
        edge_vertices,
        edge_indices,
        edge_ctrl_pixels,
        edge_ctrl_width,
        edge_ctrl_height,
        edge_splat_params_x,
        edge_splat0,
        edge_splat1,
    }
}

/// Build the fill mesh as an egui::Mesh with the fill color applied to all vertices.
/// UV coordinates are tiled based on world position / TILE_SIZE.
fn build_fill_mesh(td: &TerrainData, offset: Vec3) -> Option<egui::Mesh> {
    let verts = &td.fill_mesh.vertices;
    let indices = &td.fill_mesh.indices;
    if verts.is_empty() || indices.len() < 3 {
        return None;
    }

    let [r, g, b, a] = td.fill_color.to_rgba8();
    let color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);

    let tile_size = 5.0_f32;
    let tile_off_x = td.fill_texture_tile_offset_x;
    let tile_off_y = td.fill_texture_tile_offset_y;

    let mut mesh = egui::Mesh::default();
    // Add vertices in world coordinates (will be transformed to screen later)
    // UV uses LOCAL-space position (matching Unity's GetPointFillUV), not world-space.
    // Y is negated: Unity UV (0,0)=bottom-left vs wgpu (0,0)=top-left.
    for v in verts {
        let wx = v.x + offset.x;
        let wy = v.y + offset.y;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(wx, wy),
            uv: egui::pos2(
                (v.x - tile_off_x) / tile_size,
                (tile_off_y - v.y) / tile_size,
            ),
            color,
        });
    }

    // Add indices
    for &idx in indices {
        let i = idx as usize;
        if idx < 0 || i >= verts.len() {
            log::error!(
                "build_fill_mesh: index {} out of bounds (verts={}), skipping triangle",
                idx,
                verts.len()
            );
            continue;
        }
        mesh.indices.push(idx as u32);
    }

    // Validate mesh
    if !mesh.is_valid() {
        log::error!(
            "build_fill_mesh: egui mesh INVALID! verts={} indices={}",
            mesh.vertices.len(),
            mesh.indices.len()
        );
    }

    Some(mesh)
}

/// Decode a raw PNG to get RGBA pixel data (without premultiplied alpha).
fn decode_control_png(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let decoder = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = decoder.decode().ok()?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Some((rgba.into_raw(), width, height))
}

fn control_selects_splat1(ctrl_pixels: Option<&[u8]>, node_idx: usize) -> bool {
    ctrl_pixels.and_then(|px| px.get(node_idx * 4 + 1).copied()) == Some(255)
}

/// Build shader-ready edge vertex and index arrays from curve mesh.
/// Returns (vertices, indices) in world space with UV and gradient attributes.
fn build_edge_shader_data(td: &TerrainData, offset: Vec3) -> (Vec<EdgeVertex>, Vec<u16>) {
    let verts = &td.curve_mesh.vertices;
    if verts.len() < 4 {
        return (Vec::new(), Vec::new());
    }

    let vert_count = verts.len();
    let mut out_verts = Vec::with_capacity(vert_count);
    let mut acc_dist: f32 = 0.0;

    for i in (0..vert_count).step_by(2) {
        let node_idx = i / 2;
        let outer = &verts[i];
        let inner = &verts[i + 1];

        // Accumulate distance along outer edge for horizontal UV tiling
        if node_idx > 0 {
            let prev = &verts[i - 2];
            let dx = outer.x - prev.x;
            let dy = outer.y - prev.y;
            acc_dist += (dx * dx + dy * dy).sqrt();
        }

        // Outer vertex: aColor = 1.0
        out_verts.push(EdgeVertex {
            pos: [outer.x + offset.x, outer.y + offset.y],
            uv: [acc_dist, node_idx as f32],
            color: 1.0,
        });
        // Inner vertex: aColor = 0.0
        out_verts.push(EdgeVertex {
            pos: [inner.x + offset.x, inner.y + offset.y],
            uv: [acc_dist, node_idx as f32],
            color: 0.0,
        });
    }

    // Use stored indices from binary (matches Unity runtime)
    let indices: Vec<u16> = td
        .curve_mesh
        .indices
        .iter()
        .map(|&idx| idx as u16)
        .collect();

    (out_verts, indices)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::control_selects_splat1;

    #[test]
    fn control_selector_matches_unity_green_channel_rule() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 128, 0, 255, 0, 0, 255, 255,
        ];

        assert!(!control_selects_splat1(Some(&pixels), 0));
        assert!(control_selects_splat1(Some(&pixels), 1));
        assert!(!control_selects_splat1(Some(&pixels), 2));
        assert!(!control_selects_splat1(Some(&pixels), 3));
        assert!(!control_selects_splat1(Some(&pixels), 99));
        assert!(!control_selects_splat1(None, 0));
    }
}

// ── Terrain draw pass (extracted from show()) ──

impl LevelRenderer {
    pub(in crate::renderer) fn draw_terrain_index(
        &mut self,
        terrain_index: usize,
        painter: &egui::Painter,
        _canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let Some(td) = self.terrain_data.get(terrain_index) else {
            return;
        };

        let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
        let cam_cx = self.camera.center.x - tdx;
        let cam_cy = self.camera.center.y - tdy;

        // Fill
        if let Some(ref fill_data) = td.fill_mesh
            && let (Some(resources), Some(device), Some(queue)) =
                (&self.fill_resources, &self.wgpu_device, &self.wgpu_queue)
            && let Some(Some(gpu_mesh)) = self.fill_gpu_meshes.get(terrain_index)
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
        }

        // Edge (right after fill)
        if let Some(gpu_idx) = self
            .edge_gpu_mesh_index
            .get(terrain_index)
            .copied()
            .flatten()
            && let Some(ref resources) = self.edge_resources
        {
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
    }
}
