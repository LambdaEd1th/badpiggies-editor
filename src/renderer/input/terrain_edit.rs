//! Terrain preset placement + freehand draw handlers.

use eframe::egui;

use crate::domain::types::Vec2;

use super::super::{DrawTerrainResult, LevelRenderer, TerrainDrawMode, TerrainPresetShape};
use super::terrain_preset_points;

fn constrain_draw_point(mode: TerrainDrawMode, anchor: Vec2, candidate: Vec2) -> Vec2 {
    match mode {
        TerrainDrawMode::Curve | TerrainDrawMode::Free => candidate,
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

        // Curve mode: 3 clicks define a quadratic conic arc
        if self.terrain_draw_mode == TerrainDrawMode::Curve {
            if response.secondary_clicked() && !self.draw_terrain_points.is_empty() {
                self.draw_terrain_points.pop();
                self.draw_terrain_active = !self.draw_terrain_points.is_empty();
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
                    let points = sample_quadratic_conic(
                        start,
                        control,
                        end,
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

            if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.draw_terrain_active = false;
                self.draw_terrain_points.clear();
            }

            self.panning = false;
            return;
        }

        // Right-click while drawing removes the most recently placed point.
        if response.secondary_clicked() && !self.draw_terrain_points.is_empty() {
            self.draw_terrain_points.pop();
            self.draw_terrain_active = !self.draw_terrain_points.is_empty();
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

        // Enter — finish as open curve
        if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.draw_terrain_active = false;
            if self.draw_terrain_points.len() >= 2 {
                self.draw_terrain_result = Some(DrawTerrainResult {
                    points: std::mem::take(&mut self.draw_terrain_points),
                    closed: false,
                    texture_index: self.terrain_draw_texture_index,
                });
            } else {
                self.draw_terrain_points.clear();
            }
        }

        // Escape — cancel drawing
        if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.draw_terrain_active = false;
            self.draw_terrain_points.clear();
        }

        self.panning = false;
    }
}
