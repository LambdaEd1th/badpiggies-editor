//! Terrain preset placement + freehand draw handlers.

use eframe::egui;

use crate::domain::types::Vec2;

use super::super::{DrawTerrainResult, LevelRenderer, TerrainDrawMode, TerrainPresetShape};
use super::terrain_preset_points;

fn constrain_draw_point(mode: TerrainDrawMode, anchor: Vec2, candidate: Vec2) -> Vec2 {
    match mode {
        TerrainDrawMode::Curve | TerrainDrawMode::CircularArc | TerrainDrawMode::Free => candidate,
        TerrainDrawMode::Horizontal => Vec2 {
            x: candidate.x,
            y: anchor.y,
        },
        TerrainDrawMode::Vertical => Vec2 {
            x: anchor.x,
            y: candidate.y,
        },
    }
}

fn sample_quadratic_conic(p0: Vec2, p1: Vec2, p2: Vec2, nodes: usize) -> Vec<Vec2> {
    let n = nodes.clamp(3, 256);
    let mut points = Vec::with_capacity(n);
    let denom = (n - 1) as f32;
    for i in 0..n {
        let t = i as f32 / denom;
        let omt = 1.0 - t;
        points.push(Vec2 {
            x: omt * omt * p0.x + 2.0 * omt * t * p1.x + t * t * p2.x,
            y: omt * omt * p0.y + 2.0 * omt * t * p1.y + t * t * p2.y,
        });
    }
    points
}

fn circle_center_from_three_points(p0: Vec2, p1: Vec2, p2: Vec2) -> Option<Vec2> {
    let x1 = p0.x;
    let y1 = p0.y;
    let x2 = p1.x;
    let y2 = p1.y;
    let x3 = p2.x;
    let y3 = p2.y;

    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-6 {
        return None;
    }

    let x1_sq_y1_sq = x1 * x1 + y1 * y1;
    let x2_sq_y2_sq = x2 * x2 + y2 * y2;
    let x3_sq_y3_sq = x3 * x3 + y3 * y3;

    let ux = (x1_sq_y1_sq * (y2 - y3) + x2_sq_y2_sq * (y3 - y1) + x3_sq_y3_sq * (y1 - y2)) / d;
    let uy = (x1_sq_y1_sq * (x3 - x2) + x2_sq_y2_sq * (x1 - x3) + x3_sq_y3_sq * (x2 - x1)) / d;
    Some(Vec2 { x: ux, y: uy })
}

fn angle_delta_ccw(from: f32, to: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (to - from).rem_euclid(tau)
}

fn sample_circular_arc(p0: Vec2, through: Vec2, p2: Vec2, nodes: usize) -> Option<Vec<Vec2>> {
    let center = circle_center_from_three_points(p0, through, p2)?;
    let radius = ((p0.x - center.x).powi(2) + (p0.y - center.y).powi(2)).sqrt();
    if radius < 1e-6 {
        return None;
    }

    let a0 = (p0.y - center.y).atan2(p0.x - center.x);
    let a1 = (through.y - center.y).atan2(through.x - center.x);
    let a2 = (p2.y - center.y).atan2(p2.x - center.x);

    let sweep = {
        let ccw_01 = angle_delta_ccw(a0, a1);
        let ccw_02 = angle_delta_ccw(a0, a2);
        if ccw_01 <= ccw_02 {
            ccw_02
        } else {
            -angle_delta_ccw(a2, a0)
        }
    };

    let n = nodes.clamp(3, 256);
    let mut points = Vec::with_capacity(n);
    let denom = (n - 1) as f32;
    for i in 0..n {
        let t = i as f32 / denom;
        let angle = a0 + sweep * t;
        points.push(Vec2 {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
        });
    }
    Some(points)
}

fn sample_curve_mode_points(
    mode: TerrainDrawMode,
    start: Vec2,
    end: Vec2,
    control: Vec2,
    segments: usize,
) -> Vec<Vec2> {
    if mode == TerrainDrawMode::CircularArc {
        sample_circular_arc(start, control, end, segments)
            .unwrap_or_else(|| sample_quadratic_conic(start, control, end, segments))
    } else {
        sample_quadratic_conic(start, control, end, segments)
    }
}

impl LevelRenderer {
    pub(super) fn handle_terrain_preset_mode(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        shape: TerrainPresetShape,
        is_shift: bool,
        is_alt: bool,
    ) {
        if response.secondary_clicked() {
            self.terrain_preset_drag_start = None;
            self.terrain_preset_shape = None;
            self.suppress_context_menu_this_frame = true;
            self.panning = false;
            return;
        }

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.terrain_preset_drag_start = None;
            self.terrain_preset_shape = None;
            self.panning = false;
            return;
        }

        if response.drag_started_by(egui::PointerButton::Primary)
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            self.terrain_preset_drag_start =
                Some(self.camera.screen_to_world(pointer, canvas_center));
        }

        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(start) = self.terrain_preset_drag_start.take()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let end = self.camera.screen_to_world(pointer, canvas_center);
            let width = (end.x - start.x).abs();
            let height = (end.y - start.y).abs();
            if width > 0.05 && height > 0.05 {
                self.draw_terrain_result = Some(DrawTerrainResult {
                    points: terrain_preset_points(shape, start, end, self.terrain_curve_segments),
                    closed: true,
                    texture_index: self.terrain_draw_texture_index,
                });
            }
            self.terrain_preset_shape = None;
        }

        self.panning = false;
    }

    /// Draw-terrain mode: click to place individual points.
    /// Close the curve by clicking near the first point (when ≥3 points exist),
    /// or press Enter to finish as open curve, Escape to cancel.
    pub(super) fn handle_draw_terrain_mode(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        _rect: egui::Rect,
        is_shift: bool,
        is_alt: bool,
    ) {
        if self.draw_terrain_points.is_empty()
            && let Some(anchor) = self.terrain_draw_continuation_anchor
        {
            self.draw_terrain_points.push(anchor);
            self.draw_terrain_active = true;
        }

        // Middle-mouse / shift+drag / alt+drag → pan
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && is_shift)
            || (response.dragged_by(egui::PointerButton::Primary) && is_alt)
        {
            let delta = response.drag_delta();
            self.camera.center.x -= delta.x / self.camera.zoom;
            self.camera.center.y += delta.y / self.camera.zoom;
            self.panning = true;
            return;
        }

        let close_threshold = 12.0 / self.camera.zoom; // 12 screen-px in world units

        if let Some(shape) = self.terrain_preset_shape {
            self.handle_terrain_preset_mode(ui, response, canvas_center, shape, is_shift, is_alt);
            return;
        }

        // Curve modes: 3 clicks define a normal conic or a true circular arc.
        if matches!(
            self.terrain_draw_mode,
            TerrainDrawMode::Curve | TerrainDrawMode::CircularArc
        ) {
            if response.secondary_clicked() && !self.draw_terrain_points.is_empty() {
                self.draw_terrain_points.pop();
                self.draw_terrain_active = !self.draw_terrain_points.is_empty();
                if self.draw_terrain_points.is_empty() {
                    self.clear_terrain_draw_continuation_anchor();
                }
                self.suppress_context_menu_this_frame = true;
                self.panning = false;
                return;
            }

            if response.clicked()
                && !is_shift
                && !is_alt
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world = self.camera.screen_to_world(pointer, canvas_center);
                self.draw_terrain_points.push(world);
                self.draw_terrain_active = true;

                if self.draw_terrain_points.len() == 3 {
                    let start = self.draw_terrain_points[0];
                    let end = self.draw_terrain_points[1];
                    let control = self.draw_terrain_points[2];
                    let points = sample_curve_mode_points(
                        self.terrain_draw_mode,
                        start,
                        end,
                        control,
                        self.terrain_curve_segments,
                    );
                    self.draw_terrain_result = Some(DrawTerrainResult {
                        points,
                        closed: false,
                        texture_index: self.terrain_draw_texture_index,
                    });
                    self.draw_terrain_points.clear();
                    self.draw_terrain_active = false;
                    self.panning = false;
                    return;
                }
            }

            if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let anchor_only = self.terrain_draw_continuation_anchor.is_some()
                    && self.draw_terrain_points.len() == 1;
                self.draw_terrain_active = false;

                if self.draw_terrain_points.len() >= 2 {
                    if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
                        let start = self.draw_terrain_points[0];
                        let end = self.draw_terrain_points[1];
                        let control = self.camera.screen_to_world(pointer, canvas_center);
                        let points = sample_curve_mode_points(
                            self.terrain_draw_mode,
                            start,
                            end,
                            control,
                            self.terrain_curve_segments,
                        );
                        self.draw_terrain_result = Some(DrawTerrainResult {
                            points,
                            closed: false,
                            texture_index: self.terrain_draw_texture_index,
                        });
                    }
                    self.draw_terrain_points.clear();
                } else {
                    self.draw_terrain_points.clear();
                    if anchor_only {
                        self.clear_terrain_draw_continuation_anchor();
                        self.clicked_empty = true;
                    }
                }

                self.panning = false;
                return;
            }

            if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.draw_terrain_active = false;
                self.draw_terrain_points.clear();
                self.clear_terrain_draw_continuation_anchor();
            }

            self.panning = false;
            return;
        }

        // Right-click while drawing removes the most recently placed point.
        if response.secondary_clicked() && !self.draw_terrain_points.is_empty() {
            self.draw_terrain_points.pop();
            self.draw_terrain_active = !self.draw_terrain_points.is_empty();
            if self.draw_terrain_points.is_empty() {
                self.clear_terrain_draw_continuation_anchor();
            }
            self.suppress_context_menu_this_frame = true;
            self.panning = false;
            return;
        }

        // Click to place a point
        if response.clicked()
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let mut world = self.camera.screen_to_world(pointer, canvas_center);
            if let Some(last) = self.draw_terrain_points.last().copied() {
                world = constrain_draw_point(self.terrain_draw_mode, last, world);
            }

            // Check if closing: ≥3 points and click is near the first point
            if self.draw_terrain_points.len() >= 3 {
                let first = self.draw_terrain_points[0];
                let dx = world.x - first.x;
                let dy = world.y - first.y;
                if dx * dx + dy * dy < close_threshold * close_threshold {
                    // Close the curve: duplicate first point as last
                    let close_pt = first;
                    self.draw_terrain_points.push(close_pt);
                    self.draw_terrain_active = false;
                    self.draw_terrain_result = Some(DrawTerrainResult {
                        points: std::mem::take(&mut self.draw_terrain_points),
                        closed: true,
                        texture_index: self.terrain_draw_texture_index,
                    });
                    self.panning = false;
                    return;
                }
            }

            // Normal point placement
            let should_add_point = self
                .draw_terrain_points
                .last()
                .map(|last| {
                    let dx = world.x - last.x;
                    let dy = world.y - last.y;
                    dx * dx + dy * dy > 1e-8
                })
                .unwrap_or(true);
            if should_add_point {
                self.draw_terrain_points.push(world);
                self.draw_terrain_active = true;
            }
        }

        // Enter — finish as open curve. If we're sitting on a continuation
        // anchor with no new segment yet, treat Enter as "done with this terrain"
        // and clear the current selection via the existing clicked_empty path.
        if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let anchor_only = self.terrain_draw_continuation_anchor.is_some()
                && self.draw_terrain_points.len() == 1;
            self.draw_terrain_active = false;
            if self.draw_terrain_points.len() >= 2 {
                self.draw_terrain_result = Some(DrawTerrainResult {
                    points: std::mem::take(&mut self.draw_terrain_points),
                    closed: false,
                    texture_index: self.terrain_draw_texture_index,
                });
            } else {
                self.draw_terrain_points.clear();
                if anchor_only {
                    self.clear_terrain_draw_continuation_anchor();
                    self.clicked_empty = true;
                }
            }
        }

        // Escape — cancel drawing
        if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.draw_terrain_active = false;
            self.draw_terrain_points.clear();
            self.clear_terrain_draw_continuation_anchor();
        }

        self.panning = false;
    }
}
