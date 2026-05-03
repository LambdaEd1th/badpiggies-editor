//! Curve "stripe" (edge strip) mesh generation from curve nodes.

use crate::types::{TerrainMesh, Vec2};

use super::curve::{CurveNode, is_closed_loop};
use super::math::{normalize, point_in_triangle, segments_intersect_point, sub};

/// Regenerate the curve mesh (edge strip) from curve nodes and strip widths.
///
/// `strip_widths` contains the strip width for each curve texture index
/// (typically `[0.5, 0.1]` for grass and outline).
pub fn rebuild_curve_mesh(nodes: &[CurveNode], strip_widths: &[f32]) -> TerrainMesh {
    if nodes.len() < 2 {
        return TerrainMesh::default();
    }

    let n = nodes.len();
    let closed = is_closed_loop(nodes);

    // Compute stripe (inner) vertices using bisector normals.
    // The perpendicular convention is (y, -x), a RIGHT (CW) rotation of the tangent.
    // For curves whose nodes are ordered right-to-left (decreasing X), this points
    // OUTWARD (up/away from terrain body) — matching the original binary data.
    let mut stripe_verts: Vec<Vec2> = Vec::with_capacity(n);
    for i in 0..n {
        let strip_w = strip_widths.get(nodes[i].texture).copied().unwrap_or(0.5);
        let inner = if closed && (i == 0 || i == n - 1) {
            // Closed loop: collapse inner to outer at junction (zero-width taper),
            // matching Unity's tapered end behavior for closed terrains.
            nodes[i].position
        } else if i == 0 {
            compute_first_stripe_vertex(nodes, strip_w)
        } else if i == n - 1 {
            compute_last_stripe_vertex(nodes, strip_w)
        } else {
            compute_interior_stripe_vertex(nodes, i, strip_w)
        };
        stripe_verts.push(inner);
    }

    // Collapse self-intersecting inner (stripe) edges, matching Unity's
    // e2dTerrainCurveMesh.ComputeStripeVertices() post-processing.
    // When strip_w exceeds inter-node spacing, adjacent inner edges can cross.
    // For each pair of non-adjacent inner segments (j,j+1) and (k,k+1), if they
    // intersect, ALL inner vertices from j+1..k are collapsed to the intersection
    // point.  This creates degenerate (zero-area) quads that are harmless.
    collapse_inner_intersections(&mut stripe_verts);

    // Build interleaved vertex array: [outer0, inner0, outer1, inner1, ...]
    let mut vertices = Vec::with_capacity(n * 2);
    for i in 0..n {
        vertices.push(nodes[i].position);
        vertices.push(stripe_verts[i]);
    }

    // Triangulate the quad strip with bowtie detection.
    // For closed loops, skip the last segment (gap at junction, matching Unity).
    let segment_count = if closed && n >= 3 { n - 2 } else { n - 1 };
    let indices = triangulate_strip_n(&vertices, segment_count);

    TerrainMesh { vertices, indices }
}

/// Compute stripe vertex for the first node.
/// Uses the tangent from node[0] to node[1].
fn compute_first_stripe_vertex(nodes: &[CurveNode], strip_w: f32) -> Vec2 {
    let tangent = sub(nodes[1].position, nodes[0].position);
    // Perpendicular: (y, -x) — RIGHT (CW) rotation, matching original binary data
    let perp = normalize(Vec2 {
        x: tangent.y,
        y: -tangent.x,
    });
    Vec2 {
        x: nodes[0].position.x + perp.x * strip_w,
        y: nodes[0].position.y + perp.y * strip_w,
    }
}

/// Compute stripe vertex for the last node.
/// Uses the tangent from node[N-2] to node[N-1].
fn compute_last_stripe_vertex(nodes: &[CurveNode], strip_w: f32) -> Vec2 {
    let n = nodes.len();
    let tangent = sub(nodes[n - 1].position, nodes[n - 2].position);
    let perp = normalize(Vec2 {
        x: tangent.y,
        y: -tangent.x,
    });
    Vec2 {
        x: nodes[n - 1].position.x + perp.x * strip_w,
        y: nodes[n - 1].position.y + perp.y * strip_w,
    }
}

/// Compute stripe vertex for an interior node using bisector of adjacent edge normals.
fn compute_interior_stripe_vertex(nodes: &[CurveNode], i: usize, strip_w: f32) -> Vec2 {
    let incoming = sub(nodes[i].position, nodes[i - 1].position);
    let outgoing = sub(nodes[i + 1].position, nodes[i].position);

    // Perpendicular: (y, -x) — RIGHT (CW) rotation, matching original binary data
    let perp_in = normalize(Vec2 {
        x: incoming.y,
        y: -incoming.x,
    });
    let perp_out = normalize(Vec2 {
        x: outgoing.y,
        y: -outgoing.x,
    });

    // Bisector = normalize(perp_in + perp_out)
    let bisector = normalize(Vec2 {
        x: perp_in.x + perp_out.x,
        y: perp_in.y + perp_out.y,
    });

    Vec2 {
        x: nodes[i].position.x + bisector.x * strip_w,
        y: nodes[i].position.y + bisector.y * strip_w,
    }
}

/// Collapse self-intersecting inner edge segments.
///
/// Mirrors Unity's `e2dTerrainCurveMesh.ComputeStripeVertices()` post-processing:
/// for each pair of non-adjacent inner segments (j,j+1) and (k,k+1), if they
/// intersect, all inner vertices from j+1..k are collapsed to the intersection
/// point.  After collapsing, those segments become zero-length (degenerate) and
/// won't trigger further false intersections.
fn collapse_inner_intersections(stripe: &mut [Vec2]) {
    let n = stripe.len();
    if n < 3 {
        return;
    }
    for j in 0..n - 1 {
        for k in (j + 2)..n - 1 {
            if let Some(pt) =
                segments_intersect_point(stripe[j], stripe[j + 1], stripe[k], stripe[k + 1])
            {
                for l in (j + 1)..=k {
                    stripe[l] = pt;
                }
                break; // inner loop only — continue outer loop from j+1
            }
        }
    }
}

/// Test if segments AB and CD intersect, returning the intersection point.

/// Triangulate `segment_count` segments of a quad strip with bowtie detection.
/// Vertices are interleaved: [outer0, inner0, outer1, inner1, ...].
fn triangulate_strip_n(verts: &[Vec2], segment_count: usize) -> Vec<i16> {
    let pair_count = verts.len() / 2;
    if pair_count < 2 || segment_count == 0 {
        return Vec::new();
    }
    let segments = segment_count.min(pair_count - 1);
    let mut indices = Vec::with_capacity(segments * 6);

    for i in 1..=segments {
        let idx = (i * 2) as i16;
        // Vertices: outer(i-1)=idx-2, inner(i-1)=idx-1, outer(i)=idx, inner(i)=idx+1
        let o_prev = idx - 2;
        let i_prev = idx - 1;
        let o_curr = idx;
        let i_curr = idx + 1;

        if point_in_triangle(
            verts[i_curr as usize],
            verts[o_prev as usize],
            verts[o_curr as usize],
            verts[i_prev as usize],
        ) {
            // Bowtie case: swap diagonal
            indices.extend_from_slice(&[o_prev, i_curr, i_prev, o_prev, o_curr, i_curr]);
        } else {
            // Normal case
            indices.extend_from_slice(&[o_prev, o_curr, i_prev, i_prev, o_curr, i_curr]);
        }
    }
    indices
}

