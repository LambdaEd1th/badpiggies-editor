use std::collections::BTreeSet;

use eframe::egui;

use crate::domain::types::*;

use super::sprites;
use super::{DragMode, LevelRenderer, ScaleHandleKind, ScaleHandleTarget};

pub(super) const ROTATION_HANDLE_STEM_PX: f32 = 10.0;
pub(super) const ROTATION_HANDLE_OFFSET_PX: f32 = 26.0;
pub(super) const ROTATION_HANDLE_RADIUS_PX: f32 = 7.0;
pub(super) const SCALE_HANDLE_OFFSET_PX: f32 = 10.0;
pub(super) const SCALE_HANDLE_RADIUS_PX: f32 = 7.0;
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
        canvas_center: egui::Vec2,
    ) -> (egui::Pos2, egui::Pos2) {
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
        let rotate = |dy: f32| egui::pos2(center.x + dy * sin_r, center.y + dy * cos_r);
        (
            rotate(-(half_height + ROTATION_HANDLE_STEM_PX)),
            rotate(-(half_height + ROTATION_HANDLE_OFFSET_PX)),
        )
    }

    pub(super) fn rotation_handle_hit(
        &self,
        pointer: egui::Pos2,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let sprite = self.selected_transform_sprite(selected)?;
        let (_, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let dx = pointer.x - handle_center.x;
        let dy = pointer.y - handle_center.y;
        ((dx * dx + dy * dy).sqrt() <= ROTATION_HANDLE_RADIUS_PX + 4.0).then_some(sprite.index)
    }

    fn scale_handle_positions(
        &self,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) -> [(ScaleHandleKind, egui::Pos2, egui::Pos2); 3] {
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
            egui::pos2(
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
        pointer: egui::Pos2,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ScaleHandleTarget> {
        let sprite = self.selected_transform_sprite(selected)?;
        self.scale_handle_positions(sprite, canvas_center)
            .into_iter()
            .find_map(|(kind, _, handle_center)| {
                let dx = pointer.x - handle_center.x;
                let dy = pointer.y - handle_center.y;
                ((dx * dx + dy * dy).sqrt() <= SCALE_HANDLE_RADIUS_PX + 4.0).then_some(
                    ScaleHandleTarget {
                        index: sprite.index,
                        kind,
                    },
                )
            })
    }

    pub(super) fn scale_handle_cursor(handle: ScaleHandleKind) -> egui::CursorIcon {
        match handle {
            ScaleHandleKind::Horizontal => egui::CursorIcon::ResizeHorizontal,
            ScaleHandleKind::Vertical => egui::CursorIcon::ResizeVertical,
            ScaleHandleKind::Corner => egui::CursorIcon::ResizeNwSe,
        }
    }

    pub(super) fn pointer_local(center: egui::Pos2, pointer: egui::Pos2, rotation: f32) -> Vec2 {
        let rel_x = pointer.x - center.x;
        let rel_y = pointer.y - center.y;
        let cos_r = rotation.cos();
        let sin_r = rotation.sin();
        Vec2 {
            x: rel_x * cos_r - rel_y * sin_r,
            y: rel_x * sin_r + rel_y * cos_r,
        }
    }

    pub(super) fn pointer_angle(center: egui::Pos2, pointer: egui::Pos2) -> f32 {
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
        painter: &egui::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) {
        let (stem_start, handle_center) = self.rotation_handle_positions(sprite, canvas_center);
        let is_active = self.dragging.as_ref().is_some_and(|drag| {
            drag.index == sprite.index && matches!(drag.mode, DragMode::Rotate { .. })
        });
        let fill = if is_active || self.hovered_rotation_handle == Some(sprite.index) {
            egui::Color32::from_rgb(255, 235, 120)
        } else {
            egui::Color32::WHITE
        };
        painter.line_segment(
            [stem_start, handle_center],
            egui::Stroke::new(2.0, egui::Color32::YELLOW),
        );
        painter.circle_filled(handle_center, ROTATION_HANDLE_RADIUS_PX, fill);
        painter.circle_stroke(
            handle_center,
            ROTATION_HANDLE_RADIUS_PX,
            egui::Stroke::new(2.0, egui::Color32::BLACK),
        );
    }

    pub(super) fn draw_scale_handle(
        &self,
        painter: &egui::Painter,
        sprite: &sprites::SpriteDrawData,
        canvas_center: egui::Vec2,
    ) {
        let active_handle = self.dragging.as_ref().and_then(|drag| match drag.mode {
            DragMode::Scale { handle, .. } if drag.index == sprite.index => Some(handle),
            _ => None,
        });
        let hovered_handle = self
            .hovered_scale_handle
            .and_then(|target| (target.index == sprite.index).then_some(target.kind));
        for (kind, anchor, handle_center) in self.scale_handle_positions(sprite, canvas_center) {
            let fill = if active_handle == Some(kind) || hovered_handle == Some(kind) {
                egui::Color32::from_rgb(140, 230, 255)
            } else {
                egui::Color32::WHITE
            };
            painter.line_segment(
                [anchor, handle_center],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 220, 255)),
            );
            let size = match kind {
                ScaleHandleKind::Horizontal => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 2.8, SCALE_HANDLE_RADIUS_PX * 1.5)
                }
                ScaleHandleKind::Vertical => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 1.5, SCALE_HANDLE_RADIUS_PX * 2.8)
                }
                ScaleHandleKind::Corner => {
                    egui::vec2(SCALE_HANDLE_RADIUS_PX * 2.0, SCALE_HANDLE_RADIUS_PX * 2.0)
                }
            };
            let handle_rect = egui::Rect::from_center_size(handle_center, size);
            painter.rect_filled(handle_rect, 2.0, fill);
            painter.rect_stroke(
                handle_rect,
                2.0,
                egui::Stroke::new(2.0, egui::Color32::BLACK),
                egui::StrokeKind::Outside,
            );
        }
    }
}
