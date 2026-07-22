use std::collections::BTreeSet;

use crate::domain::types::*;

use super::sprites;
use super::{DragMode, LevelRenderer, ScaleHandleKind, ScaleHandleTarget};

pub(super) const ROTATION_HANDLE_STEM_PX: f32 = 10.0;
pub(super) const ROTATION_HANDLE_OFFSET_PX: f32 = 26.0;
pub(super) const ROTATION_HANDLE_RADIUS_PX: f32 = 7.0;
pub(super) const SCALE_HANDLE_OFFSET_PX: f32 = 10.0;
pub(super) const SCALE_HANDLE_RADIUS_PX: f32 = 7.0;
const TOUCH_ROTATION_HANDLE_RADIUS_PX: f32 = 10.0;
const TOUCH_SCALE_HANDLE_RADIUS_PX: f32 = 10.0;
const TOUCH_HANDLE_HIT_RADIUS_PX: f32 = 22.0;
pub(crate) const MIN_OBJECT_SCALE: f32 = 0.05;

impl LevelRenderer {
    pub(super) fn selected_transform_sprite<'a>(
        &'a self,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<&'a sprites::SpriteDrawData> {
        if selected.len() != 1 {
            return None;
        }
        let index = selected.iter().next().copied()?;
        self.sprite_data
            .iter()
            .find(|sprite| sprite.index == index && !sprite.is_terrain)
    }

    fn rotation_handle_positions(
        &self,
        sprite: &sprites::SpriteDrawData,
        canvas_center: crate::gpu2d::Vec2,
    ) -> (crate::gpu2d::Pos2, crate::gpu2d::Pos2) {
        let center = self.camera.world_to_screen(
            Vec2 {
                x: sprite.world_pos.x,
                y: sprite.world_pos.y,
            },
            canvas_center,
        );
        let half_height = sprite.half_size.1 * self.camera.zoom;
        let cos_r = sprite.rotation.cos();
        let sin_r = sprite.rotation.sin();
        let rotate = |dy: f32| crate::gpu2d::pos2(center.x + dy * sin_r, center.y + dy * cos_r);
        (
            rotate(-(half_height + ROTATION_HANDLE_STEM_PX)),
            rotate(-(half_height + ROTATION_HANDLE_OFFSET_PX)),
        )
    }

    pub(super) fn rotation_handle_hit(
        &self,
        pointer: crate::gpu2d::Pos2,
        canvas_center: crate::gpu2d::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let sprite = self.selected_transform_sprite(selected)?;
        let (_, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let dx = pointer.x - handle_center.x;
        let dy = pointer.y - handle_center.y;
        let hit_radius = if self.touch_input_active {
            TOUCH_HANDLE_HIT_RADIUS_PX
        } else {
            ROTATION_HANDLE_RADIUS_PX + 4.0
        };
        ((dx * dx + dy * dy).sqrt() <= hit_radius).then_some(sprite.index)
    }

    fn scale_handle_positions(
        &self,
        sprite: &sprites::SpriteDrawData,
        canvas_center: crate::gpu2d::Vec2,
    ) -> [(ScaleHandleKind, crate::gpu2d::Pos2, crate::gpu2d::Pos2); 3] {
        let center = self.camera.world_to_screen(
            Vec2 {
                x: sprite.world_pos.x,
                y: sprite.world_pos.y,
            },
            canvas_center,
        );
        let half_width = sprite.half_size.0 * self.camera.zoom;
        let half_height = sprite.half_size.1 * self.camera.zoom;
        let cos_r = sprite.rotation.cos();
        let sin_r = sprite.rotation.sin();
        let rotate = |dx: f32, dy: f32| {
            crate::gpu2d::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };
        [
            (
                ScaleHandleKind::Horizontal,
                rotate(half_width, 0.0),
                rotate(half_width + SCALE_HANDLE_OFFSET_PX, 0.0),
            ),
            (
                ScaleHandleKind::Vertical,
                rotate(0.0, half_height),
                rotate(0.0, half_height + SCALE_HANDLE_OFFSET_PX),
            ),
            (
                ScaleHandleKind::Corner,
                rotate(half_width, half_height),
                rotate(
                    half_width + SCALE_HANDLE_OFFSET_PX,
                    half_height + SCALE_HANDLE_OFFSET_PX,
                ),
            ),
        ]
    }

    pub(super) fn scale_handle_hit(
        &self,
        pointer: crate::gpu2d::Pos2,
        canvas_center: crate::gpu2d::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ScaleHandleTarget> {
        let sprite = self.selected_transform_sprite(selected)?;
        let hit_radius = if self.touch_input_active {
            TOUCH_HANDLE_HIT_RADIUS_PX
        } else {
            SCALE_HANDLE_RADIUS_PX + 4.0
        };
        self.scale_handle_positions(sprite, canvas_center)
            .into_iter()
            .filter_map(|(kind, _, handle_center)| {
                let dx = pointer.x - handle_center.x;
                let dy = pointer.y - handle_center.y;
                let distance = (dx * dx + dy * dy).sqrt();
                (distance <= hit_radius).then_some((
                    distance,
                    ScaleHandleTarget {
                        index: sprite.index,
                        kind,
                    },
                ))
            })
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, target)| target)
    }

    pub(super) fn scale_handle_cursor(handle: ScaleHandleKind) -> crate::gpu2d::CursorIcon {
        match handle {
            ScaleHandleKind::Horizontal => crate::gpu2d::CursorIcon::ResizeHorizontal,
            ScaleHandleKind::Vertical => crate::gpu2d::CursorIcon::ResizeVertical,
            ScaleHandleKind::Corner => crate::gpu2d::CursorIcon::ResizeNwSe,
        }
    }

    pub(super) fn pointer_local(
        center: crate::gpu2d::Pos2,
        pointer: crate::gpu2d::Pos2,
        rotation: f32,
    ) -> Vec2 {
        let rel_x = pointer.x - center.x;
        let rel_y = pointer.y - center.y;
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();
        Vec2 {
            x: rel_x * cos_r - rel_y * sin_r,
            y: rel_x * sin_r + rel_y * cos_r,
        }
    }

    pub(super) fn pointer_angle(center: crate::gpu2d::Pos2, pointer: crate::gpu2d::Pos2) -> f32 {
        (center.y - pointer.y).atan2(pointer.x - center.x)
    }

    pub(super) fn normalize_angle_delta(mut angle: f32) -> f32 {
        while angle <= -std::f32::consts::PI {
            angle += std::f32::consts::TAU;
        }
        while angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        angle
    }

    pub(super) fn draw_rotation_handle(
        &self,
        painter: &crate::gpu2d::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: crate::gpu2d::Vec2,
    ) {
        let (stem_start, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let is_active = self.dragging.as_ref().is_some_and(|drag| {
            drag.index == sprite.index && matches!(drag.mode, DragMode::Rotate { .. })
        });
        let fill = if is_active || self.hovered_rotation_handle == Some(sprite.index) {
            crate::gpu2d::Color32::from_rgb(255, 235, 120)
        } else {
            crate::gpu2d::Color32::WHITE
        };
        let radius = if self.touch_input_active {
            TOUCH_ROTATION_HANDLE_RADIUS_PX
        } else {
            ROTATION_HANDLE_RADIUS_PX
        };
        painter.line_segment(
            [stem_start, handle_center],
            crate::gpu2d::Stroke::new(2.0, crate::gpu2d::Color32::YELLOW),
        );
        painter.circle_filled(handle_center, radius, fill);
        painter.circle_stroke(
            handle_center,
            radius,
            crate::gpu2d::Stroke::new(2.0, crate::gpu2d::Color32::BLACK),
        );
    }

    pub(super) fn draw_scale_handle(
        &self,
        painter: &crate::gpu2d::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: crate::gpu2d::Vec2,
    ) {
        let active_handle = self.dragging.as_ref().and_then(|drag| match drag.mode {
            DragMode::Scale { handle, .. } if drag.index == sprite.index => Some(handle),
            _ => None,
        });
        let hovered_handle = self
            .hovered_scale_handle
            .and_then(|target| (target.index == sprite.index).then_some(target.kind));
        let radius = if self.touch_input_active {
            TOUCH_SCALE_HANDLE_RADIUS_PX
        } else {
            SCALE_HANDLE_RADIUS_PX
        };
        for (kind, anchor, handle_center) in self
            .scale_handle_positions(sprite, canvas_center)
            .into_iter()
        {
            let fill = if active_handle == Some(kind) || hovered_handle == Some(kind) {
                crate::gpu2d::Color32::from_rgb(140, 230, 255)
            } else {
                crate::gpu2d::Color32::WHITE
            };
            painter.line_segment(
                [anchor, handle_center],
                crate::gpu2d::Stroke::new(2.0, crate::gpu2d::Color32::from_rgb(120, 220, 255)),
            );
            let size = match kind {
                ScaleHandleKind::Horizontal => crate::gpu2d::vec2(radius * 2.8, radius * 1.5),
                ScaleHandleKind::Vertical => crate::gpu2d::vec2(radius * 1.5, radius * 2.8),
                ScaleHandleKind::Corner => crate::gpu2d::vec2(radius * 2.0, radius * 2.0),
            };
            let handle_rect = crate::gpu2d::Rect::from_center_size(handle_center, size);
            painter.rect_filled(handle_rect, 2.0, fill);
            painter.rect_stroke(
                handle_rect,
                2.0,
                crate::gpu2d::Stroke::new(2.0, crate::gpu2d::Color32::BLACK),
                crate::gpu2d::StrokeKind::Outside,
            );
        }
    }
}
