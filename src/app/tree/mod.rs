use std::collections::BTreeSet;

use eframe::egui;

use crate::domain::types::*;

use super::EditorApp;

/// Drag-and-drop payload for the object tree.
pub(super) struct DndPayload(ObjectIndex);

pub(super) enum TreeContextAction {
    Copy(Vec<ObjectIndex>),
    Cut(Vec<ObjectIndex>),
    Duplicate(Vec<ObjectIndex>),
    Delete(Vec<ObjectIndex>),
}

pub(super) enum TreeBlankAction {
    AddObject,
    Paste(PastePosition),
    ExpandAll,
    CollapseAll,
    ClearSelection,
}

const OBJECT_TREE_TAIL_BLANK_HEIGHT: f32 = 48.0;

use crate::domain::types::DropPosition;

pub(super) fn tree_collapse_id(idx: ObjectIndex) -> egui::Id {
    egui::Id::new("object_tree_collapsing").with(idx)
}

fn set_tree_expanded_recursive(
    ctx: &egui::Context,
    level: &LevelData,
    idx: ObjectIndex,
    expanded: bool,
) {
    if let LevelObject::Parent(parent) = &level.objects[idx] {
        let collapse_id = tree_collapse_id(idx);
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            collapse_id,
            false,
        );
        state.set_open(expanded);
        state.store(ctx);

        for &child in &parent.children {
            set_tree_expanded_recursive(ctx, level, child, expanded);
        }
    }
}

fn set_tree_expanded_all(ctx: &egui::Context, level: &LevelData, expanded: bool) {
    for &root in &level.roots {
        set_tree_expanded_recursive(ctx, level, root, expanded);
    }
}

pub(super) fn handle_tree_blank_response(
    response: &egui::Response,
    can_paste: bool,
    has_selection: bool,
    paste_position: PastePosition,
    t: &'static crate::i18n::locale::I18n,
    blank_action: &mut Option<TreeBlankAction>,
) {
    if response.clicked() {
        *blank_action = Some(TreeBlankAction::ClearSelection);
    }
    response.context_menu(|ui| {
        if ui
            .add_enabled(can_paste, egui::Button::new(t.get("menu_paste")))
            .clicked()
        {
            *blank_action = Some(TreeBlankAction::Paste(paste_position));
            ui.close();
        }
        ui.separator();
        if ui.button(t.get("menu_expand_all")).clicked() {
            *blank_action = Some(TreeBlankAction::ExpandAll);
            ui.close();
        }
        if ui.button(t.get("menu_collapse_all")).clicked() {
            *blank_action = Some(TreeBlankAction::CollapseAll);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(t.get("menu_clear_selection")),
            )
            .clicked()
        {
            *blank_action = Some(TreeBlankAction::ClearSelection);
            ui.close();
        }
        ui.separator();
        if ui.button(t.get("menu_add_object")).clicked() {
            *blank_action = Some(TreeBlankAction::AddObject);
            ui.close();
        }
    });
}

fn root_end_paste_position(level: &LevelData) -> PastePosition {
    level
        .roots
        .last()
        .copied()
        .map_or(PastePosition::AppendTo(None), |idx| {
            PastePosition::Exact(DropPosition::After(idx))
        })
}

pub(super) fn row_blank_paste_position(
    response: &egui::Response,
    row_rect: egui::Rect,
    idx: ObjectIndex,
    allows_into_parent: bool,
) -> PastePosition {
    let frac = response
        .interact_pointer_pos()
        .map(|pointer| ((pointer.y - row_rect.top()) / row_rect.height()).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let drop_pos = if allows_into_parent {
        if frac < 0.25 {
            DropPosition::Before(idx)
        } else if frac > 0.75 {
            DropPosition::After(idx)
        } else {
            DropPosition::IntoParent(idx)
        }
    } else if frac < 0.5 {
        DropPosition::Before(idx)
    } else {
        DropPosition::After(idx)
    };
    PastePosition::Exact(drop_pos)
}

pub(super) fn handle_tree_item_context_menu(
    response: &egui::Response,
    context_indices: &[ObjectIndex],
    t: &'static crate::i18n::locale::I18n,
    context_action: &mut Option<TreeContextAction>,
) {
    response.context_menu(|ui| {
        if ui.button(t.get("menu_copy")).clicked() {
            *context_action = Some(TreeContextAction::Copy(context_indices.to_vec()));
            ui.close();
        }
        if ui.button(t.get("menu_cut")).clicked() {
            *context_action = Some(TreeContextAction::Cut(context_indices.to_vec()));
            ui.close();
        }
        if ui.button(t.get("menu_duplicate")).clicked() {
            *context_action = Some(TreeContextAction::Duplicate(context_indices.to_vec()));
            ui.close();
        }
        ui.separator();
        if ui.button(t.get("menu_delete")).clicked() {
            *context_action = Some(TreeContextAction::Delete(context_indices.to_vec()));
            ui.close();
        }
    });
}

pub(super) fn interact_tree_row_blank(
    ui: &mut egui::Ui,
    idx: ObjectIndex,
    row_rect: egui::Rect,
) -> Option<egui::Response> {
    let panel_right = ui.max_rect().right();
    if row_rect.right() >= panel_right {
        return None;
    }
    let blank_rect = egui::Rect::from_min_max(
        egui::pos2(row_rect.right(), row_rect.top()),
        egui::pos2(panel_right, row_rect.bottom()),
    );
    Some(ui.interact(
        blank_rect,
        ui.id().with("object_tree_row_blank").with(idx),
        egui::Sense::click(),
    ))
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
                let mut context_action: Option<TreeContextAction> = None;
                let mut blank_action: Option<TreeBlankAction> = None;
                let can_paste = self.clipboard.is_some();
                let has_selection = !self.tabs[self.active_tab].selected.is_empty();
                let sel_snapshot = self.tabs[self.active_tab].selected.clone();
                if let Some(ref level) = self.tabs[self.active_tab].level {
                    let tree_ctx = TreeRenderCtx {
                        selected: &sel_snapshot,
                        can_paste,
                        has_selection,
                        t,
                    };
                    let root_end_paste = root_end_paste_position(level);
                    let scroll_output =
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                for &root_idx in &level.roots {
                                    let (dr, cl, action, row_blank_action) =
                                        show_object_tree(ui, level, root_idx, 0, &tree_ctx);
                                    if dr.is_some() && drop_action.is_none() {
                                        drop_action = dr;
                                    }
                                    if cl.is_some() && tree_clicked.is_none() {
                                        tree_clicked = cl;
                                    }
                                    if action.is_some() && context_action.is_none() {
                                        context_action = action;
                                    }
                                    if row_blank_action.is_some() && blank_action.is_none() {
                                        blank_action = row_blank_action;
                                    }
                                }

                                let (_tail_blank_rect, tail_blank_response) = ui
                                    .allocate_exact_size(
                                        egui::vec2(
                                            ui.available_width().max(1.0),
                                            OBJECT_TREE_TAIL_BLANK_HEIGHT,
                                        ),
                                        egui::Sense::click(),
                                    );
                                handle_tree_blank_response(
                                    &tail_blank_response,
                                    can_paste,
                                    has_selection,
                                    root_end_paste,
                                    t,
                                    &mut blank_action,
                                );
                            });

                    let blank_top = (scroll_output.inner_rect.top() + scroll_output.content_size.y
                        - scroll_output.state.offset.y)
                        .clamp(
                            scroll_output.inner_rect.top(),
                            scroll_output.inner_rect.bottom(),
                        );
                    let blank_rect = egui::Rect::from_min_max(
                        egui::pos2(scroll_output.inner_rect.left(), blank_top),
                        scroll_output.inner_rect.right_bottom(),
                    );
                    if blank_rect.height() > 0.0 {
                        let blank_response = ui.interact(
                            blank_rect,
                            ui.id().with("object_tree_blank_menu"),
                            egui::Sense::click(),
                        );
                        handle_tree_blank_response(
                            &blank_response,
                            can_paste,
                            has_selection,
                            root_end_paste,
                            t,
                            &mut blank_action,
                        );
                    }
                }
                // Handle click selection (plain / Cmd / Shift)
                if let Some(clicked_idx) = tree_clicked {
                    let tab = &mut self.tabs[self.active_tab];
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.shift {
                        if let (Some(anchor), Some(level)) = (tab.select_anchor, &tab.level) {
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
                if let Some(action) = context_action {
                    match action {
                        TreeContextAction::Copy(indices) => self.copy_objects(&indices),
                        TreeContextAction::Cut(indices) => self.cut_objects(&indices),
                        TreeContextAction::Duplicate(indices) => self.duplicate_objects(&indices),
                        TreeContextAction::Delete(indices) => {
                            self.request_delete_objects(&indices);
                        }
                    }
                }
                if let Some(action) = blank_action {
                    match action {
                        TreeBlankAction::AddObject => {
                            self.prepare_add_object_dialog();
                        }
                        TreeBlankAction::Paste(paste_position) => {
                            self.paste_with_context(&[], None, Some(paste_position));
                        }
                        TreeBlankAction::ExpandAll => {
                            if let Some(level) = self.tabs[self.active_tab].level.as_ref() {
                                set_tree_expanded_all(ui.ctx(), level, true);
                            }
                        }
                        TreeBlankAction::CollapseAll => {
                            if let Some(level) = self.tabs[self.active_tab].level.as_ref() {
                                set_tree_expanded_all(ui.ctx(), level, false);
                            }
                        }
                        TreeBlankAction::ClearSelection => {
                            self.tabs[self.active_tab].selected.clear();
                            self.tabs[self.active_tab].select_anchor = None;
                        }
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

mod show;

use show::{TreeRenderCtx, collect_tree_order, show_object_tree};
