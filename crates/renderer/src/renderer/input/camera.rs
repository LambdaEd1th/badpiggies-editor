//! Camera pan / zoom + bounds-rect drag.

use crate::domain::types::Vec2;

use super::super::{
    BoundsDragResult, BoundsDragState, BoundsEditTarget, BoundsHandle, BoundsHandleHit,
    LevelRenderer, RouteNodeDragResult, RouteNodeDragState, RouteNodeTarget,
};

impl LevelRenderer {
    pub(super) fn handle_pan_mode(
        &mut self,
        response: &crate::gpu2d::Response,
        _is_shift: bool,
        _is_alt: bool,
    ) {
        if response.dragged_by(crate::gpu2d::PointerButton::Primary)
            || response.dragged_by(crate::gpu2d::PointerButton::Middle)
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
        ui: &crate::gpu2d::Ui,
        response: &crate::gpu2d::Response,
        canvas_center: crate::gpu2d::Vec2,
        rect: crate::gpu2d::Rect,
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
                    .screen_to_world(crate::gpu2d::pos2(center.x, center.y), canvas_center);
                self.camera.zoom = (self.camera.zoom * touch.zoom_delta).clamp(2.0, 500.0);
                let world_after = self
                    .camera
                    .screen_to_world(crate::gpu2d::pos2(center.x, center.y), canvas_center);
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
    pub(super) fn handle_bounds_drag(
        &mut self,
        response: &crate::gpu2d::Response,
        canvas_center: crate::gpu2d::Vec2,
    ) -> bool {
        if !self.show_preview_route {
            self.bounds_hovered_handle = None;
            self.route_node_hovered = None;
            return false;
        }

        let edge_threshold = 8.0; // screen pixels
        let node_threshold = 9.0; // screen pixels

        if let Some(drag) = self.bounds_dragging {
            if response.dragged_by(crate::gpu2d::PointerButton::Primary) {
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
                            Self::compute_corner_drag(drag.handle, [otl_x, otl_y, ow, oh], (dx, dy))
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
                    self.set_bounds_for_target(drag.target, new_limits);
                }
                return true;
            }
            if response.drag_stopped_by(crate::gpu2d::PointerButton::Primary) {
                let new_bounds = self
                    .bounds_for_target(drag.target)
                    .unwrap_or(drag.original_limits);
                self.bounds_dragging = None;
                self.bounds_drag_result = Some(BoundsDragResult {
                    target: drag.target,
                    new_bounds,
                });
                return false;
            }
        }

        if let Some(drag) = self.route_node_dragging {
            if response.dragged_by(crate::gpu2d::PointerButton::Primary) {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let current = self.camera.screen_to_world(pointer, canvas_center);
                    let dx = current.x - drag.start_mouse.x;
                    let dy = current.y - drag.start_mouse.y;
                    match drag.target {
                        RouteNodeTarget::InitialView
                        | RouteNodeTarget::CameraLimits
                        | RouteNodeTarget::ConstructionView => {
                            if let Some(target) = Self::route_node_bounds_target(drag.target)
                                && let Some([tl_x, tl_y, w, h]) = drag.original_bounds
                            {
                                self.set_bounds_for_target(target, [tl_x + dx, tl_y + dy, w, h]);
                            }
                        }
                        RouteNodeTarget::CustomPreview(index) => {
                            self.set_custom_route_point(
                                index,
                                Vec2 {
                                    x: drag.original_pos.x + dx,
                                    y: drag.original_pos.y + dy,
                                },
                            );
                        }
                    }
                }
                return true;
            }
            if response.drag_stopped_by(crate::gpu2d::PointerButton::Primary) {
                match drag.target {
                    RouteNodeTarget::InitialView
                    | RouteNodeTarget::CameraLimits
                    | RouteNodeTarget::ConstructionView => {
                        if let Some(target) = Self::route_node_bounds_target(drag.target) {
                            let new_bounds = self
                                .bounds_for_target(target)
                                .or(drag.original_bounds)
                                .unwrap_or([0.0, 0.0, 1.0, 1.0]);
                            self.bounds_drag_result = Some(BoundsDragResult { target, new_bounds });
                        }
                    }
                    RouteNodeTarget::CustomPreview(index) => {
                        let new_position = self
                            .route_node_position(drag.target)
                            .unwrap_or(drag.original_pos);
                        self.route_node_drag_result = Some(RouteNodeDragResult {
                            index,
                            new_position,
                        });
                    }
                }
                self.route_node_dragging = None;
                return false;
            }
        }

        let route_target = response
            .hover_pos()
            .and_then(|pointer| self.detect_route_node(pointer, canvas_center, node_threshold));
        self.route_node_hovered = route_target;
        self.bounds_hovered_handle = None;

        if let Some(target) = route_target {
            if response.drag_started_by(crate::gpu2d::PointerButton::Primary)
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let world = self.camera.screen_to_world(pointer, canvas_center);
                self.route_node_dragging = Some(RouteNodeDragState {
                    target,
                    start_mouse: world,
                    original_pos: self.route_node_position(target).unwrap_or(world),
                    original_bounds: Self::route_node_bounds_target(target)
                        .and_then(|bounds_target| self.bounds_for_target(bounds_target)),
                });
                return true;
            }
            return response.dragged_by(crate::gpu2d::PointerButton::Primary);
        }

        let handle = response.hover_pos().and_then(|pointer| {
            self.detect_visible_bounds_handle(pointer, canvas_center, edge_threshold)
        });
        self.bounds_hovered_handle = handle;

        if let Some(hit) = handle
            && response.drag_started_by(crate::gpu2d::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(limits) = self.bounds_for_target(hit.target)
        {
            let world = self.camera.screen_to_world(pointer, canvas_center);
            self.bounds_dragging = Some(BoundsDragState {
                target: hit.target,
                handle: hit.handle,
                start_mouse: world,
                original_limits: limits,
            });
            return true;
        }

        handle.is_some() && response.dragged_by(crate::gpu2d::PointerButton::Primary)
    }

    fn bounds_for_target(&self, target: BoundsEditTarget) -> Option<[f32; 4]> {
        match target {
            BoundsEditTarget::InitialView => self.initial_view_bounds,
            BoundsEditTarget::CameraLimits => self.camera_limits,
            BoundsEditTarget::ConstructionView => self.construction_view_bounds,
        }
    }

    fn set_bounds_for_target(&mut self, target: BoundsEditTarget, bounds: [f32; 4]) {
        match target {
            BoundsEditTarget::InitialView => self.initial_view_bounds = Some(bounds),
            BoundsEditTarget::CameraLimits => self.camera_limits = Some(bounds),
            BoundsEditTarget::ConstructionView => self.construction_view_bounds = Some(bounds),
        }
    }

    fn route_node_bounds_target(target: RouteNodeTarget) -> Option<BoundsEditTarget> {
        match target {
            RouteNodeTarget::InitialView => Some(BoundsEditTarget::InitialView),
            RouteNodeTarget::CameraLimits => Some(BoundsEditTarget::CameraLimits),
            RouteNodeTarget::ConstructionView => Some(BoundsEditTarget::ConstructionView),
            RouteNodeTarget::CustomPreview(_) => None,
        }
    }

    fn route_node_position(&self, target: RouteNodeTarget) -> Option<Vec2> {
        match target {
            RouteNodeTarget::InitialView
            | RouteNodeTarget::CameraLimits
            | RouteNodeTarget::ConstructionView => Self::route_node_bounds_target(target)
                .and_then(|bounds_target| self.bounds_for_target(bounds_target))
                .map(Self::bounds_center),
            RouteNodeTarget::CustomPreview(index) => self
                .custom_preview_route
                .as_ref()
                .and_then(|points| points.get(index).copied()),
        }
    }

    fn set_custom_route_point(&mut self, index: usize, point: Vec2) {
        if let Some(points) = self.custom_preview_route.as_mut()
            && let Some(slot) = points.get_mut(index)
        {
            *slot = point;
        }
    }

    fn bounds_center(bounds: [f32; 4]) -> Vec2 {
        Vec2 {
            x: bounds[0] + (bounds[2] * 0.5),
            y: bounds[1] - (bounds[3] * 0.5),
        }
    }

    fn detect_route_node(
        &self,
        pointer: crate::gpu2d::Pos2,
        canvas_center: crate::gpu2d::Vec2,
        threshold: f32,
    ) -> Option<RouteNodeTarget> {
        let threshold_sq = threshold * threshold;
        let mut best = None;
        let mut best_dist_sq = f32::INFINITY;

        if let Some(points) = self.custom_preview_route.as_ref() {
            for (index, &point) in points.iter().enumerate() {
                let screen = self.camera.world_to_screen(point, canvas_center);
                let dx = pointer.x - screen.x;
                let dy = pointer.y - screen.y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= threshold_sq && dist_sq < best_dist_sq {
                    best = Some(RouteNodeTarget::CustomPreview(index));
                    best_dist_sq = dist_sq;
                }
            }
            return best;
        }

        for target in [
            RouteNodeTarget::InitialView,
            RouteNodeTarget::CameraLimits,
            RouteNodeTarget::ConstructionView,
        ] {
            let Some(point) = self.route_node_position(target) else {
                continue;
            };
            let screen = self.camera.world_to_screen(point, canvas_center);
            let dx = pointer.x - screen.x;
            let dy = pointer.y - screen.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= threshold_sq && dist_sq < best_dist_sq {
                best = Some(target);
                best_dist_sq = dist_sq;
            }
        }

        best
    }

    fn detect_visible_bounds_handle(
        &self,
        pointer: crate::gpu2d::Pos2,
        canvas_center: crate::gpu2d::Vec2,
        threshold: f32,
    ) -> Option<BoundsHandleHit> {
        for target in [
            BoundsEditTarget::InitialView,
            BoundsEditTarget::CameraLimits,
            BoundsEditTarget::ConstructionView,
        ] {
            let Some([tl_x, tl_y, w, h]) = self.bounds_for_target(target) else {
                continue;
            };
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
            if let Some(handle) = self.detect_bounds_handle(pointer, p_tl, p_br, threshold) {
                return Some(BoundsHandleHit { target, handle });
            }
        }
        None
    }

    /// Detect which bounds handle (if any) the pointer is hovering.
    fn detect_bounds_handle(
        &self,
        pointer: crate::gpu2d::Pos2,
        p_tl: crate::gpu2d::Pos2,
        p_br: crate::gpu2d::Pos2,
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
    fn compute_corner_drag(handle: BoundsHandle, bounds: [f32; 4], delta: (f32, f32)) -> [f32; 4] {
        let [tl_x, tl_y, w, h] = bounds;
        let (dx, dy) = delta;
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
