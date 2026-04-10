//! Hit testing, terrain point tests, and geometry helpers for the renderer.

use std::collections::BTreeSet;

use crate::assets;
use crate::types::{ObjectIndex, Vec2};

use super::terrain;
use super::{Camera, CursorMode, DragState, LevelRenderer, NodeDragResult, NodeDragState, NodeEditAction, BoundsHandle, BoundsDragState};

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
pub(super) fn point_to_segment_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> (f32, f32) {
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

impl LevelRenderer {
    /// Draw a single terrain's edge using CPU fallback (splat textures or flat vertex-color).
    pub(super) fn draw_terrain_edge_cpu(
        painter: &egui::Painter,
        td: &terrain::TerrainDrawData,
        camera: &Camera,
        canvas_center: egui::Vec2,
        tex_cache: &assets::TextureCache,
        scratch: &mut egui::Mesh,
    ) {
        let mut drew_textured = false;
        if let Some(ref sm) = td.edge_splat0_mesh
            && let Some(ref name) = td.edge_splat0
            && let Some(tex_id) = tex_cache.get(name)
        {
            terrain::transform_mesh_to_screen_into(sm, camera, canvas_center, scratch);
            scratch.texture_id = tex_id;
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
            drew_textured = true;
        }
        if let Some(ref sm) = td.edge_splat1_mesh
            && let Some(ref name) = td.edge_splat1
            && let Some(tex_id) = tex_cache.get(name)
        {
            terrain::transform_mesh_to_screen_into(sm, camera, canvas_center, scratch);
            scratch.texture_id = tex_id;
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
            drew_textured = true;
        }
        if !drew_textured && let Some(ref edge) = td.edge_mesh {
            terrain::transform_mesh_to_screen_into(edge, camera, canvas_center, scratch);
            painter.add(egui::Shape::mesh(std::mem::take(scratch)));
        }
    }

    pub(super) fn hit_test(&self, pos: Vec2, selected: &BTreeSet<ObjectIndex>) -> Option<ObjectIndex> {
        let mut best: Option<(ObjectIndex, f32)> = None;
        for sprite in self.sprite_data.iter().rev() {
            // Allow terrain through only if it's the currently selected object
            if sprite.is_terrain && !selected.contains(&sprite.index) {
                continue;
            }
            // Skip objects that are hidden (not rendered) unless selected or parent is selected
            let is_selected = selected.contains(&sprite.index)
                || (sprite.is_hidden
                    && sprite.parent.is_some()
                    && sprite.parent.map_or(false, |p| selected.contains(&p)));
            if !is_selected && sprite.is_hidden {
                continue;
            }
            let dx = (pos.x - sprite.world_pos.x).abs();
            let dy = (pos.y - sprite.world_pos.y).abs();
            if dx <= sprite.half_size.0 && dy <= sprite.half_size.1 {
                // For terrain, refine hit test using fill mesh triangles
                if sprite.is_terrain && !self.point_in_terrain(sprite.index, pos) {
                    continue;
                }
                let dist = dx * dx + dy * dy;
                if best.is_none() || dist < best.unwrap().1 {
                    best = Some((sprite.index, dist));
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    /// Check whether a world-space point lies inside a terrain's fill mesh triangles.
    fn point_in_terrain(&self, index: ObjectIndex, pos: Vec2) -> bool {
        let td = self.terrain_data.iter().find(|t| t.object_index == index);
        let td = match td {
            Some(t) => t,
            None => return true, // no terrain data → fall back to AABB
        };
        let mesh = match td.fill_mesh {
            Some(ref m) => m,
            None => return true, // no fill mesh → fall back to AABB
        };
        let verts = &mesh.vertices;
        let indices = &mesh.indices;
        // Apply drag offset so hit test matches the drawn position
        let (ox, oy) = self.terrain_drag_offset(index);
        for tri in indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if i0 >= verts.len() || i1 >= verts.len() || i2 >= verts.len() {
                continue;
            }
            let a = egui::pos2(verts[i0].pos.x + ox, verts[i0].pos.y + oy);
            let b = egui::pos2(verts[i1].pos.x + ox, verts[i1].pos.y + oy);
            let c = egui::pos2(verts[i2].pos.x + ox, verts[i2].pos.y + oy);
            if point_in_triangle(egui::pos2(pos.x, pos.y), a, b, c) {
                return true;
            }
        }
        false
    }

    /// Returns (dx, dy) drag offset for a given object index, or (0, 0) if not being dragged.
    pub(super) fn terrain_drag_offset(&self, object_index: ObjectIndex) -> (f32, f32) {
        if let Some(ref drag) = self.dragging {
            if drag.index == object_index
                && let Some(sprite) = self.sprite_data.iter().find(|s| s.index == object_index)
            {
                return (
                    sprite.world_pos.x - drag.original_pos.x,
                    sprite.world_pos.y - drag.original_pos.y,
                );
            }
        } else if let Some((idx, dx, dy)) = self.pending_drag_offset
            && idx == object_index
        {
            return (dx, dy);
        }
        (0.0, 0.0)
    }

    /// Handle all user interaction: drag, pan, click, zoom, terrain node editing.
    pub(super) fn handle_interaction(
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
        self.drag_result = None;
        self.node_drag_result = None;
        self.node_edit_action = None;
        self.box_select_result = None;
        self.draw_terrain_result = None;
        self.bounds_drag_result = None;

        // Level bounds dragging takes priority (available in all modes when visible)
        if self.handle_bounds_drag(response, canvas_center) {
            self.handle_zoom(ui, response, canvas_center, rect);
            return;
        }

        match cursor_mode {
            CursorMode::Select => {
                self.handle_select_mode(ui, response, canvas_center, rect, selected, is_shift, is_alt);
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
    fn handle_select_mode(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        selected: &BTreeSet<ObjectIndex>,
        is_shift: bool,
        is_alt: bool,
    ) {
        // Start drag on primary press (without shift/alt)
        if response.drag_started_by(egui::PointerButton::Primary)
            && !is_shift
            && !is_alt
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let world = self.camera.screen_to_world(pointer, canvas_center);

            // Check if we're starting a terrain node drag
            if let Some((obj_idx, node_idx)) = self.hovered_terrain_node {
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
                    let dx = sprite.world_pos.x - drag.original_pos.x;
                    let dy = sprite.world_pos.y - drag.original_pos.y;
                    if dx.abs() > 0.001 || dy.abs() > 0.001 {
                        self.drag_result = Some((drag.index, Vec2 { x: dx, y: dy }));
                        // Keep camera offset active until batch is rebuilt
                        self.pending_drag_offset = Some((drag.index, dx, dy));
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
            let click_world = self.camera.screen_to_world(click_pos, canvas_center);
            self.clicked_object = self.hit_test(click_world, selected);
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

        // Toggle terrain node texture: right-click on a hovered node
        if response.secondary_clicked()
            && let Some((obj_idx, node_idx)) = self.hovered_terrain_node
        {
            self.node_edit_action = Some(NodeEditAction::ToggleTexture {
                object_index: obj_idx,
                node_index: node_idx,
            });
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
                        if dist < threshold && (best.is_none() || dist < best.unwrap().2) {
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

        // Handle zoom (scroll wheel, center-preserving)
        self.handle_zoom(ui, response, canvas_center, rect);
    }

    /// Box-select mode: drag a rectangle to select all objects inside it.
    fn handle_box_select_mode(
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
            self.box_select_result = Some(super::BoxSelectResult { indices });
        }

        self.panning = false;
    }

    /// Draw-terrain mode: click to place individual points.
    /// Close the curve by clicking near the first point (when ≥3 points exist),
    /// or press Enter to finish as open curve, Escape to cancel.
    fn handle_draw_terrain_mode(
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
                    self.draw_terrain_result = Some(super::DrawTerrainResult {
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
        if self.draw_terrain_active
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            self.draw_terrain_active = false;
            if self.draw_terrain_points.len() >= 2 {
                self.draw_terrain_result = Some(super::DrawTerrainResult {
                    points: std::mem::take(&mut self.draw_terrain_points),
                    closed: false,
                });
            } else {
                self.draw_terrain_points.clear();
            }
        }

        // Escape — cancel drawing
        if self.draw_terrain_active
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.draw_terrain_active = false;
            self.draw_terrain_points.clear();
        }

        self.panning = false;
    }

    /// Pan-only mode: all primary drags pan the view.
    fn handle_pan_mode(
        &mut self,
        response: &egui::Response,
        _is_shift: bool,
        _is_alt: bool,
    ) {
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
    fn handle_zoom(
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
    fn handle_bounds_drag(
        &mut self,
        response: &egui::Response,
        canvas_center: egui::Vec2,
    ) -> bool {
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
        let p_tl = self.camera.world_to_screen(
            Vec2 { x: tl_x, y: tl_y },
            canvas_center,
        );
        let p_br = self.camera.world_to_screen(
            Vec2 { x: tl_x + w, y: tl_y - h },
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
                        BoundsHandle::Right => {
                            [otl_x, otl_y, (ow + dx).max(1.0), oh]
                        }
                        BoundsHandle::Top => {
                            let nh = (oh + dy).max(1.0);
                            [otl_x, otl_y + dy - (nh - oh - dy), nh.max(1.0).min(oh + dy).max(1.0), oh]
                        }
                        BoundsHandle::Bottom => {
                            [otl_x, otl_y, ow, (oh - dy).max(1.0)]
                        }
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
                self.bounds_drag_result = Some(super::BoundsDragResult { new_limits });
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
        if near_left && near_top { return Some(BoundsHandle::TopLeft); }
        if near_right && near_top { return Some(BoundsHandle::TopRight); }
        if near_left && near_bottom { return Some(BoundsHandle::BottomLeft); }
        if near_right && near_bottom { return Some(BoundsHandle::BottomRight); }

        // Edges
        if near_left && in_y { return Some(BoundsHandle::Left); }
        if near_right && in_y { return Some(BoundsHandle::Right); }
        if near_top && in_x { return Some(BoundsHandle::Top); }
        if near_bottom && in_x { return Some(BoundsHandle::Bottom); }

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
        tl_x: f32, tl_y: f32, w: f32, h: f32,
        dx: f32, dy: f32,
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
