//! Recursive object-tree rendering and traversal helpers.

use std::collections::BTreeSet;

use eframe::egui;

use crate::types::*;

use super::{
    DndPayload, TreeBlankAction, TreeContextAction, handle_tree_blank_response,
    handle_tree_item_context_menu, interact_tree_row_blank, row_blank_paste_position,
    selectable_label_draggable, tree_collapse_id,
};

/// Recursively render the object tree with drag-and-drop support.
pub(super) fn show_object_tree(
    ui: &mut egui::Ui,
    level: &LevelData,
    idx: ObjectIndex,
    selected: &BTreeSet<ObjectIndex>,
    depth: usize,
    can_paste: bool,
    has_selection: bool,
    t: &'static crate::locale::I18n,
) -> (
    Option<(ObjectIndex, DropPosition)>,
    Option<ObjectIndex>,
    Option<TreeContextAction>,
    Option<TreeBlankAction>,
) {
    let obj = &level.objects[idx];
    let is_selected = selected.contains(&idx);
    let mut drop_result: Option<(ObjectIndex, DropPosition)> = None;
    let mut clicked: Option<ObjectIndex> = None;
    let mut context_action: Option<TreeContextAction> = None;
    let mut blank_action: Option<TreeBlankAction> = None;
    let context_indices: Vec<ObjectIndex> = if is_selected {
        selected.iter().copied().collect()
    } else {
        vec![idx]
    };

    match obj {
        LevelObject::Parent(parent) => {
            let collapse_id = tree_collapse_id(idx);
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                collapse_id,
                depth < 1,
            );
            let header_res = ui.horizontal(|ui| {
                let label_res = selectable_label_draggable(ui, is_selected, &parent.name);
                if label_res.clicked() {
                    clicked = Some(idx);
                }
                if label_res.dragged() {
                    label_res.dnd_set_drag_payload(DndPayload(idx));
                }
                state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                label_res
            });
            let header_rect = header_res.response.rect;
            handle_tree_item_context_menu(&header_res.inner, &context_indices, t, &mut context_action);
            if let Some(blank_res) = interact_tree_row_blank(ui, idx, header_rect) {
                let paste_position = row_blank_paste_position(&blank_res, header_rect, idx, true);
                handle_tree_blank_response(
                    &blank_res,
                    can_paste,
                    has_selection,
                    paste_position,
                    t,
                    &mut blank_action,
                );
            }

            // Drop target detection on the header
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let in_rect = header_rect.contains(egui::pos2(pointer_pos.x, pointer_pos.y));
                if in_rect {
                    let frac = (pointer_pos.y - header_rect.top()) / header_rect.height();
                    if frac < 0.25 {
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter()
                                .hline(header_rect.x_range(), header_rect.top(), stroke);
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::Before(idx)));
                            }
                        }
                    } else if frac > 0.75 {
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter()
                                .hline(header_rect.x_range(), header_rect.bottom(), stroke);
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::After(idx)));
                            }
                        }
                    } else {
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter().rect_stroke(
                                header_rect,
                                2.0,
                                stroke,
                                egui::StrokeKind::Outside,
                            );
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::IntoParent(idx)));
                            }
                        }
                    }
                }
            }

            // Show children
            state.show_body_indented(&header_res.response, ui, |ui| {
                for &child in &parent.children {
                    let (dr, cl, action, child_blank_action) =
                        show_object_tree(
                            ui,
                            level,
                            child,
                            selected,
                            depth + 1,
                            can_paste,
                            has_selection,
                            t,
                        );
                    if dr.is_some() && drop_result.is_none() {
                        drop_result = dr;
                    }
                    if cl.is_some() && clicked.is_none() {
                        clicked = cl;
                    }
                    if action.is_some() && context_action.is_none() {
                        context_action = action;
                    }
                    if child_blank_action.is_some() && blank_action.is_none() {
                        blank_action = child_blank_action;
                    }
                }
            });
            state.store(ui.ctx());
        }
        LevelObject::Prefab(prefab) => {
            let label = format!("{} [{}]", prefab.name, prefab.prefab_index);
            let label_res = selectable_label_draggable(ui, is_selected, &label);
            if label_res.clicked() {
                clicked = Some(idx);
            }
            if label_res.dragged() {
                label_res.dnd_set_drag_payload(DndPayload(idx));
            }
            handle_tree_item_context_menu(&label_res, &context_indices, t, &mut context_action);
            if let Some(blank_res) = interact_tree_row_blank(ui, idx, label_res.rect) {
                let paste_position = row_blank_paste_position(&blank_res, label_res.rect, idx, false);
                handle_tree_blank_response(
                    &blank_res,
                    can_paste,
                    has_selection,
                    paste_position,
                    t,
                    &mut blank_action,
                );
            }

            // Drop target: upper half = before, lower half = after
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let r = label_res.rect;
                if r.contains(egui::pos2(pointer_pos.x, pointer_pos.y)) {
                    let frac = (pointer_pos.y - r.top()) / r.height();
                    let pos = if frac < 0.5 {
                        DropPosition::Before(idx)
                    } else {
                        DropPosition::After(idx)
                    };
                    if let Some(_payload) = label_res.dnd_hover_payload::<DndPayload>() {
                        let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                        let y = if frac < 0.5 { r.top() } else { r.bottom() };
                        ui.painter().hline(r.x_range(), y, stroke);
                    }
                    if let Some(payload) = label_res.dnd_release_payload::<DndPayload>()
                        && payload.0 != idx
                    {
                        drop_result = Some((payload.0, pos));
                    }
                }
            }
        }
    }
    (drop_result, clicked, context_action, blank_action)
}

/// Collect all object indices in tree display order (depth-first).
pub(super) fn collect_tree_order(level: &LevelData, idx: ObjectIndex, out: &mut Vec<ObjectIndex>) {
    out.push(idx);
    if let LevelObject::Parent(p) = &level.objects[idx] {
        for &child in &p.children {
            collect_tree_order(level, child, out);
        }
    }
}
