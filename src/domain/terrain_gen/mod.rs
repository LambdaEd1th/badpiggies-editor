//! Terrain mesh generation — rebuilds curve mesh, fill mesh, and control texture
//! from curve node positions and texture indices.
//!
//! Implements the Unity e2d algorithms:
//! - `compute_stripe_vertices`: bisector normals × strip width
//! - `triangulate_strip`: quad-strip indices with bowtie detection
//! - `ear_clip_triangulate`: fill polygon from boundary + curve nodes
//! - `encode_control_png`: 1×N RGBA PNG from node texture indices

mod curve;
mod fill_mesh;
mod math;
mod png;
mod stripe_mesh;

pub use curve::{CurveNode, extract_curve_nodes, is_closed_loop};
pub use fill_mesh::rebuild_fill_mesh;
pub use png::{decode_control_png_pixels, encode_control_png};
pub use stripe_mesh::rebuild_curve_mesh;

use crate::domain::types::{TerrainData, Vec2};

/// Compute initial boundary rect from the original fill mesh vertices.
/// Returns `[min_x, min_y, max_x, max_y]`.
fn infer_boundary_from_fill_mesh(fill_verts: &[Vec2]) -> [f32; 4] {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for v in fill_verts {
        min_x = min_x.min(v.x);
        min_y = min_y.min(v.y);
        max_x = max_x.max(v.x);
        max_y = max_y.max(v.y);
    }

    if min_x > max_x {
        // Empty mesh — return a small default boundary
        [0.0, 0.0, 1.0, 1.0]
    } else {
        [min_x, min_y, max_x, max_y]
    }
}

/// Expand boundary to include all node positions.
/// The boundary never shrinks — only grows when nodes move outside.
fn expand_boundary_for_nodes(boundary: [f32; 4], nodes: &[CurveNode]) -> [f32; 4] {
    let [mut min_x, mut min_y, mut max_x, mut max_y] = boundary;

    for n in nodes {
        min_x = min_x.min(n.position.x);
        min_y = min_y.min(n.position.y);
        max_x = max_x.max(n.position.x);
        max_y = max_y.max(n.position.y);
    }

    [min_x, min_y, max_x, max_y]
}

/// Regenerate all terrain meshes and control texture from curve nodes.
/// Updates `td` in-place.
pub fn regenerate_terrain(td: &mut TerrainData, nodes: &[CurveNode]) {
    // Collect strip widths from curve textures
    let strip_widths: Vec<f32> = td.curve_textures.iter().map(|ct| ct.size.y).collect();
    let strip_widths = if strip_widths.is_empty() {
        vec![0.5, 0.1]
    } else {
        strip_widths
    };

    // Rebuild curve mesh
    td.curve_mesh = rebuild_curve_mesh(nodes, &strip_widths);

    // Compute fill boundary: start from the cached boundary (preserving original
    // ground extent), then expand if nodes moved outside it.
    let boundary = if let Some(cached) = td.fill_boundary {
        expand_boundary_for_nodes(cached, nodes)
    } else {
        // First edit: compute initial boundary from the original fill mesh
        let initial = infer_boundary_from_fill_mesh(&td.fill_mesh.vertices);
        expand_boundary_for_nodes(initial, nodes)
    };
    td.fill_boundary = Some(boundary);

    td.fill_mesh = rebuild_fill_mesh(nodes, boundary);

    // Rebuild control texture
    match encode_control_png(nodes) {
        Ok(png) => {
            td.control_texture_data = Some(png);
            td.control_texture_count = 1;
        }
        Err(error) => {
            log::error!("Failed to rebuild terrain control texture: {error}");
            td.control_texture_data = None;
            td.control_texture_count = 0;
        }
    }
}

#[cfg(test)]
mod tests;
