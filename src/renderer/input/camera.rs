//! Camera pan / zoom + bounds-rect drag.

use eframe::egui;

use crate::types::Vec2;

use super::super::{BoundsDragResult, BoundsDragState, BoundsHandle, LevelRenderer};

impl LevelRenderer {
    pub(super) fn handle_pan_mode(&mut self, response: &egui::Response, _is_shift: bool, _is_alt: bool) {
        if response.dragged_by(egui::PointerButton::Primary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let delta = response.drag_delta();
            self.camera.center.x -= delta.x / self.camera.zoom;
            self.camera.center.y += delta.y / self.camera.zoom;
            self.panning = true;
        } else {
            self.panning = false;
        }
    }

    /// Handle zoom (scroll wheel + pinch-to-zoom). Shared across all modes.
    pub(super) fn handle_zoom(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        if !response.hovered() {
            return;
        }
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(pointer) = response.hover_pos() {
                let world_before = self.camera.screen_to_world(pointer, canvas_center);
                let factor = 1.0 + scroll * 0.002;
                self.camera.zoom = (self.camera.zoom * factor).clamp(2.0, 500.0);
                let world_after = self.camera.screen_to_world(pointer, canvas_center);
                self.camera.center.x -= world_after.x - world_before.x;
                self.camera.center.y -= world_after.y - world_before.y;
            } else {
                let factor = 1.0 + scroll * 0.002;
                self.camera.zoom = (self.camera.zoom * factor).clamp(2.0, 500.0);
            }
        }

        // Pinch-to-zoom on touch devices (mobile WASM)
        if let Some(touch) = ui.input(|i| i.multi_touch()) {
            if (touch.zoom_delta - 1.0).abs() > 0.001 {
                let center = rect.center();
                let world_before = self
                    .camera
                    .screen_to_world(egui::pos2(center.x, center.y), canvas_center);
                self.camera.zoom = (self.camera.zoom * touch.zoom_delta).clamp(2.0, 500.0);
                let world_after = self
                    .camera
                    .screen_to_world(egui::pos2(center.x, center.y), canvas_center);
                self.camera.center.x -= world_after.x - world_before.x;
                self.camera.center.y -= world_after.y - world_before.y;
            }
            let td = touch.translation_delta;
            if td.x.abs() > 0.5 || td.y.abs() > 0.5 {
                self.camera.center.x -= td.x / self.camera.zoom;
                self.camera.center.y += td.y / self.camera.zoom;
            }
        }
    }

    /// Handle level-bounds drag interaction. Returns `true` if a bounds drag
    /// is active or was just started (caller should skip normal mode handling).
    pub(super) fn handle_bounds_drag(&mut self, response: &egui::Response, canvas_center: egui::Vec2) -> bool {
        // Check if bounds are visible and exist
        if !self.show_level_bounds {
            self.bounds_hovered_handle = None;
            return false;
        }
        let limits = match self.camera_limits {
            Some(v) => v,
            None => {
                self.bounds_hovered_handle = None;
                return false;
            }
        };

        let edge_threshold = 8.0; // screen pixels

        // Compute screen-space rectangle for the bounds
        let [tl_x, tl_y, w, h] = limits;
        let p_tl = self
            .camera
            .world_to_screen(Vec2 { x: tl_x, y: tl_y }, canvas_center);
        let p_br = self.camera.world_to_screen(
            Vec2 {
                x: tl_x + w,
                y: tl_y - h,
            },
            canvas_center,
        );

        // Continue active drag
        if let Some(ref drag) = self.bounds_dragging {
            if response.dragged_by(egui::PointerButton::Primary) {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let current = self.camera.screen_to_world(pointer, canvas_center);
                    let dx = current.x - drag.start_mouse.x;
                    let dy = current.y - drag.start_mouse.y;
                    let [otl_x, otl_y, ow, oh] = drag.original_limits;
                    let new_limits = match drag.handle {
                        BoundsHandle::Move => [otl_x + dx, otl_y + dy, ow, oh],
                        BoundsHandle::Left => {
                            let nw = (ow - dx).max(1.0);
                            [otl_x + ow - nw, otl_y, nw, oh]
                        }
                        BoundsHandle::Right => [otl_x, otl_y, (ow + dx).max(1.0), oh],
                        BoundsHandle::Top => {
                            let nh = (oh + dy).max(1.0);
                            [
                                otl_x,
                                otl_y + dy - (nh - oh - dy),
                                nh.max(1.0).min(oh + dy).max(1.0),
                                oh,
                            ]
                        }
                        BoundsHandle::Bottom => [otl_x, otl_y, ow, (oh - dy).max(1.0)],
                        _ => {
                            // Compute for corner handles
                            self.compute_corner_drag(drag.handle, otl_x, otl_y, ow, oh, dx, dy)
                        }
                    };
                    // Fix Top handle: topLeft.y moves with dy, height grows with dy
                    let new_limits = match drag.handle {
                        BoundsHandle::Top => {
                            let nh = (oh + dy).max(1.0);
                            [otl_x, otl_y + (nh - oh), ow, nh]
                        }
                        _ => new_limits,
                    };
                    self.camera_limits = Some(new_limits);
                }
                return true;
            }
            // Drag ended
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                let new_limits = self.camera_limits.unwrap_or(drag.original_limits);
                self.bounds_dragging = None;
                self.bounds_drag_result = Some(BoundsDragResult { new_limits });
                return false;
            }
        }

        // Hover detection + start drag
        let handle = if let Some(pointer) = response.hover_pos() {
            self.detect_bounds_handle(pointer, p_tl, p_br, edge_threshold)
        } else {
            None
        };
        self.bounds_hovered_handle = handle;

        if let Some(handle) = handle
            && response.drag_started_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let world = self.camera.screen_to_world(pointer, canvas_center);
            self.bounds_dragging = Some(BoundsDragState {
                handle,
                start_mouse: world,
                original_limits: limits,
            });
            return true;
        }

        handle.is_some() && response.dragged_by(egui::PointerButton::Primary)
    }

    /// Detect which bounds handle (if any) the pointer is hovering.
    fn detect_bounds_handle(
        &self,
        pointer: egui::Pos2,
        p_tl: egui::Pos2,
        p_br: egui::Pos2,
        threshold: f32,
    ) -> Option<BoundsHandle> {
        let left = p_tl.x;
        let right = p_br.x;
        let top = p_tl.y;
        let bottom = p_br.y;

        let near_left = (pointer.x - left).abs() < threshold;
        let near_right = (pointer.x - right).abs() < threshold;
        let near_top = (pointer.y - top).abs() < threshold;
        let near_bottom = (pointer.y - bottom).abs() < threshold;

        let in_x = pointer.x >= left - threshold && pointer.x <= right + threshold;
        let in_y = pointer.y >= top - threshold && pointer.y <= bottom + threshold;

        // Corners first (more specific)
        if near_left && near_top {
            return Some(BoundsHandle::TopLeft);
        }
        if near_right && near_top {
            return Some(BoundsHandle::TopRight);
        }
        if near_left && near_bottom {
            return Some(BoundsHandle::BottomLeft);
        }
        if near_right && near_bottom {
            return Some(BoundsHandle::BottomRight);
        }

        // Edges
        if near_left && in_y {
            return Some(BoundsHandle::Left);
        }
        if near_right && in_y {
            return Some(BoundsHandle::Right);
        }
        if near_top && in_x {
            return Some(BoundsHandle::Top);
        }
        if near_bottom && in_x {
            return Some(BoundsHandle::Bottom);
        }

        // Interior → move
        if pointer.x > left + threshold
            && pointer.x < right - threshold
            && pointer.y > top + threshold
            && pointer.y < bottom - threshold
        {
            return Some(BoundsHandle::Move);
        }

        None
    }

    /// Compute new limits for a corner drag handle.
    fn compute_corner_drag(
        &self,
        handle: BoundsHandle,
        tl_x: f32,
        tl_y: f32,
        w: f32,
        h: f32,
        dx: f32,
        dy: f32,
    ) -> [f32; 4] {
        match handle {
            BoundsHandle::TopLeft => {
                let nw = (w - dx).max(1.0);
                let nh = (h + dy).max(1.0);
                [tl_x + w - nw, tl_y + (nh - h), nw, nh]
            }
            BoundsHandle::TopRight => {
                let nw = (w + dx).max(1.0);
                let nh = (h + dy).max(1.0);
                [tl_x, tl_y + (nh - h), nw, nh]
            }
            BoundsHandle::BottomLeft => {
                let nw = (w - dx).max(1.0);
                let nh = (h - dy).max(1.0);
                [tl_x + w - nw, tl_y, nw, nh]
            }
            BoundsHandle::BottomRight => {
                let nw = (w + dx).max(1.0);
                let nh = (h - dy).max(1.0);
                [tl_x, tl_y, nw, nh]
            }
            _ => [tl_x, tl_y, w, h],
        }
    }
}
