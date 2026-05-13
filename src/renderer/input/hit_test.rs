//! Hit testing for objects and terrain.

use std::collections::BTreeSet;

use crate::domain::types::{ObjectIndex, Vec2};

use super::super::LevelRenderer;
use super::point_in_triangle;

impl LevelRenderer {
    pub(in crate::renderer) fn hit_test(
        &self,
        pos: Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
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
                    && sprite.parent.is_some_and(|p| selected.contains(&p)));
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
                if best.is_none_or(|(_, best_dist)| dist < best_dist) {
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
    pub(in crate::renderer) fn terrain_drag_offset(&self, object_index: ObjectIndex) -> (f32, f32) {
        if let Some(ref drag) = self.dragging {
            if drag.index == object_index
                && matches!(drag.mode, super::super::DragMode::Move)
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
}
