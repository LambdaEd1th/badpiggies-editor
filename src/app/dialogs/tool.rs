//! Tool mode selector window.

use eframe::egui;

use crate::i18n::locale::I18n;
use crate::renderer::{CursorMode, PreviewPlaybackState, TerrainPresetShape};

use super::super::EditorApp;
use super::{terrain_preset_icon, terrain_preset_label_key, tool_mode_icon};

fn preview_tool_target_name(app: &EditorApp) -> Option<String> {
    let tab = app.tabs.get(app.active_tab)?;
    let level = tab.level.as_ref()?;
    if tab.is_save_tab() || tab.selected.len() != 1 {
        return None;
    }
    let index = *tab.selected.iter().next()?;
    let object = level.objects.get(index)?;
    let name = match object {
        crate::domain::types::LevelObject::Prefab(prefab) => prefab.name.as_str(),
        crate::domain::types::LevelObject::Parent(parent) => parent.name.as_str(),
    };
    (name == "Fan" || name.starts_with("WindArea")).then(|| name.to_string())
}

impl EditorApp {
    /// Tool mode selector window.
    pub(in crate::app) fn render_tool_window(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_tools {
            return;
        }
        let button_size = egui::vec2(40.0, 40.0);
        let button_spacing = 6.0;
        let button_count = 4.0;
        let base_window_width =
            button_size.x * button_count + button_spacing * (button_count - 1.0);
        let show_terrain_presets = self.cursor_mode == CursorMode::DrawTerrain
            && !self.tabs[self.active_tab].is_save_tab();
        let active_terrain_preset = if show_terrain_presets {
            self.tabs[self.active_tab].renderer.active_terrain_preset()
        } else {
            None
        };
        let mut terrain_round_segments = if show_terrain_presets {
            self.tabs[self.active_tab].renderer.terrain_round_segments()
        } else {
            24
        };
        let initial_round_segments = terrain_round_segments;
        let window_width = if show_terrain_presets {
            base_window_width.max(button_size.x * 5.0 + button_spacing * 4.0)
        } else {
            base_window_width
        };
        let preview_tool_target = preview_tool_target_name(self);
        let show_preview_controls = preview_tool_target.is_some();
        let preview_state = if show_preview_controls {
            self.tabs[self.active_tab].renderer.preview_playback_state()
        } else {
            PreviewPlaybackState::Build
        };
        let window_height = if show_terrain_presets {
            button_size.y + 126.0 + if show_preview_controls { 42.0 } else { 0.0 }
        } else {
            button_size.y + 16.0 + if show_preview_controls { 42.0 } else { 0.0 }
        };
        let mut queued_preset = None;
        let mut queued_preview_state = None;
        egui::Window::new(t.get("tool_window_title"))
            .collapsible(true)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_tools)
            .default_pos([8.0, 80.0])
            .fixed_size(egui::vec2(window_width + 16.0, window_height))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(button_spacing, 0.0);
                ui.set_width(window_width);
                ui.horizontal(|ui| {
                    let modes = [
                        (CursorMode::Select, "tool_select", "V"),
                        (CursorMode::BoxSelect, "tool_box_select", "M"),
                        (CursorMode::DrawTerrain, "tool_draw_terrain", "P"),
                        (CursorMode::Pan, "tool_pan", "H"),
                    ];
                    for (mode, key, shortcut) in &modes {
                        let tooltip = format!("{} ({})", t.get(key), shortcut);
                        let response = ui.add(
                            egui::Button::image(tool_mode_icon(*mode))
                                .selected(self.cursor_mode == *mode)
                                .frame(true)
                                .min_size(button_size)
                                .image_tint_follows_text_color(true),
                        );
                        let response = response.on_hover_text(tooltip);
                        if response.clicked() {
                            self.cursor_mode = *mode;
                        }
                    }
                });

                if show_terrain_presets {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(t.get("tool_terrain_presets"));
                    ui.horizontal(|ui| {
                        for shape in [
                            TerrainPresetShape::Circle,
                            TerrainPresetShape::Rectangle,
                            TerrainPresetShape::PerfectCircle,
                            TerrainPresetShape::Square,
                            TerrainPresetShape::EquilateralTriangle,
                        ] {
                            let response = ui.add(
                                egui::Button::image(terrain_preset_icon(shape))
                                    .selected(active_terrain_preset == Some(shape))
                                    .frame(true)
                                    .min_size(button_size)
                                    .image_tint_follows_text_color(true),
                            );
                            let response =
                                response.on_hover_text(t.get(terrain_preset_label_key(shape)));
                            if response.clicked() {
                                queued_preset = Some(shape);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(t.get("tool_terrain_round_segments"));
                        ui.add(
                            egui::DragValue::new(&mut terrain_round_segments)
                                .range(3..=128)
                                .speed(1),
                        );
                    });
                }

                if show_preview_controls {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    let preview_title = if let Some(name) = preview_tool_target.as_deref() {
                        format!("{}: {}", t.get("tool_preview_title"), name)
                    } else {
                        t.get("tool_preview_title")
                    };
                    ui.label(preview_title);
                    ui.horizontal(|ui| {
                        for (state, key) in [
                            (PreviewPlaybackState::Build, "tool_preview_build"),
                            (PreviewPlaybackState::Play, "tool_preview_play"),
                            (PreviewPlaybackState::Pause, "tool_preview_pause"),
                        ] {
                            if ui
                                .add(
                                    egui::Button::new(t.get(key))
                                        .selected(preview_state == state)
                                        .min_size(egui::vec2(68.0, 28.0)),
                                )
                                .clicked()
                            {
                                queued_preview_state = Some(state);
                            }
                        }
                    });
                }
            });

        if show_terrain_presets && terrain_round_segments != initial_round_segments {
            self.tabs[self.active_tab]
                .renderer
                .set_terrain_round_segments(terrain_round_segments);
        }

        if let Some(shape) = queued_preset {
            self.toggle_active_terrain_preset(shape);
        }

        if let Some(state) = queued_preview_state {
            self.tabs[self.active_tab]
                .renderer
                .set_preview_playback_state(state);
        }
    }
}
