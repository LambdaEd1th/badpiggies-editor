//! Terrain preset placement + freehand draw handlers.

use eframe::egui;

use super::super::{DrawTerrainResult, LevelRenderer, TerrainPresetShape};
use super::terrain_preset_points;

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
            self.terrain_preset_drag_start = Some(self.camera.screen_to_world(pointer, canvas_center));
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
                    points: terrain_preset_points(shape, start, end, self.terrain_round_segments),
                    closed: true,
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
            let world = self.camera.screen_to_world(pointer, canvas_center);

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
                    });
                    self.panning = false;
                    return;
                }
            }

            // Normal point placement
            self.draw_terrain_points.push(world);
            self.draw_terrain_active = true;
        }

        // Enter — finish as open curve
        if self.draw_terrain_active && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.draw_terrain_active = false;
            if self.draw_terrain_points.len() >= 2 {
                self.draw_terrain_result = Some(DrawTerrainResult {
                    points: std::mem::take(&mut self.draw_terrain_points),
                    closed: false,
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
