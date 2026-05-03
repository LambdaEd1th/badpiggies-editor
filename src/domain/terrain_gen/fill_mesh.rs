//! Fill (interior) mesh generation — boundary stitching + ear-clip triangulation.

use crate::types::{TerrainMesh, Vec2};

use super::curve::{CurveNode, is_closed_loop};
use super::math::{
    count_self_intersections, cross2d, dist_sq, dot, point_in_triangle_strict,
    point_to_segment_dist_sq, project_onto_segment, signed_area, sub,
};

/// Rebuild the fill mesh from curve nodes and a bounding rectangle.
///
/// For **closed loops** (first ≈ last node), the fill polygon is simply the
/// curve nodes themselves — no boundary rectangle needed.
///
/// For **open curves**, the fill polygon is: boundary corners (walked CCW from
/// end to start edge) + curve node positions. Then ear-clipped into triangles.
///
/// `boundary` is `[min_x, min_y, max_x, max_y]` — inferred from existing fill mesh.
pub fn rebuild_fill_mesh(nodes: &[CurveNode], boundary: [f32; 4]) -> TerrainMesh {
    if nodes.len() < 2 {
        return TerrainMesh::default();
    }

    // Detect closed loop: first and last node nearly coincident
    let is_closed = is_closed_loop(nodes);

    let polygon = if is_closed {
        // Closed terrain: fill polygon = curve nodes (skip duplicate last node)
        let mut poly: Vec<Vec2> = nodes.iter().map(|n| n.position).collect();
        if poly.len() >= 2 && dist_sq(poly[poly.len() - 1], poly[0]) < 0.5 * 0.5 {
            poly.pop();
        }
        poly
    } else {
        // Open terrain: use boundary rectangle + corner walk
        build_fill_polygon(nodes, boundary)
    };
    if polygon.len() < 3 {
        return TerrainMesh::default();
    }

    let indices = ear_clip_triangulate(&polygon);

    TerrainMesh {
        vertices: polygon,
        indices,
    }
}

/// Build the fill polygon from curve nodes + boundary rect corners.
///

///   0: (max_x, min_y)  bottom-right
///   1: (min_x, min_y)  bottom-left
///   2: (min_x, max_y)  top-left
///   3: (max_x, max_y)  top-right
///
/// The polygon walks: end_project → boundary corners → start_project → curve nodes.
fn build_fill_polygon(nodes: &[CurveNode], boundary: [f32; 4]) -> Vec<Vec2> {
    let [min_x, min_y, max_x, max_y] = boundary;
    let corners = [
        Vec2 { x: max_x, y: min_y }, // 0: bottom-right
        Vec2 { x: min_x, y: min_y }, // 1: bottom-left
        Vec2 { x: min_x, y: max_y }, // 2: top-left
        Vec2 { x: max_x, y: max_y }, // 3: top-right
    ];

    let start = nodes[0].position;
    let end = nodes[nodes.len() - 1].position;

    let start_edge = project_to_boundary_edge(start, &corners);
    let start_proj = project_to_boundary(start, &corners);
    let end_edge = project_to_boundary_edge(end, &corners);
    let end_proj = project_to_boundary(end, &corners);

    let mut polygon = Vec::new();

    // Add projected end point if it differs from the actual end
    if dist_sq(end_proj, end) > 1e-6 {
        polygon.push(end_proj);
    }

    // Walk boundary corners from end edge to start edge (CCW).
    // Special case: if both endpoints project to the SAME edge, check whether
    // end_proj comes before start_proj in CCW order on that edge.  If yes, no
    // corners are needed (short path); otherwise walk all 4 corners (long path).
    let skip_corners = if end_edge == start_edge {
        // Parametric position along the edge: 0 at corners[edge], 1 at corners[(edge+1)%4]
        let a = corners[end_edge];
        let b = corners[(end_edge + 1) % 4];
        let ab = sub(b, a);
        let len2 = dot(ab, ab).max(1e-10);
        let t_end = dot(sub(end_proj, a), ab) / len2;
        let t_start = dot(sub(start_proj, a), ab) / len2;
        t_end <= t_start // end comes first in CCW order → short path, no corners
    } else {
        false
    };
    if !skip_corners {
        let mut edge = end_edge;
        loop {
            polygon.push(corners[(edge + 1) % 4]);
            edge = (edge + 1) % 4;
            if edge == start_edge {
                break;
            }
        }
    }

    // Add projected start point if it differs
    if dist_sq(start_proj, start) > 1e-6 {
        polygon.push(start_proj);
    }

    // Add all curve nodes
    for node in nodes {
        polygon.push(node.position);
    }

    // Remove duplicate if last == first
    if polygon.len() >= 2 && dist_sq(polygon[polygon.len() - 1], polygon[0]) < 1e-6 {
        polygon.pop();
    }

    polygon
}

/// Project a point to the nearest boundary edge, return the edge index (0-3).
fn project_to_boundary_edge(p: Vec2, corners: &[Vec2; 4]) -> usize {
    let mut best_edge = 0;
    let mut best_dist = f32::MAX;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        let d = point_to_segment_dist_sq(p, a, b);
        if d < best_dist {
            best_dist = d;
            best_edge = i;
        }
    }
    best_edge
}

/// Project a point onto the nearest boundary edge.
fn project_to_boundary(p: Vec2, corners: &[Vec2; 4]) -> Vec2 {
    let edge = project_to_boundary_edge(p, corners);
    let a = corners[edge];
    let b = corners[(edge + 1) % 4];
    project_onto_segment(p, a, b)
}

/// Triangulate a simple polygon using ear-clipping.
/// Returns indices (i16) into the original polygon. Handles both CW and CCW winding.
pub(super) fn ear_clip_triangulate(polygon: &[Vec2]) -> Vec<i16> {
    let n = polygon.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![0, 1, 2];
    }

    // Detect self-intersections (O(n²) diagnostic)
    let si = count_self_intersections(polygon);
    if si > 0 {
        log::warn!(
            "ear_clip_triangulate: polygon has {n} vertices and {si} self-intersection(s) — triangulation may be incorrect"
        );
    }

    // Ensure CCW winding for consistent is_ear checks
    let area = signed_area(polygon);
    let ccw: Vec<Vec2> = if area < 0.0 {
        polygon.iter().copied().rev().collect()
    } else {
        polygon.to_vec()
    };

    // Active vertex indices (into `ccw`)
    let mut active: Vec<usize> = (0..n).collect();
    let mut indices = Vec::with_capacity((n - 2) * 3);

    let mut fail_count = 0;
    while active.len() > 3 {
        let m = active.len();
        let mut clipped = false;
        for ai in 0..m {
            let prev = active[(ai + m - 1) % m];
            let curr = active[ai];
            let next = active[(ai + 1) % m];

            // Must be a convex vertex (left turn in CCW polygon)
            let cross_val = cross2d(sub(ccw[curr], ccw[prev]), sub(ccw[next], ccw[curr]));
            if cross_val < 1e-10 {
                continue; // reflex or collinear vertex, skip
            }

            // No other active vertex must lie inside this triangle
            let mut ear = true;
            for aj in 0..m {
                let vi = active[aj];
                if vi == prev || vi == curr || vi == next {
                    continue;
                }
                if point_in_triangle_strict(ccw[vi], ccw[prev], ccw[curr], ccw[next]) {
                    ear = false;
                    break;
                }
            }
            if !ear {
                continue;
            }

            indices.extend_from_slice(&[prev as i16, curr as i16, next as i16]);
            active.remove(ai);
            clipped = true;
            fail_count = 0;
            break;
        }
        if !clipped {
            fail_count += 1;
            if fail_count > active.len() {
                // Degenerate polygon — emit remaining as fan to avoid infinite loop
                for i in 2..active.len() {
                    indices.extend_from_slice(&[
                        active[0] as i16,
                        active[i - 1] as i16,
                        active[i] as i16,
                    ]);
                }
                break;
            }
        }
    }
    if active.len() == 3 {
        indices.extend_from_slice(&[active[0] as i16, active[1] as i16, active[2] as i16]);
    }

    // Map back to original polygon indices if we reversed
    if area < 0.0 {
        indices.iter().map(|&i| (n as i16 - 1 - i) as i16).collect()
    } else {
        indices
    }
}

