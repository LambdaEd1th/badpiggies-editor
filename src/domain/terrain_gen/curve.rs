//! Curve nodes — extraction from existing terrain data and closed-loop detection.

use crate::domain::types::{TerrainData, Vec2};

use super::math::dist_sq;
use super::png::decode_control_png_pixels;

/// A curve node: position + texture index (0 or 1 in practice).
#[derive(Debug, Clone)]
pub struct CurveNode {
    pub position: Vec2,
    pub texture: usize,
}

/// Extract curve nodes from existing TerrainData.
/// Outer vertices (even indices) are curve node positions.
/// Texture index per node comes from the control texture PNG.
pub fn extract_curve_nodes(td: &TerrainData) -> Vec<CurveNode> {
    let verts = &td.curve_mesh.vertices;
    let node_count = verts.len() / 2;
    let ctrl_pixels = td
        .control_texture_data
        .as_ref()
        .and_then(|d| decode_control_png_pixels(d));

    (0..node_count)
        .map(|i| {
            let pos = verts[i * 2]; // outer vertex
            let texture = ctrl_pixels
                .as_ref()
                .map(|px| {
                    let gi = i * 4 + 1; // green channel
                    if gi < px.len() && px[gi] > 128 { 1 } else { 0 }
                })
                .unwrap_or(0);
            CurveNode {
                position: pos,
                texture,
            }
        })
        .collect()
}

/// Returns true if the curve nodes form a closed loop (first ≈ last node).
pub fn is_closed_loop(nodes: &[CurveNode]) -> bool {
    nodes.len() >= 2 && dist_sq(nodes[0].position, nodes[nodes.len() - 1].position) < 0.5 * 0.5
}
