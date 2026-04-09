//! Dialog windows — delete confirmation, add object, shortcuts, about.

use eframe::egui;

use crate::locale::I18n;
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

    /// Shortcuts help window.
    pub(super) fn render_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let t = self.t();
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
                        ui.label(t.get("shortcuts_scroll"));
                        ui.label(t.get("shortcuts_zoom"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_drag"));
                        ui.label(t.get("shortcuts_pan"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_click"));
                        ui.label(t.get("shortcuts_select"));
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
