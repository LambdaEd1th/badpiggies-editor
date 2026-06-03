//! Hit testing, terrain point tests, and geometry helpers for the renderer.

use crate::domain::types::Vec2;

use super::{LevelRenderer, TerrainDrawMode, TerrainPresetShape};

fn constrain_square_end(start: Vec2, end: Vec2) -> Vec2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let side = dx.abs().max(dy.abs());
    let sign_x = if dx < 0.0 { -1.0 } else { 1.0 };
    let sign_y = if dy < 0.0 { -1.0 } else { 1.0 };

    Vec2 {
        x: start.x + sign_x * side,
        y: start.y + sign_y * side,
    }
}

fn constrain_equilateral_triangle_end(start: Vec2, end: Vec2) -> Vec2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let sign_x = if dx < 0.0 { -1.0 } else { 1.0 };
    let sign_y = if dy < 0.0 { -1.0 } else { 1.0 };
    let height_ratio = (3.0_f32).sqrt() * 0.5;
    let side = dx.abs().max(dy.abs() / height_ratio.max(1e-6));

    Vec2 {
        x: start.x + sign_x * side,
        y: start.y + sign_y * side * height_ratio,
    }
}

fn terrain_preset_points(
    shape: TerrainPresetShape,
    start: Vec2,
    end: Vec2,
    curve_segments: usize,
) -> Vec<Vec2> {
    let constrained_end = match shape {
        TerrainPresetShape::PerfectCircle | TerrainPresetShape::Square => {
            constrain_square_end(start, end)
        }
        TerrainPresetShape::EquilateralTriangle => constrain_equilateral_triangle_end(start, end),
        _ => end,
    };

    let min_x = start.x.min(constrained_end.x);
    let max_x = start.x.max(constrained_end.x);
    let min_y = start.y.min(constrained_end.y);
    let max_y = start.y.max(constrained_end.y);

    match shape {
        TerrainPresetShape::Circle | TerrainPresetShape::PerfectCircle => {
            let segments = curve_segments.max(3);
            let center = Vec2 {
                x: (min_x + max_x) * 0.5,
                y: (min_y + max_y) * 0.5,
            };
            let radius_x = (max_x - min_x) * 0.5;
            let radius_y = (max_y - min_y) * 0.5;

            let mut points = Vec::with_capacity(segments + 1);
            for index in 0..segments {
                let angle = index as f32 / segments as f32 * std::f32::consts::TAU;
                points.push(Vec2 {
                    x: center.x + angle.cos() * radius_x,
                    y: center.y + angle.sin() * radius_y,
                });
            }
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
            points
        }
        TerrainPresetShape::Rectangle | TerrainPresetShape::Square => {
            vec![
                Vec2 { x: min_x, y: min_y },
                Vec2 { x: max_x, y: min_y },
                Vec2 { x: max_x, y: max_y },
                Vec2 { x: min_x, y: max_y },
                Vec2 { x: min_x, y: min_y },
            ]
        }
        TerrainPresetShape::EquilateralTriangle => {
            let apex_is_top = constrained_end.y >= start.y;
            let apex_y = if apex_is_top { max_y } else { min_y };
            let base_y = if apex_is_top { min_y } else { max_y };
            let center_x = (min_x + max_x) * 0.5;

            vec![
                Vec2 {
                    x: center_x,
                    y: apex_y,
                },
                Vec2 {
                    x: max_x,
                    y: base_y,
                },
                Vec2 {
                    x: min_x,
                    y: base_y,
                },
                Vec2 {
                    x: center_x,
                    y: apex_y,
                },
            ]
        }
    }
}

/// Point-in-triangle test using barycentric coordinates (sign of cross products).
fn point_in_triangle(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> bool {
    let d1 = (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
    let d2 = (p.x - c.x) * (b.y - c.y) - (b.x - c.x) * (p.y - c.y);
    let d3 = (p.x - a.x) * (c.y - a.y) - (c.x - a.x) * (p.y - a.y);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

/// Distance from point (px,py) to segment (ax,ay)→(bx,by).
/// Returns (distance, t) where t ∈ [0,1] is the projection parameter.
pub(super) fn point_to_segment_dist(
    px: f32,
    py: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> (f32, f32) {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq < 1e-12 {
        0.0
    } else {
        ((px - ax) * dx + (py - ay) * dy) / len_sq
    }
    .clamp(0.0, 1.0);
    let cx = ax + dx * t;
    let cy = ay + dy * t;
    let dist = ((px - cx) * (px - cx) + (py - cy) * (py - cy)).sqrt();
    (dist, t)
}

mod camera;
mod hit_test;
mod interaction;
mod terrain_edit;

impl LevelRenderer {
    pub fn active_terrain_preset(&self) -> Option<TerrainPresetShape> {
        self.terrain_preset_shape
    }

    pub fn terrain_curve_segments(&self) -> usize {
        self.terrain_curve_segments
    }

    pub fn terrain_draw_has_collider(&self) -> bool {
        self.terrain_draw_has_collider
    }

    pub fn set_terrain_draw_has_collider(&mut self, has_collider: bool) {
        self.terrain_draw_has_collider = has_collider;
    }

    pub fn set_terrain_curve_segments(&mut self, segments: usize) {
        self.terrain_curve_segments = segments.clamp(3, 128);
    }

    pub fn terrain_draw_mode(&self) -> TerrainDrawMode {
        self.terrain_draw_mode
    }

    pub fn set_terrain_draw_mode(&mut self, mode: TerrainDrawMode) {
        let mode_changed = self.terrain_draw_mode != mode;
        if mode_changed {
            self.draw_terrain_points.clear();
            self.draw_terrain_active = false;
        }
        self.terrain_draw_mode = mode;

        // Terrain draw modes and preset-shape placement are mutually exclusive.
        if mode != TerrainDrawMode::Free {
            self.terrain_preset_shape = None;
            self.terrain_preset_drag_start = None;
        }

        if mode_changed
            && self.draw_terrain_points.is_empty()
            && let Some(anchor) = self.terrain_draw_continuation_anchor
        {
            self.draw_terrain_points = vec![anchor];
            self.draw_terrain_active = true;
        }
    }

    pub fn terrain_draw_texture_index(&self) -> usize {
        self.terrain_draw_texture_index
    }

    pub fn set_terrain_draw_texture_index(&mut self, texture_index: usize) {
        self.terrain_draw_texture_index = texture_index.min(1);
    }

    pub fn toggle_terrain_preset(&mut self, shape: TerrainPresetShape) {
        self.draw_terrain_points.clear();
        self.draw_terrain_active = false;
        self.terrain_preset_drag_start = None;
        if self.terrain_preset_shape == Some(shape) {
            self.terrain_preset_shape = None;
        } else {
            // Terrain preset placement and draw modes are mutually exclusive.
            self.terrain_draw_mode = TerrainDrawMode::Free;
            self.terrain_preset_shape = Some(shape);
        }
    }

    pub fn has_active_terrain_preset(&self) -> bool {
        self.terrain_preset_shape.is_some()
    }

    pub fn clear_terrain_preset(&mut self) {
        self.terrain_preset_shape = None;
        self.terrain_preset_drag_start = None;
    }

    pub fn set_terrain_draw_continuation_anchor(&mut self, anchor: Option<Vec2>) {
        self.terrain_draw_continuation_anchor = anchor;
        self.draw_terrain_points.clear();
        self.draw_terrain_active = false;

        if let Some(point) = anchor {
            self.draw_terrain_points.push(point);
            self.draw_terrain_active = true;
        }
    }

    pub fn prime_terrain_draw_anchor_if_idle(&mut self, anchor: Vec2) {
        // Allow re-anchoring only when the tool is idle (no active stroke),
        // or when it only has the previously armed anchor point.
        let can_reanchor = self.draw_terrain_points.is_empty()
            || (self.draw_terrain_active && self.draw_terrain_points.len() == 1);
        if !can_reanchor || self.terrain_preset_shape.is_some() {
            return;
        }

        let should_update = self
            .terrain_draw_continuation_anchor
            .map(|current| {
                let dx = current.x - anchor.x;
                let dy = current.y - anchor.y;
                dx * dx + dy * dy > 1e-8
            })
            .unwrap_or(true);
        if !should_update {
            return;
        }

        self.set_terrain_draw_continuation_anchor(Some(anchor));
    }

    pub(super) fn clear_terrain_draw_continuation_anchor(&mut self) {
        self.terrain_draw_continuation_anchor = None;
    }

    pub fn clear_terrain_draw_anchor_if_idle(&mut self) {
        if self.terrain_draw_continuation_anchor.is_none() {
            return;
        }

        let idle_or_anchor_only = self.draw_terrain_points.is_empty()
            || (self.draw_terrain_active && self.draw_terrain_points.len() == 1);
        if !idle_or_anchor_only {
            return;
        }

        self.terrain_draw_continuation_anchor = None;
        self.draw_terrain_points.clear();
        self.draw_terrain_active = false;
    }

    pub(crate) fn terrain_preset_preview_points(&self) -> Option<Vec<Vec2>> {
        let shape = self.terrain_preset_shape?;
        let start = self.terrain_preset_drag_start?;
        let end = self.mouse_world?;
        Some(terrain_preset_points(
            shape,
            start,
            end,
            self.terrain_curve_segments,
        ))
    }
}
