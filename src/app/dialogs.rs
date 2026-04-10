//! Dialog windows — delete confirmation, add object, shortcuts, about, tools.

use eframe::egui;

use crate::locale::I18n;
use crate::renderer::CursorMode;
use crate::types::*;

use super::EditorApp;

impl EditorApp {
    /// Delete confirmation dialog.
    pub(super) fn render_delete_confirm(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if let Some((ref del_indices, ref del_name)) =
            self.tabs[self.active_tab].pending_delete.clone()
        {
            let mut action = 0u8; // 0=pending, 1=confirm, 2=cancel
            egui::Window::new(t.get("win_confirm_delete"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(t.fmt1("status_delete_confirm", del_name));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(t.get("btn_ok")).clicked() {
                            action = 1;
                        }
                        if ui.button(t.get("btn_cancel")).clicked() {
                            action = 2;
                        }
                    });
                });
            match action {
                1 => {
                    self.push_undo();
                    let tab = &mut self.tabs[self.active_tab];
                    if let Some(ref mut level) = tab.level {
                        let mut sorted: Vec<ObjectIndex> = del_indices.clone();
                        sorted.sort_unstable_by(|a, b| b.cmp(a));
                        for idx in sorted {
                            level.delete_object(idx);
                        }
                        tab.selected.clear();
                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                        tab.status = format!("已删除: {}", del_name);
                    }
                    tab.pending_delete = None;
                }
                2 => {
                    self.tabs[self.active_tab].pending_delete = None;
                }
                _ => {}
            }
        }
    }

    /// Tool mode selector window.
    pub(super) fn render_tool_window(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_tools {
            return;
        }
        egui::Window::new(t.get("tool_window_title"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_tools)
            .default_pos([8.0, 80.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let modes = [
                        (CursorMode::Select, "tool_select", "V"),
                        (CursorMode::BoxSelect, "tool_box_select", "M"),
                        (CursorMode::DrawTerrain, "tool_draw_terrain", "P"),
                        (CursorMode::Pan, "tool_pan", "H"),
                    ];
                    for (mode, key, shortcut) in &modes {
                        let label = format!("{} ({})", t.get(key), shortcut);
                        if ui
                            .selectable_label(self.cursor_mode == *mode, label)
                            .clicked()
                        {
                            self.cursor_mode = *mode;
                        }
                    }
                });
            });
    }

    /// Shortcuts help window.
    pub(super) fn render_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let t = self.t();

        // Add tool mode shortcuts to the shortcuts window
        egui::Window::new(t.get("win_shortcuts"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_shortcuts)
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(t.get("shortcuts_key"));
                        ui.strong(t.get("shortcuts_action"));
                        ui.end_row();

                        // ── Mouse ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_mouse"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_scroll"));
                        ui.label(t.get("shortcuts_zoom"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_drag"));
                        ui.label(t.get("shortcuts_pan"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_click"));
                        ui.label(t.get("shortcuts_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_cmd_click"));
                        ui.label(t.get("shortcuts_cmd_click_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_shift_click"));
                        ui.label(t.get("shortcuts_shift_click_action"));
                        ui.end_row();

                        // ── Keyboard Shortcuts ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_keyboard"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_b_key"));
                        ui.label(t.get("shortcuts_toggle_bg"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_undo"));
                        ui.label(t.get("shortcuts_undo_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_redo"));
                        ui.label(t.get("shortcuts_redo_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_copy"));
                        ui.label(t.get("shortcuts_copy_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_cut"));
                        ui.label(t.get("shortcuts_cut_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_paste"));
                        ui.label(t.get("shortcuts_paste_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_duplicate"));
                        ui.label(t.get("shortcuts_duplicate_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_delete"));
                        ui.label(t.get("shortcuts_delete_action"));
                        ui.end_row();

                        // ── Tool Modes ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_tools"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_select"));
                        ui.label(t.get("tool_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_box_select"));
                        ui.label(t.get("tool_box_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_draw_terrain"));
                        ui.label(t.get("tool_draw_terrain"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_pan"));
                        ui.label(t.get("tool_pan"));
                        ui.end_row();

                        // ── Terrain Editing ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_terrain"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_select"));
                        ui.label(t.get("shortcuts_terrain_select_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_drag"));
                        ui.label(t.get("shortcuts_terrain_drag_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_dblclick"));
                        ui.label(t.get("shortcuts_terrain_dblclick_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_delete"));
                        ui.label(t.get("shortcuts_terrain_delete_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_rclick"));
                        ui.label(t.get("shortcuts_terrain_rclick_action"));
                        ui.end_row();
                    });
            });
    }

    /// About window.
    pub(super) fn render_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let t = self.t();
        egui::Window::new(t.get("win_about"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_about)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Bad Piggies Editor");
                    ui.label(format!(
                        "{}{}",
                        t.get("about_version_prefix"),
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.separator();
                    ui.hyperlink_to(
                        env!("CARGO_PKG_AUTHORS"),
                        "https://space.bilibili.com/8217621",
                    );
                    ui.label(t.get("about_built_with"));
                    ui.label(t.get("about_license"));
                });
            });
    }

    /// Add Object dialog.
    pub(super) fn render_add_obj_dialog(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_add_dialog {
            return;
        }
        let mut open = true;
        egui::Window::new(t.get("win_add_object"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t.get("add_type"));
                    ui.radio_value(&mut self.add_obj_is_parent, false, "Prefab");
                    ui.radio_value(&mut self.add_obj_is_parent, true, "Parent");
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("add_name"));
                    ui.text_edit_singleline(&mut self.add_obj_name);
                });
                if !self.add_obj_is_parent {
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_prefab_index"));
                        ui.add(egui::DragValue::new(&mut self.add_obj_prefab_index));
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t.get("btn_ok")).clicked() {
                        self.push_undo();
                        let add_name = if self.add_obj_name.is_empty() {
                            "NewObject".to_string()
                        } else {
                            self.add_obj_name.clone()
                        };
                        let is_parent = self.add_obj_is_parent;
                        let prefab_index = self.add_obj_prefab_index;
                        let tab = &mut self.tabs[self.active_tab];
                        if let Some(ref mut level) = tab.level {
                            let new_idx = level.objects.len();
                            if is_parent {
                                level.objects.push(LevelObject::Parent(ParentObject {
                                    name: add_name.clone(),
                                    position: Vec3 {
                                        x: 0.0,
                                        y: 0.0,
                                        z: 0.0,
                                    },
                                    children: Vec::new(),
                                    parent: None,
                                }));
                            } else {
                                level.objects.push(LevelObject::Prefab(PrefabInstance {
                                    name: add_name.clone(),
                                    position: Vec3 {
                                        x: 0.0,
                                        y: 0.0,
                                        z: 0.0,
                                    },
                                    prefab_index,
                                    rotation: Vec3 {
                                        x: 0.0,
                                        y: 0.0,
                                        z: 0.0,
                                    },
                                    scale: Vec3 {
                                        x: 1.0,
                                        y: 1.0,
                                        z: 1.0,
                                    },
                                    data_type: DataType::None,
                                    terrain_data: None,
                                    override_data: None,
                                    parent: None,
                                }));
                            }
                            level.roots.push(new_idx);
                            tab.selected = std::collections::BTreeSet::from([new_idx]);
                            let cam = tab.renderer.camera.clone();
                            tab.renderer.set_level(level);
                            tab.renderer.camera = cam;
                            tab.status = t.fmt1("status_added", &add_name);
                        }
                        self.show_add_dialog = false;
                    }
                    if ui.button(t.get("btn_cancel")).clicked() {
                        self.show_add_dialog = false;
                    }
                });
            });
        if !open {
            self.show_add_dialog = false;
        }
    }
}

/// Update (or create) `m_cameraLimits` in the LevelManager override data.
pub(super) fn update_camera_limits_in_level(level: &mut LevelData, vals: [f32; 4]) {
    // Find LevelManager with override data
    for obj in level.objects.iter_mut() {
        if let LevelObject::Prefab(p) = obj
            && p.name == "LevelManager"
            && let Some(ref mut od) = p.override_data
        {
            if let Some(pos) = od.raw_text.find("m_cameraLimits") {
                // Replace existing float values in-place
                let mut result = od.raw_text[..pos].to_string();
                let after = &od.raw_text[pos..];
                let mut remaining = after;
                let mut val_idx = 0;
                while val_idx < 4 {
                    let fx = remaining.find("Float x = ");
                    let fy = remaining.find("Float y = ");
                    let fp = match (fx, fy) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => None,
                    };
                    if let Some(fp) = fp {
                        let eq = &remaining[fp..];
                        if let Some(eq_pos) = eq.find("= ") {
                            let before_num = &remaining[..fp + eq_pos + 2];
                            result.push_str(before_num);
                            let num_start = &eq[eq_pos + 2..];
                            let end = num_start
                                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                                .unwrap_or(num_start.len());
                            // Write new value
                            result.push_str(&format!("{}", vals[val_idx]));
                            remaining = &num_start[end..];
                            val_idx += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.push_str(remaining);
                od.raw_bytes = result.as_bytes().to_vec();
                od.raw_text = result;
            }
            return;
        }
    }
}
