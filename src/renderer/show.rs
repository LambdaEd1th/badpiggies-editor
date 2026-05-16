use std::collections::BTreeSet;

use eframe::egui;

use crate::domain::types::*;

use super::{
    BoundsHandle, CanvasContextAction, CursorMode, DragMode, GLOW_ATLAS,
    LevelRenderer, NodeEditAction, PreviewPlaybackState, grid, particles,
};

/// Known atlas filenames and their paths relative to the sprites directory.
pub(crate) const ATLAS_FILES: &[&str] = &[
    "IngameAtlas.png",
    "IngameAtlas2.png",
    "IngameAtlas3.png",
    "Ingame_Characters_Sheet_01.png",
    "Ingame_Sheet_04.png",
    "Props_Generic_Sheet_01.png",
];

impl LevelRenderer {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        selected: &BTreeSet<ObjectIndex>,
        cursor_mode: CursorMode,
        tr: &'static crate::i18n::locale::I18n,
        has_clipboard: bool,
    ) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
        let rect = response.rect;
        let canvas_center = rect.center().to_vec2();
        self.context_action = None;
        self.context_selected_object = None;

        // Advance animation time (use stable_dt = measured frame interval, not predicted)
        self.time += ui.input(|i| i.stable_dt as f64);

        // Track mouse world position
        self.mouse_world = response
            .hover_pos()
            .map(|p| self.camera.screen_to_world(p, canvas_center));

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
        if self.show_grid_overlay
            && let Some(ref cg) = self.construction_grid
        {
            grid::draw_construction_grid(
                &painter,
                cg,
                &self.camera,
                canvas_center,
                rect,
                &mut self.tex_cache,
                ui.ctx(),
            );
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

        // Draw Zzz particles BEFORE sprites — in Unity emitter is at z=+0.5 (behind bird body)
        particles::draw_zzz_particles(
            &self.zzz_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache.get(GLOW_ATLAS),
        );

        // Sprites with goal bobbing + compound sub-sprites (renderOrder=12)
        self.draw_sprites(&painter, canvas_center, rect, selected);

        // Draw fan particles (cloud puffs, renderOrder=12)
        particles::draw_fan_particles(
            &self.fan_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache.get(GLOW_ATLAS),
        );

        particles::draw_attached_effect_particles(
            &self.attached_effect_particles,
            &self.camera,
            &painter,
            canvas_center,
            rect,
            self.tex_cache
                .load_texture(ui.ctx(), "particles/Particles_Sheet_01.png", "Particles_Sheet_01"),
        );

        // Terrain overlays stay after the world transparent layer so inline collider
        // terrain rendering does not paint over selection or debug visuals.
        if self.show_terrain_tris {
            self.draw_terrain_wireframe(&painter, canvas_center);
        }
        self.draw_terrain_selection(&painter, canvas_center, selected);

        // ── Dark level overlay with LitArea cutouts ──
        if self.dark_level && self.show_dark_overlay {
            self.draw_dark_overlay(&painter, canvas_center, rect);
        }

        // ── Front-ground + foreground (eff_z < 0): waves/foam/dummy + foreground, after sprites ──
        if self.show_bg {
            self.draw_bg_z_range(&painter, canvas_center, rect, (f32::NEG_INFINITY, 0.0));
        }

        // Grid (drawn on top of all scene content)
        if self.show_grid {
            self.draw_grid(&painter, rect, canvas_center);
        }

        // HUD overlays: origin axes, physics ground, level bounds, zoom info
        self.draw_hud(&painter, rect, canvas_center, tr);

        // Tool mode overlays (box-select rect, terrain draw preview)
        self.draw_tool_overlay(&painter, canvas_center, cursor_mode);

        // Set cursor icon for bounds handles
        if let Some(handle) = self.bounds_hovered_handle {
            let icon = match handle {
                BoundsHandle::Move => egui::CursorIcon::Grab,
                BoundsHandle::Left | BoundsHandle::Right => egui::CursorIcon::ResizeHorizontal,
                BoundsHandle::Top | BoundsHandle::Bottom => egui::CursorIcon::ResizeVertical,
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
            };
            ui.ctx().set_cursor_icon(icon);
        }
        if let Some(bounds_dragging) = self.bounds_dragging.as_ref() {
            let icon = match bounds_dragging.handle {
                BoundsHandle::Move => egui::CursorIcon::Grabbing,
                BoundsHandle::Left | BoundsHandle::Right => egui::CursorIcon::ResizeHorizontal,
                BoundsHandle::Top | BoundsHandle::Bottom => egui::CursorIcon::ResizeVertical,
                BoundsHandle::TopLeft | BoundsHandle::BottomRight => egui::CursorIcon::ResizeNwSe,
                BoundsHandle::TopRight | BoundsHandle::BottomLeft => egui::CursorIcon::ResizeNeSw,
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
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if let Some(target) = self.hovered_scale_handle {
            ui.ctx()
                .set_cursor_icon(Self::scale_handle_cursor(target.kind));
        } else if self.hovered_rotation_handle.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }

        // Request continuous repaint for animations
        ui.ctx().request_repaint();

        // Lazy-load atlas textures (only attempt once per atlas)
        self.lazy_load_textures(ui.ctx());

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
        let context_menu_node_can_delete = terrain_node_can_delete(self.context_menu_node);
        if !suppress_context_menu && response.secondary_clicked() {
            let hovered_node_can_delete = terrain_node_can_delete(hovered_node);
            let context_world = response
                .interact_pointer_pos()
                .map(|pointer| self.camera.screen_to_world(pointer, canvas_center));
            let context_object = context_world
                .and_then(|world| self.hit_test(world, selected))
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
            if let Some(index) = context_object
                && !selected.contains(&index)
            {
                self.context_selected_object = Some(index);
            }
        }
        let context_world = self.context_menu_world_pos;
        let context_indices = self.context_menu_indices.clone();
        let has_context_selection = !context_indices.is_empty();
        let is_mac = cfg!(target_os = "macos");
        let copy_shortcut = if is_mac { "Cmd+C" } else { "Ctrl+C" };
        let cut_shortcut = if is_mac { "Cmd+X" } else { "Ctrl+X" };
        let paste_shortcut = if is_mac { "Cmd+V" } else { "Ctrl+V" };
        let dup_shortcut = if is_mac { "Cmd+D" } else { "Ctrl+D" };
        if !suppress_context_menu {
            response.context_menu(|ui| {
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_copy")).shortcut_text(copy_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Copy(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_cut")).shortcut_text(cut_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Cut(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_clipboard,
                        egui::Button::new(tr.get("menu_paste")).shortcut_text(paste_shortcut),
                    )
                    .clicked()
                {
                    self.context_action = Some(CanvasContextAction::Paste {
                        context_indices: context_indices.clone(),
                        world_pos: context_world,
                    });
                    ui.close();
                }
                ui.separator();
                if ui.button(tr.get("menu_add_object")).clicked() {
                    self.context_action = Some(CanvasContextAction::AddObject {
                        world_pos: context_world,
                    });
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_duplicate")).shortcut_text(dup_shortcut),
                    )
                    .clicked()
                {
                    self.context_action =
                        Some(CanvasContextAction::Duplicate(context_indices.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has_context_selection,
                        egui::Button::new(tr.get("menu_delete")).shortcut_text("Del"),
                    )
                    .clicked()
                {
                    self.context_action =
                        Some(CanvasContextAction::Delete(context_indices.clone()));
                    ui.close();
                }

                if let Some((object_index, node_index, can_delete)) = context_menu_node_can_delete {
                    ui.separator();
                    if ui.button(tr.get("context_toggle_node_texture")).clicked() {
                        self.node_edit_action = Some(NodeEditAction::ToggleTexture {
                            object_index,
                            node_index,
                        });
                        ui.close();
                    }
                    if ui
                        .add_enabled(can_delete, egui::Button::new(tr.get("menu_delete")))
                        .clicked()
                    {
                        self.node_edit_action = Some(NodeEditAction::Delete {
                            object_index,
                            node_index,
                        });
                        ui.close();
                    }
                }

                ui.separator();
                if ui.button(tr.get("menu_fit_view")).clicked() {
                    self.fit_to_level();
                    ui.close();
                }
            });
        }
    }
}
