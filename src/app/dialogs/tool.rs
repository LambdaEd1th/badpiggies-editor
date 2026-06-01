//! Tool mode selector window.

use eframe::egui;

use crate::i18n::locale::I18n;
use crate::renderer::{CursorMode, PreviewPlaybackState, TerrainPresetShape};

use super::super::EditorApp;
use super::{preview_playback_icon, terrain_preset_icon, terrain_preset_label_key, tool_mode_icon};

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

struct PreviewPanelState {
    title: String,
    preview_state: PreviewPlaybackState,
    show_dark_preview_controls: bool,
    night_vision_enabled: bool,
}

fn preview_panel_state(app: &EditorApp, t: &'static I18n) -> Option<PreviewPanelState> {
    let tab = app.tabs.get(app.active_tab)?;
    let has_level = tab.level.is_some();
    if !has_level || tab.is_save_tab() {
        return None;
    }

    let show_dark_preview_controls = tab.renderer.is_dark_level();
    let preview_tool_target = preview_tool_target_name(app);

    let title = if let Some(name) = preview_tool_target.as_deref() {
        format!("{}: {}", t.get("tool_preview_title"), name)
    } else if show_dark_preview_controls {
        t.get("tool_preview_dark_overlay_title")
    } else {
        t.get("tool_preview_title")
    };

    Some(PreviewPanelState {
        title,
        preview_state: tab.renderer.preview_playback_state(),
        show_dark_preview_controls,
        night_vision_enabled: if show_dark_preview_controls {
            tab.renderer.night_vision_enabled()
        } else {
            false
        },
    })
}

impl EditorApp {
    pub(in crate::app) fn should_show_preview_controls_panel(&self) -> bool {
        self.show_preview_controls_panel && preview_panel_state(self, self.t()).is_some()
    }

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
        let mut terrain_draw_has_collider = if show_terrain_presets {
            self.tabs[self.active_tab]
                .renderer
                .terrain_draw_has_collider()
        } else {
            true
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
        let window_height = if show_terrain_presets {
            button_size.y + 126.0
        } else {
            button_size.y + 16.0
        };
        let mut queued_preset = None;
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
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut terrain_draw_has_collider, t.get("prop_collider"));
                    });
                }
            });

        if show_terrain_presets {
            self.tabs[self.active_tab]
                .renderer
                .set_terrain_draw_has_collider(terrain_draw_has_collider);
        }

        if show_terrain_presets && terrain_round_segments != initial_round_segments {
            self.tabs[self.active_tab]
                .renderer
                .set_terrain_round_segments(terrain_round_segments);
        }

        if let Some(shape) = queued_preset {
            self.toggle_active_terrain_preset(shape);
        }
    }

    pub(in crate::app) fn render_preview_controls_panel(
        &mut self,
        ui: &mut egui::Ui,
        t: &'static I18n,
    ) {
        if !self.show_preview_controls_panel {
            return;
        }

        let Some(state) = preview_panel_state(self, t) else {
            return;
        };

        let button_size = egui::vec2(40.0, 40.0);
        let mut queued_preview_state = None;
        let mut queued_night_vision_enabled = None;

        egui::Panel::bottom("preview_controls_panel")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.vertical(|ui| {
                        ui.heading(&state.title);
                        ui.separator();
                        ui.horizontal(|ui| {
                            for (next_state, key) in [
                                (PreviewPlaybackState::Build, "tool_preview_build"),
                                (PreviewPlaybackState::Play, "tool_preview_play"),
                                (PreviewPlaybackState::Pause, "tool_preview_pause"),
                            ] {
                                let response = ui.add(
                                    egui::Button::image(preview_playback_icon(next_state))
                                        .selected(state.preview_state == next_state)
                                        .frame(true)
                                        .min_size(button_size)
                                        .image_tint_follows_text_color(true),
                                );
                                let response = response.on_hover_text(t.get(key));
                                if response.clicked() {
                                    queued_preview_state = Some(next_state);
                                }
                            }
                        });

                        if state.show_dark_preview_controls {
                            let mut night_vision_enabled = state.night_vision_enabled;
                            if ui
                                .checkbox(
                                    &mut night_vision_enabled,
                                    t.get("tool_preview_night_vision"),
                                )
                                .clicked()
                            {
                                queued_night_vision_enabled = Some(night_vision_enabled);
                            }
                        }
                    });
                });
                ui.add_space(4.0);
            });

        if let Some(next_state) = queued_preview_state {
            self.request_preview_playback_state(next_state, t);
        }

        if let Some(enabled) = queued_night_vision_enabled {
            self.tabs[self.active_tab]
                .renderer
                .set_night_vision_enabled(enabled);
        }
    }
}
