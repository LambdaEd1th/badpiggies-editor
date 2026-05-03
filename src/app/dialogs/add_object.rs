//! Add Object dialog.

use eframe::egui;

use crate::domain::types::*;
use crate::i18n::locale::I18n;

use super::super::EditorApp;
use super::{
    add_data_type_key, add_prefab_default_name, build_default_terrain_data,
    current_level_prefab_options, make_override_data, render_prefab_index_picker, render_vec3_row,
};

impl EditorApp {
    /// Add Object dialog.
    pub(in crate::app) fn render_add_obj_dialog(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_add_dialog {
            return;
        }
        let prefab_options = {
            let tab = &self.tabs[self.active_tab];
            current_level_prefab_options(
                tab.level.as_ref(),
                tab.file_name.as_deref(),
                tab.source_path.as_deref(),
            )
        };
        let mut open = true;
        egui::Window::new(t.get("win_add_object"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let previous_data_type = self.add_obj_data_type;
                ui.horizontal(|ui| {
                    ui.label(t.get("add_type"));
                    ui.radio_value(&mut self.add_obj_is_parent, false, "Prefab");
                    ui.radio_value(&mut self.add_obj_is_parent, true, "Parent");
                    if !self.add_obj_is_parent {
                        ui.separator();
                        ui.label(t.get("add_data_type"));
                        egui::ComboBox::from_id_salt("add_object_data_type")
                            .selected_text(t.get(add_data_type_key(self.add_obj_data_type)))
                            .show_ui(ui, |ui| {
                                for data_type in
                                    [DataType::None, DataType::Terrain, DataType::PrefabOverrides]
                                {
                                    ui.selectable_value(
                                        &mut self.add_obj_data_type,
                                        data_type,
                                        t.get(add_data_type_key(data_type)),
                                    );
                                }
                            });
                    }
                });
                if !self.add_obj_is_parent
                    && self.add_obj_data_type != previous_data_type
                    && (self.add_obj_name.is_empty()
                        || self.add_obj_name == add_prefab_default_name(previous_data_type))
                {
                    self.add_obj_name = add_prefab_default_name(self.add_obj_data_type).to_string();
                }
                ui.horizontal(|ui| {
                    ui.label(t.get("add_name"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.add_obj_name).hint_text(
                            if self.add_obj_is_parent {
                                "NewObject"
                            } else {
                                add_prefab_default_name(self.add_obj_data_type)
                            },
                        ),
                    );
                });
                if !self.add_obj_is_parent {
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_prefab_index"));
                        render_prefab_index_picker(
                            ui,
                            "add_object_prefab_index",
                            &mut self.add_obj_prefab_index,
                            &prefab_options,
                        );
                    });
                }
                ui.separator();
                let position_label = t.get("prop_position");
                let rotation_label = t.get("prop_rotation");
                let scale_label = t.get("prop_scale");
                render_vec3_row(ui, &position_label, &mut self.add_obj_position);
                render_vec3_row(ui, &rotation_label, &mut self.add_obj_rotation);
                render_vec3_row(ui, &scale_label, &mut self.add_obj_scale);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t.get("btn_ok")).clicked() {
                        self.push_undo();
                        let add_name = if self.add_obj_name.trim().is_empty() {
                            if self.add_obj_is_parent {
                                "NewObject".to_string()
                            } else {
                                add_prefab_default_name(self.add_obj_data_type).to_string()
                            }
                        } else {
                            self.add_obj_name.clone()
                        };
                        let is_parent = self.add_obj_is_parent;
                        let data_type = self.add_obj_data_type;
                        let prefab_index = self.add_obj_prefab_index;
                        let position = self.add_obj_position;
                        let rotation = self.add_obj_rotation;
                        let scale = self.add_obj_scale;
                        let tab = &mut self.tabs[self.active_tab];
                        if let Some(ref mut level) = tab.level {
                            let new_idx = level.objects.len();
                            if is_parent {
                                level.objects.push(LevelObject::Parent(ParentObject {
                                    name: add_name.clone(),
                                    position,
                                    children: Vec::new(),
                                    parent: None,
                                }));
                            } else {
                                let terrain_data = (data_type == DataType::Terrain)
                                    .then(|| Box::new(build_default_terrain_data()));
                                let override_data = (data_type == DataType::PrefabOverrides)
                                    .then(|| make_override_data(String::new()));
                                level.objects.push(LevelObject::Prefab(PrefabInstance {
                                    name: add_name.clone(),
                                    position,
                                    prefab_index,
                                    rotation,
                                    scale,
                                    data_type,
                                    terrain_data,
                                    override_data,
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
