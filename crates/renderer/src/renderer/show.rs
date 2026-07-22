use std::collections::BTreeSet;

use crate::domain::types::*;

use super::{BoundsHandle, CursorMode, DragMode, LevelRenderer, PreviewPlaybackState};

/// Known atlas filenames and their embedded asset keys.
pub(crate) const ATLAS_FILES: &[&str] = &[
    "IngameAtlas.png",
    "IngameAtlas2.png",
    "IngameAtlas3.png",
    "Ingame_Characters_Sheet_01.png",
    "Ingame_Sheet_04.png",
    "MenuAtlas.png",
    "MenuAtlas2.png",
    "Props_Generic_Sheet_01.png",
];

impl LevelRenderer {
    pub fn show(
        &mut self,
        ui: &mut crate::gpu2d::Ui,
        selected: &BTreeSet<ObjectIndex>,
        cursor_mode: CursorMode,
        tr: &'static crate::i18n::locale::I18n,
        _has_clipboard: bool,
    ) {
        let available = ui.available_size();
        let (response, painter) =
            ui.allocate_painter(available, crate::gpu2d::Sense::click_and_drag());
        let rect = response.rect;
        let canvas_center = rect.center().to_vec2();
        self.context_action = None;
        self.context_menu_request = None;
        self.context_selected_object = None;
        self.context_menu_just_opened = false;

        // Advance animation time (use stable_dt = measured frame interval, not predicted)
        self.time += ui.input(|i| i.stable_dt as f64);

        // Track mouse world position
        self.mouse_world = response
            .hover_pos()
            .map(|p| self.camera.screen_to_world(p, canvas_center));

        // Ensure textures needed by CPU-drawn sprites, particles, and other
        // non-shader paths are resident for the current frame.
        self.lazy_load_textures(ui.ctx());

        // Background (sky + ground fill + parallax layers + clouds)
        if self.show_bg {
            let dt = ui.input(|i| i.stable_dt);
            self.draw_background_all(&painter, canvas_center, rect, dt);
        }

        // ── Mid-ground transparent layer: decorative terrain + ground bg (0 <= Z < 5) ──
        // These now share one back-to-front Z flow so decorative terrain no longer depends
        // on a fixed pass to stay behind beach/grass.
        self.draw_ground_bg_and_decorative_terrain(&painter, canvas_center, rect);

        // Construction grid overlay (renderOrder=9, between ground and collider terrain)
        if self.show_grid_overlay && self.construction_grid.is_some() {
            self.draw_construction_grid_overlay(&painter, canvas_center, rect);
        }

        // ── Interaction: drag, pan, click ──
        self.handle_interaction(ui, &response, canvas_center, rect, selected, cursor_mode);

        // ── World transparent layer: collider terrain + sprites + wind ──

        // Glow starbursts + goal flags stay before collider terrain/world objects.
        self.draw_pre_terrain_effects(&painter, canvas_center, rect);

        // ── Particle simulation (fan, wind, zzz) ──
        let dt = ui.input(|i| i.stable_dt);
        if self.preview_playback_state == PreviewPlaybackState::Play {
            self.update_particles(dt);
        }

        // Sprites with goal bobbing + compound sub-sprites (renderOrder=12)
        self.draw_sprites(&painter, canvas_center, rect, selected);

        // Terrain overlays stay after the world transparent layer so inline collider
        // terrain rendering does not paint over selection or debug visuals.
        if self.show_terrain_tris {
            self.draw_terrain_wireframe(&painter, rect);
        }
        self.draw_terrain_selection(&painter, canvas_center, selected);

        // ── Front-ground + foreground (eff_z < 0), normal queues first ──
        if self.show_bg {
            self.draw_bg_z_range(
                &painter,
                canvas_center,
                rect,
                (f32::NEG_INFINITY, 0.0),
                Some(false),
            );
        }

        // ── Dark level overlay with LitArea cutouts ──
        if self.dark_level && self.show_dark_overlay {
            self.draw_dark_overlay(&painter, canvas_center, rect);
        }

        // ── Front-ground render-last materials (queue > 3005), after dark overlay ──
        if self.show_bg {
            self.draw_bg_z_range(
                &painter,
                canvas_center,
                rect,
                (f32::NEG_INFINITY, 0.0),
                Some(true),
            );
        }

        if self.dark_level && self.show_dark_overlay && self.night_vision_enabled() {
            self.draw_night_vision_overlay(&painter, rect);
        }

        // Grid (drawn on top of all scene content)
        if self.show_grid {
            self.draw_grid(&painter, rect, canvas_center);
        }

        // HUD overlays: origin axes, physics ground, level bounds, zoom info
        self.draw_hud(&painter, rect, canvas_center, tr);

        // Tool mode overlays (box-select rect, terrain draw preview)
        self.draw_tool_overlay(&painter, canvas_center, cursor_mode);

        // Set cursor icon for overlay editing targets.
        if self.route_node_dragging.is_some() {
            ui.ctx().set_cursor_icon(crate::gpu2d::CursorIcon::Grabbing);
        } else if self.route_node_hovered.is_some() {
            ui.ctx().set_cursor_icon(crate::gpu2d::CursorIcon::Grab);
        } else if let Some(handle) = self.bounds_hovered_handle {
            let icon = match handle.handle {
                BoundsHandle::Move => crate::gpu2d::CursorIcon::Grab,
                BoundsHandle::Left | BoundsHandle::Right => {
                    crate::gpu2d::CursorIcon::ResizeHorizontal
                }
                BoundsHandle::Top | BoundsHandle::Bottom => {
                    crate::gpu2d::CursorIcon::ResizeVertical
                }
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => {
                    crate::gpu2d::CursorIcon::ResizeNwSe
                }
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => {
                    crate::gpu2d::CursorIcon::ResizeNeSw
                }
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if let Some(bounds_dragging) = self.bounds_dragging.as_ref() {
            let icon = match bounds_dragging.handle {
                BoundsHandle::Move => crate::gpu2d::CursorIcon::Grabbing,
                BoundsHandle::Left | BoundsHandle::Right => {
                    crate::gpu2d::CursorIcon::ResizeHorizontal
                }
                BoundsHandle::Top | BoundsHandle::Bottom => {
                    crate::gpu2d::CursorIcon::ResizeVertical
                }
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => {
                    crate::gpu2d::CursorIcon::ResizeNwSe
                }
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => {
                    crate::gpu2d::CursorIcon::ResizeNeSw
                }
            };
            ui.ctx().set_cursor_icon(icon);
        } else if self
            .dragging
            .as_ref()
            .and_then(|drag| match drag.mode {
                DragMode::Scale { handle, .. } => Some(handle),
                _ => None,
            })
            .map(|handle| {
                ui.ctx().set_cursor_icon(Self::scale_handle_cursor(handle));
            })
            .is_some()
        {
        } else if self
            .dragging
            .as_ref()
            .is_some_and(|drag| matches!(drag.mode, DragMode::Rotate { .. }))
        {
            ui.ctx().set_cursor_icon(crate::gpu2d::CursorIcon::Grabbing);
        } else if let Some(target) = self.hovered_scale_handle {
            ui.ctx()
                .set_cursor_icon(Self::scale_handle_cursor(target.kind));
        } else if self.hovered_rotation_handle.is_some() {
            ui.ctx().set_cursor_icon(crate::gpu2d::CursorIcon::Grab);
        }

        if self.has_ambient_animation
            || (self.preview_playback_state == PreviewPlaybackState::Play
                && self.has_preview_animation)
        {
            ui.ctx().request_repaint();
        }

        let hovered_node = self.hovered_terrain_node;
        let suppress_context_menu = self.suppress_context_menu_this_frame
            || (cursor_mode == CursorMode::DrawTerrain && !self.draw_terrain_points.is_empty());

        let terrain_node_can_delete = |node: Option<(ObjectIndex, usize)>| {
            node.and_then(|(object_index, node_index)| {
                self.terrain_data
                    .iter()
                    .find(|terrain| terrain.object_index == object_index)
                    .map(|terrain| {
                        (
                            object_index,
                            node_index,
                            terrain.curve_world_verts.len() > 2,
                        )
                    })
            })
        };
        if !suppress_context_menu && response.secondary_clicked() {
            let hovered_node_can_delete = terrain_node_can_delete(hovered_node);
            let pointer = response.interact_pointer_pos();
            let context_world =
                pointer.map(|position| self.camera.screen_to_world(position, canvas_center));
            let context_object = context_world
                .and_then(|world| {
                    self.hit_test_with_screen_slop(
                        world,
                        selected,
                        if response.pointer_source() == crate::gpu2d::PointerSource::Touch {
                            super::TOUCH_OBJECT_HIT_SLOP_PX
                        } else {
                            0.0
                        },
                    )
                })
                .or_else(|| hovered_node.map(|(object_index, _)| object_index));
            self.context_menu_world_pos = context_world;
            self.context_menu_indices = context_object
                .map(|index| {
                    if selected.contains(&index) {
                        selected.iter().copied().collect()
                    } else {
                        vec![index]
                    }
                })
                .unwrap_or_else(|| selected.iter().copied().collect());
            self.context_menu_node = hovered_node_can_delete
                .map(|(object_index, node_index, _)| (object_index, node_index));
            self.context_menu_just_opened = true;
            if let Some(index) = context_object
                && !selected.contains(&index)
            {
                self.context_selected_object = Some(index);
            }
            if let Some(screen_pos) = pointer {
                self.context_menu_request = Some(super::CanvasContextMenuRequest {
                    screen_pos,
                    world_pos: context_world,
                    indices: self.context_menu_indices.clone(),
                    node: hovered_node_can_delete,
                    can_flip: self
                        .context_menu_indices
                        .iter()
                        .any(|&index| self.sprite_data.iter().any(|sprite| sprite.index == index)),
                });
            }
        }
    }

    pub fn apply_context_menu_action(&mut self, action: &str) -> bool {
        let indices = self.context_menu_indices.clone();
        match action {
            "copy" => self.context_action = Some(super::CanvasContextAction::Copy(indices)),
            "cut" => self.context_action = Some(super::CanvasContextAction::Cut(indices)),
            "paste" => {
                self.context_action = Some(super::CanvasContextAction::Paste {
                    context_indices: indices,
                    world_pos: self.context_menu_world_pos,
                });
            }
            "add" => {
                self.context_action = Some(super::CanvasContextAction::AddObject {
                    world_pos: self.context_menu_world_pos,
                });
            }
            "duplicate" => {
                self.context_action = Some(super::CanvasContextAction::Duplicate(indices));
            }
            "delete" => self.context_action = Some(super::CanvasContextAction::Delete(indices)),
            "flip_horizontal" => {
                self.context_action = Some(super::CanvasContextAction::FlipHorizontal(indices));
            }
            "flip_vertical" => {
                self.context_action = Some(super::CanvasContextAction::FlipVertical(indices));
            }
            "toggle_node_texture" => {
                let Some((object_index, node_index)) = self.context_menu_node else {
                    return false;
                };
                self.node_edit_action = Some(super::NodeEditAction::ToggleTexture {
                    object_index,
                    node_index,
                });
            }
            "delete_node" => {
                let Some((object_index, node_index)) = self.context_menu_node else {
                    return false;
                };
                self.node_edit_action = Some(super::NodeEditAction::Delete {
                    object_index,
                    node_index,
                });
            }
            "fit" => self.fit_to_level(),
            _ => return false,
        }
        true
    }
}
