//! Structured save data table editors — Progress, Contraption, Achievements.

use std::collections::HashSet;

use eframe::egui;

use crate::locale::I18n;
use crate::save_parser::*;

use super::save_viewer::Filter;

pub(super) fn edit_progress(
    filter: &Filter,
    ui: &mut egui::Ui,
    entries: &mut Vec<ProgressEntry>,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    t: &'static I18n,
) -> bool {
    let filtered_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            filter.is_match(&e.key) || filter.is_match(&e.value) || filter.is_match(&e.value_type)
        })
        .map(|(i, _)| i)
        .collect();

    let mut changed = false;
    let mut to_delete: Vec<usize> = Vec::new();

    // Reserve bottom area for add button
    let mut add_clicked = false;
    egui::Panel::bottom("progress_add_btn")
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            if ui.small_button(t.get("save_editor_add_entry")).clicked() {
                add_clicked = true;
            }
        });

    let spacing_x = ui.spacing().item_spacing.x;
    let scroll_w = ui.spacing().scroll.allocated_width();
    let type_w = 80.0_f32;
    let value_w = 100.0_f32;
    let del_w = 24.0_f32;
    let key_w =
        (ui.available_width() - scroll_w - type_w - value_w - del_w - 3.0 * spacing_x).max(100.0);
    let max_height = ui.available_height();

    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .max_scroll_height(max_height)
        .auto_shrink(false)
        .column(egui_extras::Column::exact(key_w).clip(true))
        .column(egui_extras::Column::exact(type_w))
        .column(egui_extras::Column::exact(value_w))
        .column(egui_extras::Column::exact(del_w))
        .sense(egui::Sense::click())
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong(t.get("save_col_key"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_type"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_value"));
            });
            header.col(|_| {});
        })
        .body(|body| {
            body.rows(22.0, filtered_indices.len(), |mut row| {
                let idx = filtered_indices[row.index()];
                row.set_selected(selected.contains(&idx));
                row.col(|ui| {
                    if ui.text_edit_singleline(&mut entries[idx].key).changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    let current = entries[idx].value_type.clone();
                    egui::ComboBox::from_id_salt(format!("ptype_{idx}"))
                        .selected_text(&current)
                        .width(70.0)
                        .show_ui(ui, |ui| {
                            for &ty in &["Int32", "Boolean", "Single", "String"] {
                                if ui
                                    .selectable_value(
                                        &mut entries[idx].value_type,
                                        ty.to_string(),
                                        ty,
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                        });
                });
                row.col(|ui| {
                    if entries[idx].value_type == "Boolean" {
                        let mut b = entries[idx].value.eq_ignore_ascii_case("true");
                        if ui.checkbox(&mut b, "").changed() {
                            entries[idx].value = if b { "True".into() } else { "False".into() };
                            changed = true;
                        }
                    } else {
                        if ui.text_edit_singleline(&mut entries[idx].value).changed() {
                            changed = true;
                        }
                    }
                });
                row.col(|ui| {
                    if ui.small_button("×").clicked() {
                        to_delete.push(idx);
                    }
                });
                let resp = row.response();
                if resp.clicked() {
                    handle_row_click(
                        &resp.ctx.input(|i| i.modifiers),
                        idx,
                        selected,
                        last_clicked,
                        &filtered_indices,
                    );
                }
            });
        });

    to_delete.sort_unstable();
    for idx in to_delete.into_iter().rev() {
        entries.remove(idx);
        changed = true;
    }

    if add_clicked {
        entries.push(ProgressEntry {
            key: String::new(),
            value_type: "Int32".into(),
            value: "0".into(),
        });
        changed = true;
    }

    changed
}

pub(super) fn edit_contraption(
    filter: &Filter,
    ui: &mut egui::Ui,
    parts: &mut Vec<ContraptionPart>,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    t: &'static I18n,
) -> bool {
    ui.label(format!(
        "{}: {}",
        t.get("save_viewer_part_count"),
        parts.len()
    ));

    if !parts.is_empty() {
        let min_x = parts.iter().map(|p| p.x).min().unwrap();
        let max_x = parts.iter().map(|p| p.x).max().unwrap();
        let min_y = parts.iter().map(|p| p.y).min().unwrap();
        let max_y = parts.iter().map(|p| p.y).max().unwrap();
        ui.label(format!("Grid: X [{min_x}, {max_x}]  Y [{min_y}, {max_y}]"));
    }
    ui.separator();

    let filtered_indices: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            filter.is_match(&p.part_type.to_string())
                || filter.is_match(&p.x.to_string())
                || filter.is_match(&p.y.to_string())
                || filter.is_match(&p.rot.to_string())
                || filter.is_match(if p.flipped { "true" } else { "false" })
        })
        .map(|(i, _)| i)
        .collect();

    let mut changed = false;
    let mut to_delete: Vec<usize> = Vec::new();

    // Reserve bottom area for add button
    let mut add_clicked = false;
    egui::Panel::bottom("contraption_add_btn")
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            if ui.small_button(t.get("save_editor_add_entry")).clicked() {
                add_clicked = true;
            }
        });

    let spacing_x = ui.spacing().item_spacing.x;
    let scroll_w = ui.spacing().scroll.allocated_width();
    let x_w = 50.0_f32;
    let y_w = 50.0_f32;
    let ci_w = 50.0_f32;
    let rot_w = 50.0_f32;
    let flip_w = 40.0_f32;
    let del_w = 24.0_f32;
    let pt_w = (ui.available_width()
        - scroll_w
        - x_w
        - y_w
        - ci_w
        - rot_w
        - flip_w
        - del_w
        - 6.0 * spacing_x)
        .max(60.0);
    let max_height = ui.available_height();

    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .max_scroll_height(max_height)
        .auto_shrink(false)
        .column(egui_extras::Column::exact(x_w))
        .column(egui_extras::Column::exact(y_w))
        .column(egui_extras::Column::exact(pt_w).clip(true))
        .column(egui_extras::Column::exact(ci_w))
        .column(egui_extras::Column::exact(rot_w))
        .column(egui_extras::Column::exact(flip_w))
        .column(egui_extras::Column::exact(del_w))
        .sense(egui::Sense::click())
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("X");
            });
            header.col(|ui| {
                ui.strong("Y");
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_part_type"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_custom_idx"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_rot"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_flipped"));
            });
            header.col(|_| {});
        })
        .body(|body| {
            body.rows(22.0, filtered_indices.len(), |mut row| {
                let idx = filtered_indices[row.index()];
                row.set_selected(selected.contains(&idx));
                row.col(|ui| {
                    if ui.add(egui::DragValue::new(&mut parts[idx].x)).changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.add(egui::DragValue::new(&mut parts[idx].y)).changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui
                        .add(egui::DragValue::new(&mut parts[idx].part_type))
                        .changed()
                    {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui
                        .add(egui::DragValue::new(&mut parts[idx].custom_part_index))
                        .changed()
                    {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui
                        .add(egui::DragValue::new(&mut parts[idx].rot).range(0..=3))
                        .changed()
                    {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.checkbox(&mut parts[idx].flipped, "").changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.small_button("×").clicked() {
                        to_delete.push(idx);
                    }
                });
                let resp = row.response();
                if resp.clicked() {
                    handle_row_click(
                        &resp.ctx.input(|i| i.modifiers),
                        idx,
                        selected,
                        last_clicked,
                        &filtered_indices,
                    );
                }
            });
        });

    to_delete.sort_unstable();
    for idx in to_delete.into_iter().rev() {
        parts.remove(idx);
        changed = true;
    }

    if add_clicked {
        parts.push(ContraptionPart {
            x: 0,
            y: 0,
            part_type: 0,
            custom_part_index: 0,
            rot: 0,
            flipped: false,
        });
        changed = true;
    }

    changed
}

pub(super) fn edit_achievements(
    filter: &Filter,
    ui: &mut egui::Ui,
    entries: &mut Vec<AchievementEntry>,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    t: &'static I18n,
) -> bool {
    let completed_count = entries.iter().filter(|e| e.completed).count();
    ui.label(format!(
        "{completed_count} / {} {}",
        entries.len(),
        t.get("save_viewer_completed")
    ));
    ui.separator();

    let filtered_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| filter.is_match(&e.id))
        .map(|(i, _)| i)
        .collect();

    let mut changed = false;
    let mut to_delete: Vec<usize> = Vec::new();

    // Reserve bottom area for add button
    let mut add_clicked = false;
    egui::Panel::bottom("achievements_add_btn")
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            if ui.small_button(t.get("save_editor_add_entry")).clicked() {
                add_clicked = true;
            }
        });

    let spacing_x = ui.spacing().item_spacing.x;
    let scroll_w = ui.spacing().scroll.allocated_width();
    let prog_w = 70.0_f32;
    let comp_w = 70.0_f32;
    let sync_w = 50.0_f32;
    let del_w = 24.0_f32;
    let id_w =
        (ui.available_width() - scroll_w - prog_w - comp_w - sync_w - del_w - 4.0 * spacing_x)
            .max(100.0);
    let max_height = ui.available_height();

    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .max_scroll_height(max_height)
        .auto_shrink(false)
        .column(egui_extras::Column::exact(id_w).clip(true))
        .column(egui_extras::Column::exact(prog_w))
        .column(egui_extras::Column::exact(comp_w))
        .column(egui_extras::Column::exact(sync_w))
        .column(egui_extras::Column::exact(del_w))
        .sense(egui::Sense::click())
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("ID");
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_progress"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_completed"));
            });
            header.col(|ui| {
                ui.strong(t.get("save_col_synced"));
            });
            header.col(|_| {});
        })
        .body(|body| {
            body.rows(22.0, filtered_indices.len(), |mut row| {
                let idx = filtered_indices[row.index()];
                row.set_selected(selected.contains(&idx));
                row.col(|ui| {
                    if ui.text_edit_singleline(&mut entries[idx].id).changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui
                        .add(egui::DragValue::new(&mut entries[idx].progress).speed(0.01))
                        .changed()
                    {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.checkbox(&mut entries[idx].completed, "").changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.checkbox(&mut entries[idx].synced, "").changed() {
                        changed = true;
                    }
                });
                row.col(|ui| {
                    if ui.small_button("×").clicked() {
                        to_delete.push(idx);
                    }
                });
                let resp = row.response();
                if resp.clicked() {
                    handle_row_click(
                        &resp.ctx.input(|i| i.modifiers),
                        idx,
                        selected,
                        last_clicked,
                        &filtered_indices,
                    );
                }
            });
        });

    to_delete.sort_unstable();
    for idx in to_delete.into_iter().rev() {
        entries.remove(idx);
        changed = true;
    }

    if add_clicked {
        entries.push(AchievementEntry {
            id: String::new(),
            progress: 0.0,
            completed: false,
            synced: false,
        });
        changed = true;
    }

    changed
}

/// Remove entries at the given indices (in reverse order to keep indices valid).
pub(super) fn remove_indices<T>(vec: &mut Vec<T>, indices: &HashSet<usize>) {
    let mut sorted: Vec<usize> = indices.iter().copied().collect();
    sorted.sort_unstable();
    for idx in sorted.into_iter().rev() {
        if idx < vec.len() {
            vec.remove(idx);
        }
    }
}

/// Duplicate entries at the given indices, appending copies at the end.
pub(super) fn duplicate_indices<T: Clone>(vec: &mut Vec<T>, indices: &HashSet<usize>) {
    let mut sorted: Vec<usize> = indices.iter().copied().collect();
    sorted.sort_unstable();
    let cloned: Vec<T> = sorted
        .into_iter()
        .filter_map(|i| vec.get(i).cloned())
        .collect();
    vec.extend(cloned);
}

/// Handle row click with modifier keys for selection.
fn handle_row_click(
    modifiers: &egui::Modifiers,
    actual_idx: usize,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    filtered_indices: &[usize],
) {
    if modifiers.command {
        // Cmd/Ctrl+click: toggle
        if !selected.remove(&actual_idx) {
            selected.insert(actual_idx);
        }
    } else if modifiers.shift {
        // Shift+click: range select
        if let Some(anchor) = *last_clicked {
            // Find positions in the filtered list
            let pos_anchor = filtered_indices.iter().position(|&i| i == anchor);
            let pos_current = filtered_indices.iter().position(|&i| i == actual_idx);
            if let (Some(a), Some(b)) = (pos_anchor, pos_current) {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                for &idx in &filtered_indices[lo..=hi] {
                    selected.insert(idx);
                }
            } else {
                selected.clear();
                selected.insert(actual_idx);
            }
        } else {
            selected.clear();
            selected.insert(actual_idx);
        }
    } else {
        // Plain click: exclusive select
        selected.clear();
        selected.insert(actual_idx);
    }
    *last_clicked = Some(actual_idx);
}
