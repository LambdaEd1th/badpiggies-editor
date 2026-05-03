//! Progress entries table editor.

use std::collections::HashSet;

use eframe::egui;

use crate::i18n::locale::I18n;
use crate::io::save::parser::*;

use super::super::save_viewer::Filter;
use super::{duplicate_indices, handle_row_click};

pub(in crate::app) fn edit_progress(
    filter: &Filter,
    ui: &mut egui::Ui,
    entries: &mut Vec<ProgressEntry>,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    scroll_to_xml_entry: &mut Option<usize>,
    highlighted_xml_line: &mut Option<usize>,
    xml_entry_line_offset: usize,
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
    let mut duplicate_selected = false;

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
                if resp.secondary_clicked() && !selected.contains(&idx) {
                    selected.clear();
                    selected.insert(idx);
                    *last_clicked = Some(idx);
                }
                resp.context_menu(|ui| {
                    if ui.button(t.get("save_viewer_reveal_xml")).clicked() {
                        *scroll_to_xml_entry = Some(idx);
                        *highlighted_xml_line = Some(idx + xml_entry_line_offset);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(t.get("menu_select_all")).clicked() {
                        selected.clear();
                        selected.extend(filtered_indices.iter().copied());
                        ui.close();
                    }
                    if ui.button(t.get("save_edit_deselect_all")).clicked() {
                        selected.clear();
                        *last_clicked = None;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !selected.is_empty(),
                            egui::Button::new(t.get("save_edit_duplicate_selected")),
                        )
                        .clicked()
                    {
                        duplicate_selected = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !selected.is_empty(),
                            egui::Button::new(t.get("save_edit_delete_selected")),
                        )
                        .clicked()
                    {
                        to_delete.extend(selected.iter().copied());
                        ui.close();
                    }
                });
            });
        });

    if duplicate_selected {
        duplicate_indices(entries, selected);
        selected.clear();
        *last_clicked = None;
        changed = true;
    }

    to_delete.sort_unstable();
    to_delete.dedup();
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
