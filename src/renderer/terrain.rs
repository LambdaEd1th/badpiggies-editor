//! Terrain rendering — fill mesh (colored triangles) and curve edge (vertex-colored quads).
//!
//! The terrain fill mesh is drawn as indexed triangles tinted with the fill color.
//! The edge mesh uses the fallback approach: per-quad vertex coloring based on
//! control texture data (grass vs outline selection).

use eframe::egui;

use crate::data::assets;
use crate::domain::types::*;

use super::{Camera, LevelRenderer, edge_shader, fill_shader};
use edge_shader::EdgeVertex;

/// Terrain edge default colors when no splat pixel data is available.
const DEFAULT_GRASS_COLOR: egui::Color32 = egui::Color32::from_rgb(0x70, 0xb0, 0x30);
const DEFAULT_OUTLINE_COLOR: egui::Color32 = egui::Color32::from_rgb(0x55, 0x44, 0x33);

// Note: terrain rendering uses pure Z-depth sorting (matching Unity's orthographic
// camera), not discrete render-order layers. Decorative terrain typically has Z≈5
// (behind ground background Z≈2), collider terrain Z≈0 (in front).

/// A pre-built terrain mesh ready for painting.
pub struct TerrainDrawData {
    /// Fill mesh: screen-space triangles with vertex colors.
    pub fill_mesh: Option<egui::Mesh>,
    /// Edge mesh: screen-space quads with vertex colors (fallback when GLSL unavailable).
    pub edge_mesh: Option<egui::Mesh>,
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
    /// Number of curve nodes.
    pub edge_node_count: usize,
    /// splatParamsX = 1 / curveTextures[0].size.x
    pub edge_splat_params_x: f32,
    /// Splat0 (grass/surface) texture filename.
    pub edge_splat0: Option<String>,
    /// Splat1 (outline) texture filename.
    pub edge_splat1: Option<String>,
    /// CPU-textured edge mesh for splat0 regions (fallback when no custom GLSL).
    pub edge_splat0_mesh: Option<egui::Mesh>,
    /// CPU-textured edge mesh for splat1 regions (fallback when no custom GLSL).
    pub edge_splat1_mesh: Option<egui::Mesh>,
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
                edge_mesh: None,
                world_z: world_offset.z,
                decorative: false,
                fill_texture: None,
                object_index,
                curve_world_verts: Vec::new(),
                node_textures: Vec::new(),
                edge_vertices: Vec::new(),
                edge_indices: Vec::new(),
                edge_ctrl_pixels: None,
                edge_node_count: 0,
                edge_splat_params_x: 10.0,
                edge_splat0: None,
                edge_splat1: None,
                edge_splat0_mesh: None,
                edge_splat1_mesh: None,
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
    let edge_mesh = build_edge_fallback(td, world_offset, &prefab.name);

    // Build shader-ready edge data
    let (edge_vertices, edge_indices) = build_edge_shader_data(td, world_offset);
    let edge_ctrl_pixels = td
        .control_texture_data
        .as_ref()
        .and_then(|d| decode_control_png(d));
    let edge_node_count = td.curve_mesh.vertices.len() / 2;
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
    // Most terrains should still honor loader refs, but the Maya cave / temple / dark
    // prefabs keep their prefab-authored Border splat1 even when level refs point at a
    // shared outline texture such as Ground_Rocks_Outline_Texture_06.
    let edge_splat1 = if assets::terrain_splat1_prefers_prefab_over_level_refs(&prefab.name) {
        fallback_edge_splat1.clone()
    } else if td.curve_textures.len() > 1 {
        crate::domain::level::refs::get_level_ref(level_key, td.curve_textures[1].texture_index)
            .filter(|name| crate::data::assets::read_asset(&format!("ground/{}", name)).is_some())
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
        .map(|i| {
            td.control_texture_data
                .as_ref()
                .and_then(|data| {
                    crate::domain::terrain_gen::decode_control_png_pixels(data).map(|px| {
                        let gi = i * 4 + 1; // green channel
                        if gi < px.len() && px[gi] > 128 { 1 } else { 0 }
                    })
                })
                .unwrap_or(0)
        })
        .collect();

    // Build CPU-textured edge meshes (fallback for when GLSL is unavailable)
    let (edge_splat0_mesh, edge_splat1_mesh) = build_edge_textured_meshes(
        &edge_vertices,
        &edge_indices,
        edge_ctrl_pixels.as_deref(),
        edge_node_count,
        edge_splat_params_x,
    );

    TerrainDrawData {
        fill_mesh,
        edge_mesh,
        world_z: world_offset.z,
        decorative,
        fill_texture,
        object_index,
        curve_world_verts,
        node_textures,
        edge_vertices,
        edge_indices,
        edge_ctrl_pixels,
        edge_node_count,
        edge_splat_params_x,
        edge_splat0,
        edge_splat1,
        edge_splat0_mesh,
        edge_splat1_mesh,
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

/// Build terrain edge using the vertex-color fallback approach.
/// Each quad strip segment gets a flat color based on control texture data.
fn build_edge_fallback(td: &TerrainData, offset: Vec3, name: &str) -> Option<egui::Mesh> {
    let verts = &td.curve_mesh.vertices;
    if verts.len() < 4 {
        return None;
    }

    let is_dark = assets::is_dark_terrain(name);

    // Determine grass/outline colors from terrain type
    let grass_color = if is_dark {
        egui::Color32::from_rgb(0x44, 0x88, 0x44)
    } else {
        DEFAULT_GRASS_COLOR
    };
    let outline_color = if is_dark {
        egui::Color32::from_rgb(0x33, 0x55, 0x22)
    } else {
        DEFAULT_OUTLINE_COLOR
    };

    // Decode control texture if available (raw PNG bytes in terrain data)
    let ctrl_pixels = td
        .control_texture_data
        .as_ref()
        .and_then(|data| decode_control_png(data));

    let mut mesh = egui::Mesh::default();
    let quad_count = (verts.len() - 2) / 2;

    for qi in 0..quad_count {
        let i = qi * 2;
        if i + 3 >= verts.len() {
            break;
        }

        let oa = &verts[i]; // outer A
        let ia = &verts[i + 1]; // inner A
        let ob = &verts[i + 2]; // outer B
        let ib = &verts[i + 3]; // inner B

        // Pick color based on control texture red channel
        let is_grass = ctrl_pixels
            .as_ref()
            .map(|px| {
                let node_idx = qi;
                node_idx * 4 < px.len() && px[node_idx * 4] > 128
            })
            .unwrap_or(false);

        let color = if is_grass { grass_color } else { outline_color };

        let base = mesh.vertices.len() as u32;
        let uv = egui::pos2(0.0, 0.0);

        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(oa.x + offset.x, oa.y + offset.y),
            uv,
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(ob.x + offset.x, ob.y + offset.y),
            uv,
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(ib.x + offset.x, ib.y + offset.y),
            uv,
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: egui::pos2(ia.x + offset.x, ia.y + offset.y),
            uv,
            color,
        });

        // Two triangles per quad
        mesh.indices.push(base);
        mesh.indices.push(base + 1);
        mesh.indices.push(base + 2);
        mesh.indices.push(base);
        mesh.indices.push(base + 2);
        mesh.indices.push(base + 3);
    }

    if mesh.vertices.is_empty() {
        None
    } else {
        Some(mesh)
    }
}

/// Decode a raw PNG to get RGBA pixel data (without premultiplied alpha).
fn decode_control_png(data: &[u8]) -> Option<Vec<u8>> {
    let decoder = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = decoder.decode().ok()?;
    let rgba = img.to_rgba8();
    Some(rgba.into_raw())
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

/// Build CPU-textured edge meshes split by control texture selection.
/// Returns (splat0_mesh, splat1_mesh) with proper UV for texture tiling.
/// This emulates the GLSL shader logic on CPU for when GL is unavailable.
fn build_edge_textured_meshes(
    edge_verts: &[EdgeVertex],
    edge_indices: &[u16],
    ctrl_pixels: Option<&[u8]>,
    _node_count: usize,
    splat_params_x: f32,
) -> (Option<egui::Mesh>, Option<egui::Mesh>) {
    if edge_verts.is_empty() || edge_indices.len() < 3 {
        return (None, None);
    }

    let mut mesh0 = egui::Mesh::default();
    let mut mesh1 = egui::Mesh::default();

    // Process triangles: each 3 consecutive indices form a triangle.
    // Determine which splat based on the node index of the first vertex.
    for tri in edge_indices.chunks_exact(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        if i0 >= edge_verts.len() || i1 >= edge_verts.len() || i2 >= edge_verts.len() {
            continue;
        }

        // Determine splat selection from the node index of the first vertex
        let node_idx = edge_verts[i0].uv[1] as usize;
        let use_splat1 = ctrl_pixels
            .map(|px| {
                // Green channel: pixels[node * 4 + 1]
                let gi = node_idx * 4 + 1;
                gi < px.len() && px[gi] > 128
            })
            .unwrap_or(false);

        let target = if use_splat1 { &mut mesh1 } else { &mut mesh0 };
        let base = target.vertices.len() as u32;

        for &vi in &[i0, i1, i2] {
            let ev = &edge_verts[vi];
            target.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(ev.pos[0], ev.pos[1]),
                // UV: horizontal = accumulated_distance * splatParamsX (tiling),
                //     vertical = inverted gradient because egui has V=0 at image top
                //     (outer=1.0 should map to image top = green grass)
                uv: egui::pos2(ev.uv[0] * splat_params_x, 1.0 - ev.color),
                color: egui::Color32::WHITE,
            });
        }
        target.indices.push(base);
        target.indices.push(base + 1);
        target.indices.push(base + 2);
    }

    (
        if mesh0.vertices.is_empty() {
            None
        } else {
            Some(mesh0)
        },
        if mesh1.vertices.is_empty() {
            None
        } else {
            Some(mesh1)
        },
    )
}

/// Transform world-space mesh into a reusable output buffer, avoiding allocation
/// after the first call. Reuses `out`'s vertex/index Vec capacity.
pub fn transform_mesh_to_screen_into(
    mesh: &egui::Mesh,
    camera: &Camera,
    canvas_center: egui::Vec2,
    out: &mut egui::Mesh,
) {
    out.texture_id = mesh.texture_id;
    out.vertices.clear();
    out.vertices
        .reserve(mesh.vertices.len().saturating_sub(out.vertices.capacity()));
    for v in &mesh.vertices {
        let screen = camera.world_to_screen(
            crate::domain::types::Vec2 {
                x: v.pos.x,
                y: v.pos.y,
            },
            canvas_center,
        );
        out.vertices.push(egui::epaint::Vertex {
            pos: screen,
            uv: v.uv,
            color: v.color,
        });
    }
    out.indices.clear();
    out.indices.extend_from_slice(&mesh.indices);
}

// ── Terrain draw pass (extracted from show()) ──

impl LevelRenderer {
    pub(in crate::renderer) fn draw_terrain_index(
        &mut self,
        terrain_index: usize,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let Some(td) = self.terrain_data.get(terrain_index) else {
            return;
        };

        let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
        let cam_cx = self.camera.center.x - tdx;
        let cam_cy = self.camera.center.y - tdy;

        // Fill
        if let Some(ref fill_data) = td.fill_mesh {
            let mut gpu_done = false;
            if let (Some(resources), Some(device), Some(queue)) =
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
                transform_mesh_to_screen_into(
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

        // Edge (right after fill)
        if let Some(gpu_idx) = self.edge_gpu_mesh_index.get(terrain_index).copied().flatten() {
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
                painter,
                td,
                &drag_cam,
                canvas_center,
                &self.tex_cache,
                &mut self.terrain_scratch_mesh,
            );
        }
    }

    /// Draw terrain fill + edge for either decorative or collider terrains.
    pub(super) fn draw_terrain_pass(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        decorative: bool,
    ) {
        for ti in 0..self.terrain_data.len() {
            if self.terrain_data[ti].decorative != decorative {
                continue;
            }
            self.draw_terrain_index(ti, painter, canvas_center, rect);
        }
    }
}
