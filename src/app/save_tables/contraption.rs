//! Contraption parts table editor.

use std::collections::HashSet;

use eframe::egui;

use crate::i18n::locale::I18n;
use crate::io::save::parser::*;

use super::super::save_viewer::Filter;
use super::{duplicate_indices, handle_row_click};

pub(in crate::app) fn edit_contraption(
    filter: &Filter,
    ui: &mut egui::Ui,
    parts: &mut Vec<ContraptionPart>,
    selected: &mut HashSet<usize>,
    last_clicked: &mut Option<usize>,
    scroll_to_xml_entry: &mut Option<usize>,
    highlighted_xml_line: &mut Option<usize>,
    xml_entry_line_offset: usize,
    t: &'static I18n,
) -> bool {
    ui.label(format!(
        "{}: {}",
        t.get("save_viewer_part_count"),
        parts.len()
    ));

    if !parts.is_empty() {
        if let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
            parts.iter().map(|p| p.x).min(),
            parts.iter().map(|p| p.x).max(),
            parts.iter().map(|p| p.y).min(),
            parts.iter().map(|p| p.y).max(),
        ) {
            ui.label(format!("Grid: X [{min_x}, {max_x}]  Y [{min_y}, {max_y}]"));
        }
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
    let mut duplicate_selected = false;

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
        duplicate_indices(parts, selected);
        selected.clear();
        *last_clicked = None;
        changed = true;
    }

    to_delete.sort_unstable();
    to_delete.dedup();
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
