//! Properties panel — editable object properties and override tree editor.

mod overrides;

use eframe::egui;

use crate::locale::I18n;
use crate::types::*;

use super::{
    dialogs::{
        build_default_terrain_data, current_level_prefab_options,
        render_prefab_index_picker, PrefabOption,
    },
    EditorApp, Snapshot, UNDO_MAX,
};

use overrides::{parse_override_text, serialize_override_tree, show_override_tree};

impl EditorApp {
    /// Render the right properties panel.
    pub(super) fn render_properties_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_properties {
            return;
        }
        let t = self.t();
        egui::Panel::right("properties")
            .default_size(280.0)
            .size_range(120.0..=f32::INFINITY)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().text_edit_width = f32::INFINITY;
                ui.heading(t.get("panel_properties"));
                ui.separator();

                let tab = &mut self.tabs[self.active_tab];
                let prefab_options = current_level_prefab_options(
                    tab.level.as_ref(),
                    tab.file_name.as_deref(),
                    tab.source_path.as_deref(),
                );
                let single_sel = if tab.selected.len() == 1 {
                    tab.selected.iter().next().copied()
                } else {
                    None
                };
                if let (Some(level), Some(sel)) = (&mut tab.level, single_sel) {
                    if sel < level.objects.len() {
                        let pre_obj = if !tab.props_changed_prev {
                            Some(level.objects[sel].clone())
                        } else {
                            None
                        };
                        let changed =
                            show_properties_editable(ui, &mut level.objects[sel], &prefab_options, t);
                        if changed {
                            if let Some(obj_backup) = pre_obj {
                                let mut level_snapshot = level.clone();
                                level_snapshot.objects[sel] = obj_backup;
                                tab.history.undo.push(Snapshot {
                                    level: level_snapshot,
                                    selected: tab.selected.clone(),
                                });
                                if tab.history.undo.len() > UNDO_MAX {
                                    tab.history.undo.remove(0);
                                }
                                tab.history.redo.clear();
                            }
                            tab.props_changed_prev = true;
                            let cam = tab.renderer.camera.clone();
                            tab.renderer.set_level(level);
                            tab.renderer.camera = cam;
                        } else {
                            tab.props_changed_prev = false;
                        }
                    }
                } else {
                    ui.label(t.get("panel_select_hint"));
                }
            });
    }
}

/// Show editable properties. Returns true if anything changed.
fn show_properties_editable(
    ui: &mut egui::Ui,
    obj: &mut LevelObject,
    prefab_options: &[PrefabOption],
    t: &'static I18n,
) -> bool {
    let mut changed = false;
    match obj {
        LevelObject::Prefab(p) => {
            ui.label(t.get("prop_type_prefab"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.horizontal(|ui| {
                ui.label(t.get("prop_prefab_index"));
                changed |= render_prefab_index_picker(
                    ui,
                    "properties_prefab_index",
                    &mut p.prefab_index,
                    prefab_options,
                );
            });
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "p_pos", &mut p.position);

            ui.label(t.get("prop_rotation"));
            changed |= edit_vec3(ui, "p_rot", &mut p.rotation);

            ui.label(t.get("prop_scale"));
            changed |= edit_vec3(ui, "p_scl", &mut p.scale);

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(t.get("prop_data_type"));
                let mut data_type = p.data_type;
                egui::ComboBox::from_id_salt("properties_data_type")
                    .selected_text(data_type_label(data_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut data_type, DataType::None, data_type_label(DataType::None));
                        ui.selectable_value(
                            &mut data_type,
                            DataType::Terrain,
                            data_type_label(DataType::Terrain),
                        );
                        ui.selectable_value(
                            &mut data_type,
                            DataType::PrefabOverrides,
                            data_type_label(DataType::PrefabOverrides),
                        );
                    });
                if data_type != p.data_type {
                    apply_data_type_change(p, data_type);
                    changed = true;
                }
            });

            if let Some(ref mut td) = p.terrain_data {
                ui.separator();
                ui.label(t.get("prop_terrain"));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_fill_vert_count"),
                    td.fill_mesh.vertices.len()
                ));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_curve_vert_count"),
                    td.curve_mesh.vertices.len()
                ));
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_collider"));
                    changed |= ui.checkbox(&mut td.has_collider, "").changed();
                });
                // Fill color
                ui.horizontal(|ui| {
                    ui.label(format!("  {}:", t.get("prop_fill_color")));
                    let rgba = td.fill_color.to_rgba8();
                    let mut color =
                        egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                    if egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut color,
                        egui::color_picker::Alpha::OnlyBlend,
                    )
                    .changed()
                    {
                        td.fill_color = Color {
                            r: color.r() as f32 / 255.0,
                            g: color.g() as f32 / 255.0,
                            b: color.b() as f32 / 255.0,
                            a: color.a() as f32 / 255.0,
                        };
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_tex_index"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_index).range(0..=31))
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_x"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_x).speed(0.01))
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_y"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_y).speed(0.01))
                        .changed();
                });

                // Closed loop toggle
                let nodes = crate::terrain_gen::extract_curve_nodes(td);
                let is_closed = nodes.len() >= 2 && crate::terrain_gen::is_closed_loop(&nodes);
                let mut closed_val = is_closed;
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_terrain_closed"));
                    if ui.checkbox(&mut closed_val, "").changed() {
                        let mut nodes = nodes.clone();
                        if closed_val && !is_closed && nodes.len() >= 2 {
                            nodes.push(crate::terrain_gen::CurveNode {
                                position: nodes[0].position,
                                texture: nodes[0].texture,
                            });
                        } else if !closed_val && is_closed && nodes.len() >= 3 {
                            nodes.pop();
                        }
                        crate::terrain_gen::regenerate_terrain(td, &nodes);
                        changed = true;
                    }
                });

                // Curve textures (strip width / fade threshold)
                for ct_i in 0..td.curve_textures.len() {
                    ui.horizontal(|ui| {
                        ui.label(t.fmt_idx("prop_curve_tex", ct_i));
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("  {}", t.get("prop_strip_width")));
                        if ui
                            .add(
                                egui::DragValue::new(&mut td.curve_textures[ct_i].size.y)
                                    .speed(0.01)
                                    .range(0.01..=5.0),
                            )
                            .changed()
                        {
                            let nodes = crate::terrain_gen::extract_curve_nodes(td);
                            crate::terrain_gen::regenerate_terrain(td, &nodes);
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("  {}", t.get("prop_fade_threshold")));
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut td.curve_textures[ct_i].fade_threshold)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed();
                    });
                }
            }

            if let Some(ref mut od) = p.override_data {
                ui.separator();

                let toggle_id = ui.make_persistent_id("ov_raw_toggle");
                let mut show_raw: bool = ui.data(|d| d.get_temp(toggle_id).unwrap_or(false));
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_override"));
                    if ui
                        .small_button(if show_raw {
                            t.get("btn_visual")
                        } else {
                            t.get("btn_text")
                        })
                        .clicked()
                    {
                        show_raw = !show_raw;
                        ui.data_mut(|d| d.insert_temp(toggle_id, show_raw));
                    }
                });

                if show_raw {
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            if ui.text_edit_multiline(&mut od.raw_text).changed() {
                                od.raw_bytes = od.raw_text.as_bytes().to_vec();
                                changed = true;
                            }
                        });
                } else {
                    let mut tree = parse_override_text(&od.raw_text);
                    let mut tree_changed = false;
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let ovr_root_id = ui.make_persistent_id("ovr_root");
                            tree_changed = show_override_tree(ui, &mut tree, 0, ovr_root_id, t);
                        });
                    if tree_changed {
                        let new_text = serialize_override_tree(&tree, 0);
                        od.raw_text = new_text.clone();
                        od.raw_bytes = new_text.into_bytes();
                        changed = true;
                    }
                }
            }
        }
        LevelObject::Parent(p) => {
            ui.label(t.get("prop_type_parent"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.label(format!(
                "{} {}",
                t.get("prop_child_count"),
                p.children.len()
            ));
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "par_pos", &mut p.position);
        }
    }
    changed
}

/// Editable Vec3 with three DragValue fields. Returns true if changed.
fn edit_vec3(ui: &mut egui::Ui, id_prefix: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.push_id(id_prefix, |ui| {
        ui.horizontal(|ui| {
            ui.label("  X");
            changed |= ui.add(egui::DragValue::new(&mut v.x).speed(0.05)).changed();
            ui.label("Y");
            changed |= ui.add(egui::DragValue::new(&mut v.y).speed(0.05)).changed();
            ui.label("Z");
            changed |= ui.add(egui::DragValue::new(&mut v.z).speed(0.05)).changed();
        });
    });
    changed
}

fn data_type_label(data_type: DataType) -> &'static str {
    match data_type {
        DataType::None => "None",
        DataType::Terrain => "Terrain",
        DataType::PrefabOverrides => "PrefabOverrides",
    }
}

fn apply_data_type_change(prefab: &mut PrefabInstance, data_type: DataType) {
    prefab.data_type = data_type;
    match data_type {
        DataType::None => {
            prefab.terrain_data = None;
            prefab.override_data = None;
        }
        DataType::Terrain => {
            if prefab.terrain_data.is_none() {
                prefab.terrain_data = Some(Box::new(build_default_terrain_data()));
            }
            prefab.override_data = None;
        }
        DataType::PrefabOverrides => {
            prefab.terrain_data = None;
            if prefab.override_data.is_none() {
                prefab.override_data = Some(PrefabOverrideData {
                    raw_text: String::new(),
                    raw_bytes: Vec::new(),
                });
            }
        }
    }
}
