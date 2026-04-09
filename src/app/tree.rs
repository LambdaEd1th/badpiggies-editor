//! Object tree panel with drag-and-drop reordering.

use std::collections::BTreeSet;

use eframe::egui;

use crate::types::*;

use super::EditorApp;

/// Drag-and-drop payload for the object tree.
struct DndPayload(ObjectIndex);

/// Where to drop an item in the tree.
pub enum DropPosition {
    /// Insert before `target` in its parent's children list (or in roots).
    Before(ObjectIndex),
    /// Insert after `target` in its parent's children list (or in roots).
    After(ObjectIndex),
    /// Insert as the last child of a Parent object.
    IntoParent(ObjectIndex),
}

impl EditorApp {
    /// Render the left object tree panel.
    pub(super) fn render_tree_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_object_tree {
            return;
        }
        let t = self.t();
        egui::Panel::left("object_tree")
            .default_size(240.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.heading(t.get("panel_object_list"));
                ui.separator();

                let mut drop_action: Option<(ObjectIndex, DropPosition)> = None;
                let mut tree_clicked: Option<ObjectIndex> = None;
                let sel_snapshot = self.tabs[self.active_tab].selected.clone();
                if let Some(ref level) = self.tabs[self.active_tab].level {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            for &root_idx in &level.roots {
                                let (dr, cl) =
                                    show_object_tree(ui, level, root_idx, &sel_snapshot, 0);
                                if dr.is_some() && drop_action.is_none() {
                                    drop_action = dr;
                                }
                                if cl.is_some() && tree_clicked.is_none() {
                                    tree_clicked = cl;
                                }
                            }
                        });
                }
                // Handle click selection (plain / Cmd / Shift)
                if let Some(clicked_idx) = tree_clicked {
                    let tab = &mut self.tabs[self.active_tab];
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.shift {
                        if let (Some(anchor), Some(level)) =
                            (tab.select_anchor, &tab.level)
                        {
                            let mut flat = Vec::new();
                            for &root in &level.roots {
                                collect_tree_order(level, root, &mut flat);
                            }
                            let a_pos = flat.iter().position(|&i| i == anchor);
                            let b_pos = flat.iter().position(|&i| i == clicked_idx);
                            if let (Some(a), Some(b)) = (a_pos, b_pos) {
                                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                                if !modifiers.command {
                                    tab.selected.clear();
                                }
                                for &obj_idx in &flat[lo..=hi] {
                                    tab.selected.insert(obj_idx);
                                }
                            }
                        } else {
                            tab.selected = BTreeSet::from([clicked_idx]);
                            tab.select_anchor = Some(clicked_idx);
                        }
                    } else if modifiers.command {
                        if tab.selected.contains(&clicked_idx) {
                            tab.selected.remove(&clicked_idx);
                        } else {
                            tab.selected.insert(clicked_idx);
                        }
                        tab.select_anchor = Some(clicked_idx);
                    } else {
                        tab.selected = BTreeSet::from([clicked_idx]);
                        tab.select_anchor = Some(clicked_idx);
                    }
                }
                // Handle drop action
                if let Some((source_idx, drop_pos)) = drop_action {
                    self.push_undo();
                    let tab = &mut self.tabs[self.active_tab];
                    if let Some(ref mut level) = tab.level {
                        let new_sel = level.move_object(source_idx, drop_pos);
                        if let Some(ns) = new_sel {
                            tab.selected = BTreeSet::from([ns]);
                        }
                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                    }
                }
            });
    }
}

/// Like `selectable_label` but with `Sense::click_and_drag()` so a single
/// widget handles both click-to-select and drag-to-reorder without conflicts.
pub(super) fn selectable_label_draggable(
    ui: &mut egui::Ui,
    selected: bool,
    text: &str,
) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let total_extra = button_padding + button_padding;
    let wrap_width = ui.available_width() - total_extra.x;
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        wrap_width,
        egui::TextStyle::Button,
    );
    let mut desired_size = total_extra + galley.size();
    desired_size.y = desired_size.y.max(ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());
    if ui.is_rect_visible(response.rect) {
        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), rect.shrink2(button_padding))
            .min;
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() || response.highlighted() {
            let r = rect.expand(visuals.expansion);
            ui.painter().rect(
                r,
                visuals.corner_radius,
                visuals.bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        ui.painter().galley(text_pos, galley, visuals.text_color());
    }
    response
}

/// Recursively render the object tree with drag-and-drop support.
fn show_object_tree(
    ui: &mut egui::Ui,
    level: &LevelData,
    idx: ObjectIndex,
    selected: &BTreeSet<ObjectIndex>,
    depth: usize,
) -> (Option<(ObjectIndex, DropPosition)>, Option<ObjectIndex>) {
    let obj = &level.objects[idx];
    let is_selected = selected.contains(&idx);
    let mut drop_result: Option<(ObjectIndex, DropPosition)> = None;
    let mut clicked: Option<ObjectIndex> = None;

    match obj {
        LevelObject::Parent(parent) => {
            let collapse_id = ui.make_persistent_id(format!("obj_{}", idx));
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
                    let (dr, cl) = show_object_tree(ui, level, child, selected, depth + 1);
                    if dr.is_some() && drop_result.is_none() {
                        drop_result = dr;
                    }
                    if cl.is_some() && clicked.is_none() {
                        clicked = cl;
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
    (drop_result, clicked)
}

/// Collect all object indices in tree display order (depth-first).
fn collect_tree_order(level: &LevelData, idx: ObjectIndex, out: &mut Vec<ObjectIndex>) {
    out.push(idx);
    if let LevelObject::Parent(p) = &level.objects[idx] {
        for &child in &p.children {
            collect_tree_order(level, child, out);
        }
    }
}
