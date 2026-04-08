//! Terrain rendering — fill mesh (colored triangles) and curve edge (vertex-colored quads).
//!
//! The terrain fill mesh is drawn as indexed triangles tinted with the fill color.
//! The edge mesh uses the fallback approach: per-quad vertex coloring based on
//! control texture data (grass vs outline selection).

use eframe::egui;

use crate::assets;
use crate::types::*;

use super::Camera;
use super::edge_shader::EdgeVertex;

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

    // Resolve fill texture: level-refs first, then name-based fallback.
    // Each terrain object's texture is determined by its per-level loader
    // m_references entry (captured in level-refs.toml), NOT by the background
    // theme.  Different terrain prefab names (e.g. MM_sand, MM_rock,
    // Dark_MM_rock) map to different textures within the same level.
    let fill_texture = crate::level_refs::get_level_ref(level_key, td.fill_texture_index)
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
        crate::level_refs::get_level_ref(level_key, td.curve_textures[0].texture_index)
            .map(|s| s.to_string())
            .or_else(|| assets::get_terrain_splat0(&prefab.name).map(|s| s.to_string()))
    } else {
        assets::get_terrain_splat0(&prefab.name).map(|s| s.to_string())
    };
    // Splat1: prefer the curated manual map (assets.rs) because the GUID→filename
    // mapping in level-refs is unreliable for these small solid-color outline textures.
    let edge_splat1 = assets::get_terrain_splat1(&prefab.name)
        .map(|s| s.to_string())
        .or_else(|| {
            if td.curve_textures.len() > 1 {
                crate::level_refs::get_level_ref(level_key, td.curve_textures[1].texture_index)
                    .map(|s| s.to_string())
            } else {
                None
            }
        });

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
                    crate::terrain_gen::decode_control_png_pixels(data).map(|px| {
                        let gi = i * 4 + 1; // green channel
                        if gi < px.len() && px[gi] > 128 {
                            1
                        } else {
                            0
                        }
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
        mesh.indices.push(idx as u32);
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
            crate::types::Vec2 {
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
