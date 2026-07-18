//! Vector & geometric math helpers used across the terrain mesh pipeline.

use crate::domain::types::Vec2;

// ── Vector math ──

pub(super) fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2 {
        x: a.x - b.x,
        y: a.y - b.y,
    }
}

pub(super) fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

pub(super) fn dist_sq(a: Vec2, b: Vec2) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}

pub(super) fn normalize(v: Vec2) -> Vec2 {
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

pub(super) fn cross2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

// ── Polygon area ──

/// Compute signed area of a polygon. Positive = CCW, negative = CW.
pub(super) fn signed_area(polygon: &[Vec2]) -> f64 {
    let n = polygon.len();
    let mut area = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += polygon[i].x as f64 * polygon[j].y as f64;
        area -= polygon[j].x as f64 * polygon[i].y as f64;
    }
    area * 0.5
}

// ── Point/triangle/segment tests ──

/// Test if point P is inside triangle (A, B, C) using cross products.
pub(super) fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
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

/// Strict point-in-triangle test (excludes edges).
pub(super) fn point_in_triangle_strict(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross2d(sub(b, a), sub(p, a));
    let d2 = cross2d(sub(c, b), sub(p, b));
    let d3 = cross2d(sub(a, c), sub(p, c));
    // All same sign (strictly inside)
    (d1 > 0.0 && d2 > 0.0 && d3 > 0.0) || (d1 < 0.0 && d2 < 0.0 && d3 < 0.0)
}

/// Project point P onto line segment AB.
pub(super) fn project_onto_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
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
pub(super) fn point_to_segment_dist_sq(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let proj = project_onto_segment(p, a, b);
    dist_sq(p, proj)
}

// ── Segment intersections ──

/// Reimplements `e2dUtils.SegmentsIntersect` from the Unity C# source:
/// uses Cramer's rule for the intersection of the two infinite lines, then
/// checks that the intersection lies within the bounding box of each segment
/// (with a small epsilon for near-axis-aligned segments).
pub(super) fn segments_intersect_point(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
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

/// Test if two line segments strictly cross (not just touch at endpoints).
pub(super) fn edges_cross(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = cross2d(sub(b2, b1), sub(a1, b1));
    let d2 = cross2d(sub(b2, b1), sub(a2, b1));
    let d3 = cross2d(sub(a2, a1), sub(b1, a1));
    let d4 = cross2d(sub(a2, a1), sub(b2, a1));
    // Strict crossing: opposite signs on both pairs
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Count the number of self-intersections in a polygon (edges crossing).
pub(super) fn count_self_intersections(polygon: &[Vec2]) -> usize {
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
