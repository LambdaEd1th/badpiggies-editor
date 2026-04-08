//! Terrain mesh generation — rebuilds curve mesh, fill mesh, and control texture
//! from curve node positions and texture indices.
//!
//! Implements the Unity e2d algorithms:
//! - `compute_stripe_vertices`: bisector normals × strip width
//! - `fix_self_intersections`: collapse crossing stripe edges
//! - `triangulate_strip`: quad-strip indices with bowtie detection
//! - `ear_clip_triangulate`: fill polygon from boundary + curve nodes
//! - `encode_control_png`: 1×N RGBA PNG from node texture indices

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
                    if gi < px.len() && px[gi] > 128 {
                        1
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            CurveNode { position: pos, texture }
        })
        .collect()
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

    // Compute stripe (inner) vertices using bisector normals
    let mut stripe_verts: Vec<Vec2> = Vec::with_capacity(n);
    for i in 0..n {
        let strip_w = strip_widths
            .get(nodes[i].texture)
            .copied()
            .unwrap_or(0.5);
        let inner = if i == 0 {
            compute_first_stripe_vertex(nodes, strip_w)
        } else if i == n - 1 {
            compute_last_stripe_vertex(nodes, strip_w)
        } else {
            compute_interior_stripe_vertex(nodes, i, strip_w)
        };
        stripe_verts.push(inner);
    }

    // Fix self-intersections in the stripe polyline
    fix_self_intersections(&mut stripe_verts);

    // Build interleaved vertex array: [outer0, inner0, outer1, inner1, ...]
    let mut vertices = Vec::with_capacity(n * 2);
    for i in 0..n {
        vertices.push(nodes[i].position);
        vertices.push(stripe_verts[i]);
    }

    // Triangulate the quad strip with bowtie detection
    let indices = triangulate_strip(&vertices);

    TerrainMesh { vertices, indices }
}

/// Compute stripe vertex for the first node.
/// Uses the tangent from node[0] to node[1].
fn compute_first_stripe_vertex(nodes: &[CurveNode], strip_w: f32) -> Vec2 {
    let tangent = sub(nodes[1].position, nodes[0].position);
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

    // Left-perpendicular of each edge: (y, -x)
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

/// Fix self-intersections in the stripe polyline by collapsing crossing segments.
fn fix_self_intersections(stripe: &mut [Vec2]) {
    let n = stripe.len();
    if n < 4 {
        return;
    }
    for j in 0..n - 1 {
        for k in (j + 2)..n - 1 {
            if let Some(v) = segments_intersect(stripe[j], stripe[j + 1], stripe[k], stripe[k + 1])
            {
                for item in stripe.iter_mut().take(k + 1).skip(j + 1) {
                    *item = v;
                }
                break; // only fix first crossing per starting segment
            }
        }
    }
}

/// Test if two line segments intersect and return the intersection point.
fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
    let d1 = sub(a2, a1);
    let d2 = sub(b2, b1);
    let denom = d1.x * d2.y - d1.y * d2.x;
    if denom.abs() < 1e-10 {
        return None; // parallel
    }
    let d = sub(b1, a1);
    let t = (d.x * d2.y - d.y * d2.x) / denom;
    let u = (d.x * d1.y - d.y * d1.x) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(Vec2 {
            x: a1.x + t * d1.x,
            y: a1.y + t * d1.y,
        })
    } else {
        None
    }
}

/// Triangulate a quad strip with bowtie detection.
/// Vertices are interleaved: [outer0, inner0, outer1, inner1, ...].
/// Returns indices as i16.
fn triangulate_strip(verts: &[Vec2]) -> Vec<i16> {
    let pair_count = verts.len() / 2;
    if pair_count < 2 {
        return Vec::new();
    }
    let mut indices = Vec::with_capacity((pair_count - 1) * 6);

    for i in 1..pair_count {
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
    let is_closed = dist_sq(nodes[0].position, nodes[nodes.len() - 1].position) < 0.5 * 0.5;

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

    // Walk boundary corners from end edge to start edge (CCW)
    let mut edge = end_edge;
    loop {
        polygon.push(corners[(edge + 1) % 4]);
        edge = (edge + 1) % 4;
        if edge == start_edge {
            break;
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

/// Ear-clip triangulation of a simple polygon.
/// Returns indices (i16) with reversed winding (matching Unity's CW convention).
fn ear_clip_triangulate(polygon: &[Vec2]) -> Vec<i16> {
    let n = polygon.len();
    if n < 3 {
        return Vec::new();
    }

    // Determine winding: positive area = CCW
    let area = signed_area(polygon);
    let mut idx: Vec<usize> = if area > 0.0 {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };

    let mut result = Vec::with_capacity((n - 2) * 3);
    let mut remaining = n;
    let mut max_iter = 2 * remaining;
    let mut v = remaining - 1;

    while remaining > 2 {
        if max_iter == 0 {
            break; // degenerate polygon
        }
        max_iter -= 1;

        let u = if v >= remaining { 0 } else { v };
        v = if u + 1 >= remaining { 0 } else { u + 1 };
        let w = if v + 1 >= remaining { 0 } else { v + 1 };

        if is_ear(polygon, &idx, u, v, w, remaining) {
            result.push(idx[u] as i16);
            result.push(idx[v] as i16);
            result.push(idx[w] as i16);

            // Remove vertex v
            for j in (v + 1)..remaining {
                idx[j - 1] = idx[j];
            }
            remaining -= 1;
            max_iter = 2 * remaining;
        }
    }

    // Reverse winding to match Unity convention
    result.reverse();
    result
}

/// Test if triangle (u, v, w) is a valid ear.
fn is_ear(polygon: &[Vec2], idx: &[usize], u: usize, v: usize, w: usize, n: usize) -> bool {
    let a = polygon[idx[u]];
    let b = polygon[idx[v]];
    let c = polygon[idx[w]];

    // Triangle must have positive area (CCW winding)
    let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if cross < 1e-6 {
        return false;
    }

    // No other vertex must be inside this triangle
    for p in 0..n {
        if p == u || p == v || p == w {
            continue;
        }
        if point_in_triangle(polygon[idx[p]], a, b, c) {
            return false;
        }
    }
    true
}

/// Signed area of a polygon (positive = CCW).
fn signed_area(polygon: &[Vec2]) -> f32 {
    let n = polygon.len();
    let mut area = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        area += (polygon[j].x - polygon[i].x) * (polygon[j].y + polygon[i].y);
        j = i;
    }
    area * 0.5
}

/// Encode node texture indices into a 1×N PNG (control texture).
/// Returns raw PNG bytes.
pub fn encode_control_png(nodes: &[CurveNode]) -> Vec<u8> {
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
        .expect("PNG encode failed");
    }
    buf
}

/// Infer the boundary rectangle from the existing fill mesh vertices.
/// Returns `[min_x, min_y, max_x, max_y]`.
pub fn infer_boundary(td: &TerrainData) -> [f32; 4] {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for v in &td.fill_mesh.vertices {
        min_x = min_x.min(v.x);
        min_y = min_y.min(v.y);
        max_x = max_x.max(v.x);
        max_y = max_y.max(v.y);
    }

    // Fallback: use curve mesh vertices if fill mesh is empty
    if min_x > max_x {
        for v in &td.curve_mesh.vertices {
            min_x = min_x.min(v.x);
            min_y = min_y.min(v.y);
            max_x = max_x.max(v.x);
            max_y = max_y.max(v.y);
        }
    }

    // Add margin
    let margin = 0.5;
    [min_x - margin, min_y - margin, max_x + margin, max_y + margin]
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

    // Rebuild fill mesh using inferred boundary
    let boundary = infer_boundary(td);
    td.fill_mesh = rebuild_fill_mesh(nodes, boundary);

    // Rebuild control texture
    td.control_texture_data = Some(encode_control_png(nodes));
    td.control_texture_count = 1;
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
        let png = encode_control_png(&nodes);
        let pixels = decode_control_png_pixels(&png).unwrap();
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

    #[test]
    fn self_intersection_collapse() {
        // Two segments that cross
        let mut stripe = vec![
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 1.0, y: 1.0 },
            Vec2 { x: 1.0, y: 0.0 },
            Vec2 { x: 0.0, y: 1.0 },
        ];
        fix_self_intersections(&mut stripe);
        // Middle two vertices should be collapsed to intersection point ~(0.5, 0.5)
        assert!((stripe[1].x - 0.5).abs() < 0.01);
        assert!((stripe[1].y - 0.5).abs() < 0.01);
        assert!((stripe[2].x - 0.5).abs() < 0.01);
        assert!((stripe[2].y - 0.5).abs() < 0.01);
    }
}
