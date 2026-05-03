//! Terrain mesh generation — rebuilds curve mesh, fill mesh, and control texture
//! from curve node positions and texture indices.
//!
//! Implements the Unity e2d algorithms:
//! - `compute_stripe_vertices`: bisector normals × strip width
//! - `triangulate_strip`: quad-strip indices with bowtie detection
//! - `ear_clip_triangulate`: fill polygon from boundary + curve nodes
//! - `encode_control_png`: 1×N RGBA PNG from node texture indices

use crate::error::{AppError, AppResult};
use crate::types::{TerrainData, TerrainMesh, Vec2};

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

/// Decode raw PNG bytes to RGBA pixel data.
pub fn decode_control_png_pixels(data: &[u8]) -> Option<Vec<u8>> {
    let decoder = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = decoder.decode().ok()?;
    Some(img.to_rgba8().into_raw())
}

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
///
/// Reimplements `e2dUtils.SegmentsIntersect` from the Unity C# source:
/// uses Cramer's rule for the intersection of the two infinite lines, then
/// checks that the intersection lies within the bounding box of each segment
/// (with a small epsilon for near-axis-aligned segments).
fn segments_intersect_point(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let denom = (a.x - b.x) * (c.y - d.y) - (a.y - b.y) * (c.x - d.x);
    if denom.abs() <= f32::MIN_POSITIVE {
        return None; // parallel / degenerate
    }
    let ab_cross = a.x * b.y - a.y * b.x;
    let cd_cross = c.x * d.y - c.y * d.x;
    let ix = ((c.x - d.x) * ab_cross - (a.x - b.x) * cd_cross) / denom;
    let iy = ((c.y - d.y) * ab_cross - (a.y - b.y) * cd_cross) / denom;

    // Epsilon: Unity uses 0.01 when the segment is near-vertical or near-horizontal
    let eps_ab = if (a.x - b.x).abs() <= f32::MIN_POSITIVE || (a.y - b.y).abs() <= f32::MIN_POSITIVE
    {
        0.01
    } else {
        0.0
    };
    if ix < a.x.min(b.x) - eps_ab || ix > a.x.max(b.x) + eps_ab {
        return None;
    }
    if iy < a.y.min(b.y) - eps_ab || iy > a.y.max(b.y) + eps_ab {
        return None;
    }

    let eps_cd = if (c.x - d.x).abs() <= f32::MIN_POSITIVE || (c.y - d.y).abs() <= f32::MIN_POSITIVE
    {
        0.01
    } else {
        0.0
    };
    if ix < c.x.min(d.x) - eps_cd || ix > c.x.max(d.x) + eps_cd {
        return None;
    }
    if iy < c.y.min(d.y) - eps_cd || iy > c.y.max(d.y) + eps_cd {
        return None;
    }

    Some(Vec2 { x: ix, y: iy })
}

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

/// Test if point P is inside triangle (A, B, C) using cross products.
fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let cross = |o: Vec2, v1: Vec2, v2: Vec2| -> f32 {
        (v1.x - o.x) * (v2.y - o.y) - (v1.y - o.y) * (v2.x - o.x)
    };
    let d1 = cross(p, a, b);
    let d2 = cross(p, b, c);
    let d3 = cross(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

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
/// Boundary rect corners (CCW from bottom-right):
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

/// Project point P onto line segment AB.
fn project_onto_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = sub(b, a);
    let ap = sub(p, a);
    let t = dot(ap, ab) / dot(ab, ab).max(1e-10);
    let t = t.clamp(0.0, 1.0);
    Vec2 {
        x: a.x + t * ab.x,
        y: a.y + t * ab.y,
    }
}

/// Squared distance from point to segment.
fn point_to_segment_dist_sq(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let proj = project_onto_segment(p, a, b);
    dist_sq(p, proj)
}

/// Compute signed area of a polygon. Positive = CCW, negative = CW.
fn signed_area(polygon: &[Vec2]) -> f64 {
    let n = polygon.len();
    let mut area = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += polygon[i].x as f64 * polygon[j].y as f64;
        area -= polygon[j].x as f64 * polygon[i].y as f64;
    }
    area * 0.5
}

/// Triangulate a simple polygon using ear-clipping.
/// Returns indices (i16) into the original polygon. Handles both CW and CCW winding.
fn ear_clip_triangulate(polygon: &[Vec2]) -> Vec<i16> {
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

/// Strict point-in-triangle test (excludes edges).
fn point_in_triangle_strict(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross2d(sub(b, a), sub(p, a));
    let d2 = cross2d(sub(c, b), sub(p, b));
    let d3 = cross2d(sub(a, c), sub(p, c));
    // All same sign (strictly inside)
    (d1 > 0.0 && d2 > 0.0 && d3 > 0.0) || (d1 < 0.0 && d2 < 0.0 && d3 < 0.0)
}

/// Count the number of self-intersections in a polygon (edges crossing).
fn count_self_intersections(polygon: &[Vec2]) -> usize {
    let n = polygon.len();
    let mut count = 0;
    for i in 0..n {
        let a1 = polygon[i];
        let a2 = polygon[(i + 1) % n];
        // Check against all non-adjacent edges
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue; // skip adjacent (first and last share a vertex)
            }
            let b1 = polygon[j];
            let b2 = polygon[(j + 1) % n];
            if edges_cross(a1, a2, b1, b2) {
                count += 1;
            }
        }
    }
    count
}

/// Test if two line segments strictly cross (not just touch at endpoints).
fn edges_cross(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = cross2d(sub(b2, b1), sub(a1, b1));
    let d2 = cross2d(sub(b2, b1), sub(a2, b1));
    let d3 = cross2d(sub(a2, a1), sub(b1, a1));
    let d4 = cross2d(sub(a2, a1), sub(b2, a1));
    // Strict crossing: opposite signs on both pairs
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Encode node texture indices into a 1×N PNG (control texture).
/// Returns raw PNG bytes.
pub fn encode_control_png(nodes: &[CurveNode]) -> AppResult<Vec<u8>> {
    let n = nodes.len().max(1);
    // Control texture width = next power of two of node count
    let tex_width = n.next_power_of_two();

    let mut pixels = vec![0u8; tex_width * 4]; // RGBA
    for (i, node) in nodes.iter().enumerate() {
        let base = i * 4;
        match node.texture % 4 {
            0 => pixels[base] = 255,     // R
            1 => pixels[base + 1] = 255, // G
            2 => pixels[base + 2] = 255, // B
            3 => pixels[base + 3] = 255, // A
            _ => unreachable!(),
        }
    }

    // Encode as PNG
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            &pixels,
            tex_width as u32,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| {
            AppError::invalid_data_key1("error_terrain_control_png_encode", error.to_string())
        })?;
    }
    Ok(buf)
}

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

// ── Vector math helpers ──

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn dist_sq(a: Vec2, b: Vec2) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

fn normalize(v: Vec2) -> Vec2 {
    let len = (v.x * v.x + v.y * v.y).sqrt();
    if len < 1e-10 {
        Vec2 { x: 0.0, y: 0.0 }
    } else {
        Vec2 {
            x: v.x / len,
            y: v.y / len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripe_vertex_basic() {
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 0.0 },
                texture: 0,
            },
        ];
        let mesh = rebuild_curve_mesh(&nodes, &[0.5]);
        // Should have 6 vertices (3 pairs)
        assert_eq!(mesh.vertices.len(), 6);
        // Inner vertices should be offset in Y by strip_width (0.5)
        // Perpendicular of (1,0) is (0,-1), so inner = pos + (0,-0.5)
        assert!((mesh.vertices[1].y - (-0.5)).abs() < 0.01);
        assert!((mesh.vertices[3].y - (-0.5)).abs() < 0.01);
    }

    #[test]
    fn ear_clip_triangle() {
        let polygon = vec![
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 1.0, y: 0.0 },
            Vec2 { x: 0.5, y: 1.0 },
        ];
        let indices = ear_clip_triangulate(&polygon);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn ear_clip_square() {
        let polygon = vec![
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 1.0, y: 0.0 },
            Vec2 { x: 1.0, y: 1.0 },
            Vec2 { x: 0.0, y: 1.0 },
        ];
        let indices = ear_clip_triangulate(&polygon);
        assert_eq!(indices.len(), 6); // 2 triangles
    }

    #[test]
    fn control_png_roundtrip() {
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.0, y: 0.0 },
                texture: 1,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 0.0 },
                texture: 0,
            },
        ];
        let png_result = encode_control_png(&nodes);
        assert!(png_result.is_ok(), "control PNG should encode: {png_result:?}");
        let Ok(png) = png_result else {
            return;
        };
        let pixels_result = decode_control_png_pixels(&png);
        assert!(pixels_result.is_some(), "control PNG should decode");
        let Some(pixels) = pixels_result else {
            return;
        };
        // Node 0: R=255, G=0 → texture 0
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[1], 0);
        // Node 1: R=0, G=255 → texture 1
        assert_eq!(pixels[4], 0);
        assert_eq!(pixels[5], 255);
        // Node 2: R=255, G=0 → texture 0
        assert_eq!(pixels[8], 255);
        assert_eq!(pixels[9], 0);
    }

    /// Test earcut with the real 109-vertex closed polygon from the user's terrain.
    /// Compares total triangulated area vs polygon signed area.
    #[test]
    fn ear_clip_closed_terrain_real() {
        let polygon = vec![
            Vec2 {
                x: 3.2207842,
                y: -6.590766,
            },
            Vec2 {
                x: 3.0066977,
                y: -6.5920105,
            },
            Vec2 {
                x: 2.5506659,
                y: -6.5171385,
            },
            Vec2 {
                x: 2.2368956,
                y: -6.5573473,
            },
            Vec2 {
                x: 1.1215711,
                y: -6.5516853,
            },
            Vec2 {
                x: 1.0055976,
                y: -6.469494,
            },
            Vec2 {
                x: 1.138556,
                y: -6.268608,
            },
            Vec2 {
                x: 1.5014915,
                y: -6.001313,
            },
            Vec2 {
                x: 1.6752815,
                y: -5.134963,
            },
            Vec2 {
                x: 2.2660275,
                y: -4.855136,
            },
            Vec2 {
                x: 2.110568,
                y: -4.093384,
            },
            Vec2 {
                x: 2.312665,
                y: -3.5026374,
            },
            Vec2 {
                x: 2.7168608,
                y: -3.2849941,
            },
            Vec2 {
                x: 2.4370327,
                y: -2.942983,
            },
            Vec2 {
                x: 2.3282118,
                y: -2.4144204,
            },
            Vec2 {
                x: 2.5614014,
                y: -2.0257716,
            },
            Vec2 {
                x: 2.9189577,
                y: -1.9014039,
            },
            Vec2 {
                x: 3.2776175,
                y: -1.5149561,
            },
            Vec2 {
                x: 2.9500504,
                y: -0.93755436,
            },
            Vec2 {
                x: 2.5769472,
                y: -0.8287328,
            },
            Vec2 {
                x: 2.0639305,
                y: -0.81318676,
            },
            Vec2 {
                x: 1.8618331,
                y: -0.36235404,
            },
            Vec2 {
                x: 1.9084706,
                y: 0.104024634,
            },
            Vec2 {
                x: 2.2504816,
                y: 1.3943391,
            },
            Vec2 {
                x: 1.5509138,
                y: 1.3166093,
            },
            Vec2 {
                x: 0.82075214,
                y: 1.4809455,
            },
            Vec2 {
                x: 0.804708,
                y: 2.1716368,
            },
            Vec2 {
                x: 1.1622648,
                y: 2.3892803,
            },
            Vec2 {
                x: 0.7269783,
                y: 3.1043944,
            },
            Vec2 {
                x: 0.97571325,
                y: 3.555227,
            },
            Vec2 {
                x: 1.442092,
                y: 3.8350542,
            },
            Vec2 {
                x: 1.8151951,
                y: 5.140915,
            },
            Vec2 {
                x: 2.3437576,
                y: 5.793844,
            },
            Vec2 {
                x: 2.358778,
                y: 6.5781584,
            },
            Vec2 {
                x: 2.9655952,
                y: 6.773239,
            },
            Vec2 {
                x: 3.1055098,
                y: 7.34844,
            },
            Vec2 {
                x: 3.4008827,
                y: 7.830365,
            },
            Vec2 {
                x: 3.8983536,
                y: 7.9702787,
            },
            Vec2 {
                x: 4.349186,
                y: 7.5039005,
            },
            Vec2 {
                x: 4.7067432,
                y: 7.6282673,
            },
            Vec2 {
                x: 4.629013,
                y: 8.172377,
            },
            Vec2 {
                x: 5.297489,
                y: 8.88749,
            },
            Vec2 {
                x: 5.4915133,
                y: 9.249216,
            },
            Vec2 {
                x: 5.956813,
                y: 9.389813,
            },
            Vec2 {
                x: 6.265995,
                y: 9.188709,
            },
            Vec2 {
                x: 6.3390684,
                y: 8.794214,
            },
            Vec2 {
                x: 6.7121716,
                y: 8.716485,
            },
            Vec2 {
                x: 6.9298153,
                y: 8.421112,
            },
            Vec2 {
                x: 7.0075445,
                y: 7.752635,
            },
            Vec2 {
                x: 7.520561,
                y: 7.4261703,
            },
            Vec2 {
                x: 7.6760206,
                y: 6.664418,
            },
            Vec2 {
                x: 7.9631186,
                y: 6.330127,
            },
            Vec2 {
                x: 7.7848425,
                y: 5.8715744,
            },
            Vec2 {
                x: 8.1734915,
                y: 5.638386,
            },
            Vec2 {
                x: 8.515503,
                y: 6.0270348,
            },
            Vec2 {
                x: 9.0440645,
                y: 5.824937,
            },
            Vec2 {
                x: 9.0440645,
                y: 5.2030983,
            },
            Vec2 {
                x: 9.354984,
                y: 4.767812,
            },
            Vec2 {
                x: 8.935244,
                y: 4.441347,
            },
            Vec2 {
                x: 9.774725,
                y: 4.363617,
            },
            Vec2 {
                x: 10.318833,
                y: 4.2081575,
            },
            Vec2 {
                x: 10.52093,
                y: 3.6795948,
            },
            Vec2 {
                x: 10.809194,
                y: 3.3715112,
            },
            Vec2 {
                x: 10.413821,
                y: 3.038582,
            },
            Vec2 {
                x: 10.185775,
                y: 3.0582244,
            },
            Vec2 {
                x: 9.9146385,
                y: 2.813534,
            },
            Vec2 {
                x: 9.629618,
                y: 2.8633583,
            },
            Vec2 {
                x: 9.252022,
                y: 2.701201,
            },
            Vec2 {
                x: 9.334412,
                y: 2.501111,
            },
            Vec2 {
                x: 9.308347,
                y: 2.0317233,
            },
            Vec2 {
                x: 8.863612,
                y: 1.6105142,
            },
            Vec2 {
                x: 8.484608,
                y: 1.5097895,
            },
            Vec2 {
                x: 8.63987,
                y: 1.2855173,
            },
            Vec2 {
                x: 9.05685,
                y: 1.2356344,
            },
            Vec2 {
                x: 9.293237,
                y: 0.8488258,
            },
            Vec2 {
                x: 9.277254,
                y: 0.5859493,
            },
            Vec2 {
                x: 9.453387,
                y: 0.06870089,
            },
            Vec2 {
                x: 9.35459,
                y: -0.14535916,
            },
            Vec2 {
                x: 9.402237,
                y: -0.23703015,
            },
            Vec2 {
                x: 8.727954,
                y: -0.49190336,
            },
            Vec2 {
                x: 8.2667675,
                y: -0.5333596,
            },
            Vec2 {
                x: 7.910351,
                y: -0.6764072,
            },
            Vec2 {
                x: 8.235675,
                y: -1.5127549,
            },
            Vec2 {
                x: 8.889873,
                y: -1.6298243,
            },
            Vec2 {
                x: 8.889465,
                y: -1.928469,
            },
            Vec2 {
                x: 9.331049,
                y: -2.4003015,
            },
            Vec2 {
                x: 9.328866,
                y: -3.1641505,
            },
            Vec2 {
                x: 9.150342,
                y: -3.4498353,
            },
            Vec2 {
                x: 9.068795,
                y: -3.8293705,
            },
            Vec2 {
                x: 9.228912,
                y: -4.0434694,
            },
            Vec2 {
                x: 9.230617,
                y: -4.4975786,
            },
            Vec2 {
                x: 9.095525,
                y: -4.7898064,
            },
            Vec2 {
                x: 8.981684,
                y: -4.88396,
            },
            Vec2 {
                x: 9.0466585,
                y: -5.2832966,
            },
            Vec2 {
                x: 8.875276,
                y: -5.4213963,
            },
            Vec2 {
                x: 8.526504,
                y: -5.550169,
            },
            Vec2 {
                x: 8.295506,
                y: -5.7614822,
            },
            Vec2 {
                x: 8.162485,
                y: -5.772996,
            },
            Vec2 {
                x: 8.049504,
                y: -6.1038923,
            },
            Vec2 {
                x: 8.191641,
                y: -6.284794,
            },
            Vec2 {
                x: 8.1619425,
                y: -6.460931,
            },
            Vec2 {
                x: 7.8492203,
                y: -6.575528,
            },
            Vec2 {
                x: 7.2548304,
                y: -6.4656954,
            },
            Vec2 {
                x: 6.9899387,
                y: -6.5561457,
            },
            Vec2 {
                x: 6.595832,
                y: -6.569067,
            },
            Vec2 {
                x: 5.72363,
                y: -6.4333916,
            },
            Vec2 {
                x: 4.760975,
                y: -6.407548,
            },
            Vec2 {
                x: 4.483162,
                y: -6.5432243,
            },
            Vec2 {
                x: 3.9856825,
                y: -6.485078,
            },
        ];

        let n = polygon.len();
        assert_eq!(n, 109);

        let indices = ear_clip_triangulate(&polygon);
        let tri_count = indices.len() / 3;
        assert_eq!(
            tri_count,
            n - 2,
            "expected {} triangles, got {}",
            n - 2,
            tri_count
        );

        // Compute total triangulated area (sum of absolute triangle areas)
        let mut tri_area_sum = 0.0_f64;
        for t in 0..tri_count {
            let a = polygon[indices[t * 3] as usize];
            let b = polygon[indices[t * 3 + 1] as usize];
            let c = polygon[indices[t * 3 + 2] as usize];
            let area = ((b.x - a.x) as f64 * (c.y - a.y) as f64
                - (c.x - a.x) as f64 * (b.y - a.y) as f64)
                .abs()
                * 0.5;
            tri_area_sum += area;
        }

        // Compute polygon area via shoelace
        let poly_area = signed_area(&polygon).abs();

        let diff = (tri_area_sum - poly_area).abs();
        let rel = diff / poly_area;
        assert!(
            rel < 0.01,
            "triangulated area {tri_area_sum:.4} vs polygon area {poly_area:.4}, relative error {rel:.6}"
        );
    }

    /// Test that an open U-shaped curve survives a close→open roundtrip.
    #[test]
    fn open_close_open_roundtrip() {
        // U-shaped open curve: goes up, right, down — mimics the real terrain
        // that starts at (3.7, -18.1) and loops around
        let original_nodes = vec![
            CurveNode {
                position: Vec2 {
                    x: 3.7255,
                    y: -18.1278,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.6659,
                    y: -10.8056,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.7508,
                    y: -9.9418,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.2167,
                    y: -9.8004,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.2559,
                    y: -9.4390,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.0, y: -8.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.5, y: -6.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.5, y: -5.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.5, y: -6.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.0, y: -8.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.2, y: -10.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.5, y: -15.0 },
                texture: 0,
            },
        ];
        let strip_widths = [0.5, 0.1];
        let mesh0 = rebuild_curve_mesh(&original_nodes, &strip_widths);
        // Dump for debugging
        for i in 0..original_nodes.len() {
            let outer = mesh0.vertices[i * 2];
            let inner = mesh0.vertices[i * 2 + 1];
            eprintln!(
                "original[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // Close: add duplicate of first node
        let mut closed_nodes = original_nodes.clone();
        closed_nodes.push(CurveNode {
            position: original_nodes[0].position,
            texture: original_nodes[0].texture,
        });
        let mesh_closed = rebuild_curve_mesh(&closed_nodes, &strip_widths);
        eprintln!("--- closed ---");
        for i in 0..closed_nodes.len() {
            let outer = mesh_closed.vertices[i * 2];
            let inner = mesh_closed.vertices[i * 2 + 1];
            eprintln!(
                "closed[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // Open: remove last node
        let mut reopened_nodes = closed_nodes;
        reopened_nodes.pop();
        let mesh1 = rebuild_curve_mesh(&reopened_nodes, &strip_widths);
        eprintln!("--- reopened ---");
        for i in 0..reopened_nodes.len() {
            let outer = mesh1.vertices[i * 2];
            let inner = mesh1.vertices[i * 2 + 1];
            eprintln!(
                "reopened[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // All vertices should match the original
        assert_eq!(mesh0.vertices.len(), mesh1.vertices.len());
        for i in 0..mesh0.vertices.len() {
            let v0 = mesh0.vertices[i];
            let v1 = mesh1.vertices[i];
            assert!(
                (v0.x - v1.x).abs() < 0.001 && (v0.y - v1.y).abs() < 0.001,
                "vertex {} mismatch: ({:.4},{:.4}) vs ({:.4},{:.4})",
                i,
                v0.x,
                v0.y,
                v1.x,
                v1.y
            );
        }
    }

    /// Test that inner edges are collapsed when strip_w exceeds inter-node spacing.
    #[test]
    fn inner_collapse_dense_nodes() {
        // Nodes spaced 0.3 apart with a sharp turn — strip_w=0.5 exceeds spacing
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.3, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.6, y: 0.3 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.9, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.2, y: 0.0 },
                texture: 0,
            },
        ];
        let mesh = rebuild_curve_mesh(&nodes, &[0.5]);

        // Verify no NaN/Inf in vertices
        for (i, v) in mesh.vertices.iter().enumerate() {
            assert!(v.x.is_finite(), "vertex {} x is not finite: {:?}", i, v);
            assert!(v.y.is_finite(), "vertex {} y is not finite: {:?}", i, v);
        }

        // Verify the mesh is valid
        assert_eq!(mesh.vertices.len(), 10); // 5 nodes * 2
        assert!(mesh.indices.len() >= 6, "should have at least 1 quad");
    }

    /// Test that an open terrain fill mesh is NOT just a rectangle.
    #[test]
    fn open_fill_mesh_not_rectangle() {
        // Simple open terrain: 5 nodes along a wave, boundary extending below
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 2.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 3.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 4.0, y: 1.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 6.0, y: 2.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 8.0, y: 2.0 },
                texture: 0,
            },
        ];
        // Boundary extends well below the curve
        let boundary = [-1.0, -3.0, 9.0, 4.0];
        let fill = rebuild_fill_mesh(&nodes, boundary);

        assert!(
            fill.vertices.len() > 4,
            "should have more than 4 verts (not just corners)"
        );
        assert!(fill.indices.len() >= 9, "should have at least 3 triangles");

        // Verify all vertices are finite
        for (i, v) in fill.vertices.iter().enumerate() {
            assert!(
                v.x.is_finite() && v.y.is_finite(),
                "fill vertex {} is not finite: ({}, {})",
                i,
                v.x,
                v.y
            );
        }

        // Verify all indices are in bounds
        for (i, &idx) in fill.indices.iter().enumerate() {
            assert!(
                (idx as usize) < fill.vertices.len(),
                "fill index {} = {} out of bounds (verts={})",
                i,
                idx,
                fill.vertices.len()
            );
        }

        // The polygon should include curve nodes (not just boundary corners)
        let has_node_at_y3 = fill.vertices.iter().any(|v| (v.y - 3.0).abs() < 0.01);
        assert!(has_node_at_y3, "fill should include curve node at y=3.0");

        // Compute triangulated area vs polygon area — they should match
        let tri_count = fill.indices.len() / 3;
        let mut tri_area = 0.0_f64;
        for t in 0..tri_count {
            let a = fill.vertices[fill.indices[t * 3] as usize];
            let b = fill.vertices[fill.indices[t * 3 + 1] as usize];
            let c = fill.vertices[fill.indices[t * 3 + 2] as usize];
            let area = ((b.x - a.x) as f64 * (c.y - a.y) as f64
                - (c.x - a.x) as f64 * (b.y - a.y) as f64)
                .abs()
                * 0.5;
            tri_area += area;
        }
        let poly_area = signed_area(&fill.vertices).abs();
        let rel_err = (tri_area - poly_area).abs() / poly_area;
        assert!(
            rel_err < 0.01,
            "fill triangulation area error: tri={:.4} poly={:.4} rel={:.6}",
            tri_area,
            poly_area,
            rel_err
        );
    }

    #[test]
    fn same_edge_fill_not_self_intersecting() {
        // Both start and end nodes project to the SAME boundary edge (bottom).
        // Previously, this caused the corner walk to add ALL 4 corners, creating
        // a self-intersecting polygon.
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 3.7, y: -18.1 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.7, y: -10.8 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.2, y: -9.4 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.1, y: -6.1 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 5.0, y: -3.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 9.0, y: -18.1 },
                texture: 0,
            },
        ];
        let boundary = [-0.76, -18.63, 9.82, 6.63];
        let fill = rebuild_fill_mesh(&nodes, boundary);

        // The polygon should NOT have all 4 boundary corners
        let boundary_corners = [
            Vec2 { x: 9.82, y: -18.63 },
            Vec2 {
                x: -0.76,
                y: -18.63,
            },
            Vec2 { x: -0.76, y: 6.63 },
            Vec2 { x: 9.82, y: 6.63 },
        ];
        let corner_count = boundary_corners
            .iter()
            .filter(|c| {
                fill.vertices
                    .iter()
                    .any(|v| (v.x - c.x).abs() < 0.01 && (v.y - c.y).abs() < 0.01)
            })
            .count();
        assert!(
            corner_count < 4,
            "should NOT include all 4 boundary corners when both endpoints project to same edge, got {} corners",
            corner_count
        );

        // The polygon should not self-intersect
        let si = count_self_intersections(&fill.vertices);
        assert_eq!(si, 0, "fill polygon should not self-intersect");

        // Should still have valid triangulation
        assert!(fill.indices.len() >= 9, "should have valid triangulation");
        for &idx in &fill.indices {
            assert!(
                (idx as usize) < fill.vertices.len(),
                "index {} out of bounds (verts={})",
                idx,
                fill.vertices.len()
            );
        }
    }
}
