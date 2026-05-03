//! Overlay key, transform helpers, and 1D interval arithmetic for scanline rasterization.

use eframe::egui;

use super::super::Camera;
use super::DarkOverlayKey;

pub(in crate::renderer) fn overlay_key(camera: &Camera, rect: egui::Rect) -> DarkOverlayKey {
    (
        camera.center.x,
        camera.center.y,
        camera.zoom,
        rect.center().x,
        rect.center().y,
        rect.width(),
        rect.height(),
    )
}

pub(in crate::renderer) fn can_transform_overlay(from: DarkOverlayKey, to: DarkOverlayKey) -> bool {
    (from.5 - to.5).abs() < 0.5 && (from.6 - to.6).abs() < 0.5 && from.2.abs() > f32::EPSILON
}

fn transform_overlay_pos(pos: egui::Pos2, from: DarkOverlayKey, to: DarkOverlayKey) -> egui::Pos2 {
    let (from_cam_x, from_cam_y, from_zoom, from_cx, from_cy, _, _) = from;
    let (to_cam_x, to_cam_y, to_zoom, to_cx, to_cy, _, _) = to;
    let scale = to_zoom / from_zoom;
    egui::pos2(
        to_cx + (pos.x - from_cx) * scale + (from_cam_x - to_cam_x) * to_zoom,
        to_cy + (pos.y - from_cy) * scale + (to_cam_y - from_cam_y) * to_zoom,
    )
}

pub(in crate::renderer) fn transformed_overlay_mesh(
    mesh: &egui::Mesh,
    from: DarkOverlayKey,
    to: DarkOverlayKey,
) -> egui::Mesh {
    let mut transformed = mesh.clone();
    for vertex in &mut transformed.vertices {
        vertex.pos = transform_overlay_pos(vertex.pos, from, to);
    }
    transformed
}

pub(super) fn polygon_intervals_at_y(y: f32, poly: &[egui::Pos2]) -> Vec<(f32, f32)> {
    let mut xs = Vec::new();
    let n = poly.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (y1, y2) = (poly[i].y, poly[j].y);
        if (y1 < y && y2 >= y) || (y2 < y && y1 >= y) {
            let t = (y - y1) / (y2 - y1);
            xs.push(poly[i].x + t * (poly[j].x - poly[i].x));
        }
    }
    xs.sort_by(|a, b| a.total_cmp(b));

    let mut intervals = Vec::new();
    let mut i = 0;
    while i + 1 < xs.len() {
        let left = xs[i];
        let right = xs[i + 1];
        if right - left > 0.1 {
            intervals.push((left, right));
        }
        i += 2;
    }
    intervals
}

pub(super) fn merge_intervals(mut intervals: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    if intervals.is_empty() {
        return intervals;
    }
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut merged = Vec::with_capacity(intervals.len());
    let mut current = intervals[0];
    for interval in intervals.into_iter().skip(1) {
        if interval.0 <= current.1 + 0.1 {
            current.1 = current.1.max(interval.1);
        } else {
            merged.push(current);
            current = interval;
        }
    }
    merged.push(current);
    merged
}

pub(super) fn merged_poly_intervals(y: f32, polys: &[Vec<egui::Pos2>]) -> Vec<(f32, f32)> {
    let mut intervals = Vec::new();
    for poly in polys {
        intervals.extend(polygon_intervals_at_y(y, poly));
    }
    merge_intervals(intervals)
}

pub(super) fn complement_intervals(
    left: f32,
    right: f32,
    intervals: &[(f32, f32)],
) -> Vec<(f32, f32)> {
    let mut result = Vec::new();
    let mut cursor = left;
    for &(start, end) in intervals {
        let start = start.clamp(left, right);
        let end = end.clamp(left, right);
        if start > cursor + 0.1 {
            result.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if right > cursor + 0.1 {
        result.push((cursor, right));
    }
    result
}

pub(super) fn subtract_intervals(outer: &[(f32, f32)], inner: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut result = Vec::new();
    let mut inner_index = 0;

    for &(outer_start, outer_end) in outer {
        let mut cursor = outer_start;
        while inner_index < inner.len() && inner[inner_index].1 <= cursor + 0.1 {
            inner_index += 1;
        }

        let mut j = inner_index;
        while j < inner.len() && inner[j].0 < outer_end - 0.1 {
            let (inner_start, inner_end) = inner[j];
            if inner_start > cursor + 0.1 {
                result.push((cursor, inner_start.min(outer_end)));
            }
            cursor = cursor.max(inner_end);
            if cursor >= outer_end - 0.1 {
                break;
            }
            j += 1;
        }

        if outer_end > cursor + 0.1 {
            result.push((cursor, outer_end));
        }
    }

    result
}
