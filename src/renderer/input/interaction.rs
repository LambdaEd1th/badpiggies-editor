//! Top-level pointer interaction dispatch + select / box-select handlers.

use std::collections::BTreeSet;

use eframe::egui;

use crate::domain::types::{ObjectIndex, Vec2};

use super::super::{
    BoxSelectResult, CursorMode, DragMode, DragState, LevelRenderer, NodeDragResult, NodeDragState,
    NodeEditAction,
};
use super::point_to_segment_dist;

fn corner_uniform_scale(
    original_scale: Vec2,
    start_pointer_local: Vec2,
    current_local: Vec2,
) -> (f32, f32) {
    let start_abs_x = start_pointer_local.x.abs().max(1.0);
    let start_abs_y = start_pointer_local.y.abs().max(1.0);
    let ratio_x = current_local.x.abs().max(1.0) / start_abs_x;
    let ratio_y = current_local.y.abs().max(1.0) / start_abs_y;
    let uniform_ratio = ratio_x.max(ratio_y);
    let scale_abs_x = (original_scale.x.abs() * uniform_ratio).max(super::super::MIN_OBJECT_SCALE);
    let scale_abs_y = (original_scale.y.abs() * uniform_ratio).max(super::super::MIN_OBJECT_SCALE);
    (scale_abs_x, scale_abs_y)
}

fn axis_side(value: f32) -> f32 {
    if value < 0.0 { -1.0 } else { 1.0 }
}

fn dragged_scale_sign(original_scale_axis: f32, start_axis: f32, current_axis: f32) -> f32 {
    axis_side(original_scale_axis) * axis_side(start_axis) * axis_side(current_axis)
}

impl LevelRenderer {
    pub(in crate::renderer) fn handle_interaction(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        selected: &BTreeSet<ObjectIndex>,
        cursor_mode: CursorMode,
    ) {
        let is_shift = ui.input(|i| i.modifiers.shift);
        let is_alt = ui.input(|i| i.modifiers.alt);
        self.clicked_object = None;
        self.clicked_empty = false;
        self.drag_result = None;
        self.rotation_drag_result = None;
        self.scale_drag_result = None;
        self.node_drag_result = None;
        self.node_edit_action = None;
        self.box_select_result = None;
        self.draw_terrain_result = None;
        self.bounds_drag_result = None;
        self.suppress_context_menu_this_frame = false;
        self.hovered_rotation_handle = None;
        self.hovered_scale_handle = None;

        // Level bounds dragging takes priority (available in all modes when visible)
        if self.handle_bounds_drag(response, canvas_center) {
            self.handle_zoom(ui, response, canvas_center, rect);
            return;
        }

        match cursor_mode {
            CursorMode::Select => {
                self.handle_select_mode(ui, response, canvas_center, selected, is_shift, is_alt);
            }
            CursorMode::BoxSelect => {
                self.handle_box_select_mode(ui, response, canvas_center, rect, is_shift, is_alt);
            }
            CursorMode::DrawTerrain => {
                self.handle_draw_terrain_mode(ui, response, canvas_center, rect, is_shift, is_alt);
            }
            CursorMode::Pan => {
                self.handle_pan_mode(response, is_shift, is_alt);
            }
        }

        // Zoom is available in all modes
        self.handle_zoom(ui, response, canvas_center, rect);
    }

    /// Select mode: click-to-select, drag objects, terrain node editing. (Original behavior.)
    pub(super) fn handle_select_mode(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
        is_shift: bool,
        is_alt: bool,
    ) {
        self.hovered_scale_handle = response
            .hover_pos()
            .and_then(|pointer| self.scale_handle_hit(pointer, canvas_center, selected));
        self.hovered_rotation_handle = response
            .hover_pos()
            .and_then(|pointer| self.rotation_handle_hit(pointer, canvas_center, selected));

        // Start drag on primary press (without shift/alt)
        if response.drag_started_by(egui::PointerButton::Primary)
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let world = self.camera.screen_to_world(pointer, canvas_center);

            if let Some(target) = self.hovered_scale_handle {
                if let Some(sprite) = self
                    .sprite_data
                    .iter()
                    .find(|sprite| sprite.index == target.index)
                {
                    let center = self.camera.world_to_screen(
                        Vec2 {
                            x: sprite.world_pos.x,
                            y: sprite.world_pos.y,
                        },
                        canvas_center,
                    );
                    self.dragging = Some(DragState {
                        index: target.index,
                        start_mouse: world,
                        original_pos: sprite.world_pos,
                        mode: DragMode::Scale {
                            handle: target.kind,
                            start_pointer_local: Self::pointer_local(
                                center,
                                pointer,
                                sprite.rotation,
                            ),
                            original_scale: Vec2 {
                                x: sprite.scale.0,
                                y: sprite.scale.1,
                            },
                            original_half_size: sprite.half_size,
                            original_rotation: sprite.rotation,
                        },
                    });
                    self.clicked_object = Some(target.index);
                }
            } else if let Some(idx) = self.hovered_rotation_handle {
                if let Some(sprite) = self.sprite_data.iter().find(|sprite| sprite.index == idx) {
                    let center = self.camera.world_to_screen(
                        Vec2 {
                            x: sprite.world_pos.x,
                            y: sprite.world_pos.y,
                        },
                        canvas_center,
                    );
                    self.dragging = Some(DragState {
                        index: idx,
                        start_mouse: world,
                        original_pos: sprite.world_pos,
                        mode: DragMode::Rotate {
                            start_pointer_angle: Self::pointer_angle(center, pointer),
                            original_rotation: sprite.rotation,
                        },
                    });
                    self.clicked_object = Some(idx);
                }
            }
            // Check if we're starting a terrain node drag
            else if let Some((obj_idx, node_idx)) = self.hovered_terrain_node {
                // Find the world offset of this terrain
                let (tdx, tdy) = self.terrain_drag_offset(obj_idx);
                if let Some(td) = self.terrain_data.iter().find(|t| t.object_index == obj_idx) {
                    let (nx, ny) = td.curve_world_verts[node_idx];
                    self.node_dragging = Some(NodeDragState {
                        object_index: obj_idx,
                        node_index: node_idx,
                        start_mouse: world,
                        original_pos: Vec2 {
                            x: nx + tdx,
                            y: ny + tdy,
                        },
                    });
                    self.clicked_object = Some(obj_idx);
                }
            } else if let Some(idx) = self.hit_test(world, selected) {
                let orig = self
                    .sprite_data
                    .iter()
                    .find(|s| s.index == idx)
                    .map(|s| s.world_pos)
                    .unwrap_or_default();
                self.dragging = Some(DragState {
                    index: idx,
                    start_mouse: world,
                    original_pos: orig,
                    mode: DragMode::Move,
                });
                self.clicked_object = Some(idx);
            }
        }

        // Handle pan (middle mouse / shift+drag / alt+drag / primary on empty space)
        if response.dragged_by(egui::PointerButton::Middle)
            || (response.dragged_by(egui::PointerButton::Primary) && is_shift)
            || (response.dragged_by(egui::PointerButton::Primary) && is_alt)
            || (response.dragged_by(egui::PointerButton::Primary)
                && !is_shift
                && !is_alt
                && self.dragging.is_none()
                && self.node_dragging.is_none())
        {
            let delta = response.drag_delta();
            self.camera.center.x -= delta.x / self.camera.zoom;
            self.camera.center.y += delta.y / self.camera.zoom;
            self.panning = true;
        } else if self.dragging.is_none() && self.node_dragging.is_none() {
            self.panning = false;
        }

        // Update sprite position during object drag
        if let Some(ref drag) = self.dragging
            && let Some(pointer) = response.interact_pointer_pos()
        {
            match drag.mode {
                DragMode::Move => {
                    let current = self.camera.screen_to_world(pointer, canvas_center);
                    let dx = current.x - drag.start_mouse.x;
                    let dy = current.y - drag.start_mouse.y;
                    let tidx = drag.index;
                    let orig = drag.original_pos;
                    for sprite in &mut self.sprite_data {
                        if sprite.index == tidx {
                            sprite.world_pos.x = orig.x + dx;
                            sprite.world_pos.y = orig.y + dy;
                            break;
                        }
                    }
                }
                DragMode::Rotate {
                    start_pointer_angle,
                    original_rotation,
                } => {
                    let center = self.camera.world_to_screen(
                        Vec2 {
                            x: drag.original_pos.x,
                            y: drag.original_pos.y,
                        },
                        canvas_center,
                    );
                    let current_angle = Self::pointer_angle(center, pointer);
                    let delta = Self::normalize_angle_delta(current_angle - start_pointer_angle);
                    for sprite in &mut self.sprite_data {
                        if sprite.index == drag.index {
                            sprite.rotation = original_rotation + delta;
                            break;
                        }
                    }
                }
                DragMode::Scale {
                    handle,
                    start_pointer_local,
                    original_scale,
                    original_half_size,
                    original_rotation,
                } => {
                    let center = self.camera.world_to_screen(
                        Vec2 {
                            x: drag.original_pos.x,
                            y: drag.original_pos.y,
                        },
                        canvas_center,
                    );
                    let current_local = Self::pointer_local(center, pointer, original_rotation);
                    let start_abs_x = start_pointer_local.x.abs().max(1.0);
                    let start_abs_y = start_pointer_local.y.abs().max(1.0);
                    let mut scale_abs_x = original_scale.x.abs();
                    let mut scale_abs_y = original_scale.y.abs();
                    match handle {
                        super::super::ScaleHandleKind::Horizontal => {
                            scale_abs_x = (original_scale.x.abs()
                                * (current_local.x.abs().max(1.0) / start_abs_x))
                                .max(super::super::MIN_OBJECT_SCALE);
                        }
                        super::super::ScaleHandleKind::Vertical => {
                            scale_abs_y = (original_scale.y.abs()
                                * (current_local.y.abs().max(1.0) / start_abs_y))
                                .max(super::super::MIN_OBJECT_SCALE);
                        }
                        super::super::ScaleHandleKind::Corner => {
                            (scale_abs_x, scale_abs_y) = corner_uniform_scale(
                                original_scale,
                                start_pointer_local,
                                current_local,
                            );
                        }
                    }
                    let sign_x =
                        dragged_scale_sign(original_scale.x, start_pointer_local.x, current_local.x);
                    let sign_y =
                        dragged_scale_sign(original_scale.y, start_pointer_local.y, current_local.y);
                    let base_half_x = if original_scale.x.abs() > 0.0001 {
                        original_half_size.0 / original_scale.x.abs()
                    } else {
                        original_half_size.0
                    };
                    let base_half_y = if original_scale.y.abs() > 0.0001 {
                        original_half_size.1 / original_scale.y.abs()
                    } else {
                        original_half_size.1
                    };
                    for sprite in &mut self.sprite_data {
                        if sprite.index == drag.index {
                            sprite.scale.0 = sign_x * scale_abs_x;
                            sprite.scale.1 = sign_y * scale_abs_y;
                            sprite.half_size =
                                (base_half_x * scale_abs_x, base_half_y * scale_abs_y);
                            break;
                        }
                    }
                }
            }
        }

        // Update node position during terrain node drag
        if let Some(ref ndrag) = self.node_dragging
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let current = self.camera.screen_to_world(pointer, canvas_center);
            let new_x = ndrag.original_pos.x + (current.x - ndrag.start_mouse.x);
            let new_y = ndrag.original_pos.y + (current.y - ndrag.start_mouse.y);
            // Update the visual node position in curve_world_verts
            if let Some(td) = self
                .terrain_data
                .iter_mut()
                .find(|t| t.object_index == ndrag.object_index)
                && ndrag.node_index < td.curve_world_verts.len()
            {
                td.curve_world_verts[ndrag.node_index] = (new_x, new_y);
            }
        }

        // End object drag
        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(drag) = self.dragging.take()
        {
            for sprite in &self.sprite_data {
                if sprite.index == drag.index {
                    match drag.mode {
                        DragMode::Move => {
                            let dx = sprite.world_pos.x - drag.original_pos.x;
                            let dy = sprite.world_pos.y - drag.original_pos.y;
                            if dx.abs() > 0.001 || dy.abs() > 0.001 {
                                self.drag_result = Some((drag.index, Vec2 { x: dx, y: dy }));
                                // Keep camera offset active until batch is rebuilt
                                self.pending_drag_offset = Some((drag.index, dx, dy));
                            }
                        }
                        DragMode::Rotate {
                            original_rotation, ..
                        } => {
                            let delta =
                                Self::normalize_angle_delta(sprite.rotation - original_rotation);
                            let delta_degrees = delta.to_degrees();
                            if delta_degrees.abs() > 0.01 {
                                self.pending_transform_preview = Some(drag.index);
                                self.rotation_drag_result = Some((drag.index, delta_degrees));
                            }
                        }
                        DragMode::Scale { original_scale, .. } => {
                            let scale = Vec2 {
                                x: sprite.scale.0,
                                y: sprite.scale.1,
                            };
                            if (scale.x - original_scale.x).abs() > 0.001
                                || (scale.y - original_scale.y).abs() > 0.001
                            {
                                self.pending_transform_preview = Some(drag.index);
                                self.scale_drag_result = Some((drag.index, scale));
                            }
                        }
                    }
                    break;
                }
            }
        }

        // End terrain node drag
        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(ndrag) = self.node_dragging.take()
        {
            // Compute new node position in local terrain space
            if let Some(td) = self
                .terrain_data
                .iter()
                .find(|t| t.object_index == ndrag.object_index)
                && ndrag.node_index < td.curve_world_verts.len()
            {
                let (wx, wy) = td.curve_world_verts[ndrag.node_index];
                // Find the world offset of this terrain object
                let world_offset = self
                    .world_positions
                    .iter()
                    .find(|(idx, _)| *idx == ndrag.object_index)
                    .map(|(_, p)| *p)
                    .unwrap_or_default();
                self.node_drag_result = Some(NodeDragResult {
                    object_index: ndrag.object_index,
                    node_index: ndrag.node_index,
                    new_local_pos: Vec2 {
                        x: wx - world_offset.x,
                        y: wy - world_offset.y,
                    },
                });
            }
        }

        // Click-to-select (tap without drag)
        if response.clicked()
            && !self.panning
            && let Some(click_pos) = response.interact_pointer_pos()
        {
            if let Some(target) = self.scale_handle_hit(click_pos, canvas_center, selected) {
                self.clicked_object = Some(target.index);
            } else if let Some(idx) = self.rotation_handle_hit(click_pos, canvas_center, selected) {
                self.clicked_object = Some(idx);
            } else {
                let click_world = self.camera.screen_to_world(click_pos, canvas_center);
                self.clicked_object = self.hit_test(click_world, selected);
            }
            self.clicked_empty = self.clicked_object.is_none();
            self.clicked_with_cmd = ui.input(|i| i.modifiers.command);
        }

        // Delete terrain node: Delete/Backspace while hovering a node (min 3 nodes)
        if let Some((obj_idx, node_idx)) = self.hovered_terrain_node {
            let delete_pressed = ui
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
            if delete_pressed
                && let Some(td) = self.terrain_data.iter().find(|t| t.object_index == obj_idx)
                && td.curve_world_verts.len() > 2
            {
                self.node_edit_action = Some(NodeEditAction::Delete {
                    object_index: obj_idx,
                    node_index: node_idx,
                });
            }
        }

        // Insert terrain node: double-click on a terrain curve segment
        if response.double_clicked()
            && !self.panning
            && let Some(click_pos) = response.interact_pointer_pos()
        {
            let click_world = self.camera.screen_to_world(click_pos, canvas_center);
            // Find closest segment on selected terrain
            let mut best: Option<(ObjectIndex, usize, f32, f32)> = None;
            let threshold = 12.0 / self.camera.zoom; // 12 screen-px
            for td in self.terrain_data.iter() {
                if selected.contains(&td.object_index) && td.curve_world_verts.len() >= 2 {
                    let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
                    for seg in 0..td.curve_world_verts.len() - 1 {
                        let (ax, ay) = td.curve_world_verts[seg];
                        let (bx, by) = td.curve_world_verts[seg + 1];
                        let (ax, ay) = (ax + tdx, ay + tdy);
                        let (bx, by) = (bx + tdx, by + tdy);
                        let (dist, t) =
                            point_to_segment_dist(click_world.x, click_world.y, ax, ay, bx, by);
                        if dist < threshold
                            && best.is_none_or(|(_, _, best_dist, _)| dist < best_dist)
                        {
                            best = Some((td.object_index, seg, dist, t));
                        }
                    }
                }
            }
            if let Some((obj_idx, seg, _dist, t)) = best
                && let Some(td) = self
                    .terrain_data
                    .iter()
                    .find(|tt| tt.object_index == obj_idx)
            {
                let (ax, ay) = td.curve_world_verts[seg];
                let (bx, by) = td.curve_world_verts[seg + 1];
                let (tdx, tdy) = self.terrain_drag_offset(obj_idx);
                let wx = ax + (bx - ax) * t + tdx;
                let wy = ay + (by - ay) * t + tdy;
                let world_offset = self
                    .world_positions
                    .iter()
                    .find(|(idx, _)| *idx == obj_idx)
                    .map(|(_, p)| *p)
                    .unwrap_or_default();
                self.node_edit_action = Some(NodeEditAction::Insert {
                    object_index: obj_idx,
                    after_node: seg,
                    local_pos: Vec2 {
                        x: wx - world_offset.x,
                        y: wy - world_offset.y,
                    },
                });
            }
        }
    }

    /// Box-select mode: drag a rectangle to select all objects inside it.
    pub(super) fn handle_box_select_mode(
        &mut self,
        _ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        _rect: egui::Rect,
        is_shift: bool,
        is_alt: bool,
    ) {
        // Middle-mouse / shift / alt drag → pan (same as other modes)
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

        // Start box selection on primary press
        if response.drag_started_by(egui::PointerButton::Primary)
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            self.box_select_start = Some(pointer);
        }

        // End box selection on primary release
        if response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(start) = self.box_select_start.take()
            && let Some(end) = response.interact_pointer_pos()
        {
            let screen_min = egui::pos2(start.x.min(end.x), start.y.min(end.y));
            let screen_max = egui::pos2(start.x.max(end.x), start.y.max(end.y));
            let world_min = self.camera.screen_to_world(
                egui::pos2(screen_min.x, screen_max.y), // bottom-left in screen → min in world
                canvas_center,
            );
            let world_max = self.camera.screen_to_world(
                egui::pos2(screen_max.x, screen_min.y), // top-right in screen → max in world
                canvas_center,
            );
            let mut indices = BTreeSet::new();
            for &(idx, pos) in &self.world_positions {
                if pos.x >= world_min.x
                    && pos.x <= world_max.x
                    && pos.y >= world_min.y
                    && pos.y <= world_max.y
                {
                    indices.insert(idx);
                }
            }
            self.box_select_result = Some(BoxSelectResult { indices });
        }

        self.panning = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{axis_side, corner_uniform_scale, dragged_scale_sign};
    use crate::domain::types::Vec2;
    use crate::renderer::MIN_OBJECT_SCALE;

    #[test]
    fn corner_handle_uses_shared_ratio() {
        let original_scale = Vec2 { x: 2.0, y: 3.0 };
        let start_pointer_local = Vec2 { x: 40.0, y: 20.0 };
        let current_local = Vec2 { x: 50.0, y: 40.0 };

        let (scale_x, scale_y) =
            corner_uniform_scale(original_scale, start_pointer_local, current_local);

        assert_eq!(scale_x, 4.0);
        assert_eq!(scale_y, 6.0);
    }

    #[test]
    fn corner_handle_respects_min_scale() {
        let original_scale = Vec2 { x: 0.2, y: 0.3 };
        let start_pointer_local = Vec2 { x: 40.0, y: 20.0 };
        let current_local = Vec2 { x: 0.0, y: 0.0 };

        let (scale_x, scale_y) =
            corner_uniform_scale(original_scale, start_pointer_local, current_local);

        assert_eq!(scale_x, MIN_OBJECT_SCALE);
        assert_eq!(scale_y, MIN_OBJECT_SCALE);
    }

    #[test]
    fn axis_side_keeps_zero_on_positive_side() {
        assert_eq!(axis_side(0.0), 1.0);
        assert_eq!(axis_side(3.0), 1.0);
        assert_eq!(axis_side(-3.0), -1.0);
    }

    #[test]
    fn dragged_scale_sign_flips_when_pointer_crosses_center() {
        assert_eq!(dragged_scale_sign(2.0, 40.0, 10.0), 1.0);
        assert_eq!(dragged_scale_sign(2.0, 40.0, -10.0), -1.0);
    }

    #[test]
    fn dragged_scale_sign_preserves_existing_flip_until_crossing_back() {
        assert_eq!(dragged_scale_sign(-2.0, 40.0, 10.0), -1.0);
        assert_eq!(dragged_scale_sign(-2.0, 40.0, -10.0), 1.0);
    }
}
