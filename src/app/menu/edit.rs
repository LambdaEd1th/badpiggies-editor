//! Edit menus (level + save tabs).

use eframe::egui;

use crate::locale::I18n;
use crate::types::*;

use super::super::EditorApp;

impl EditorApp {
    pub(super) fn menu_edit(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        ui.menu_button(t.get("menu_edit"), |ui| {
            ui.set_min_width(120.0);
            let has_level = self.tabs[self.active_tab].level.is_some();
            let is_save_tab = self.tabs[self.active_tab].is_save_tab();
            if is_save_tab {
                self.menu_edit_save(ui, t);
                return;
            }
            if !has_level {
                ui.label(t.get("save_viewer_no_data"));
                return;
            }
            let is_mac = cfg!(target_os = "macos");
            let undo_shortcut = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
            let redo_shortcut = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };
            if ui
                .add(egui::Button::new(t.get("menu_undo")).shortcut_text(undo_shortcut))
                .clicked()
            {
                ui.close();
                self.undo();
            }
            if ui
                .add(egui::Button::new(t.get("menu_redo")).shortcut_text(redo_shortcut))
                .clicked()
            {
                ui.close();
                self.redo();
            }
            ui.separator();
            let has_sel = !self.tabs[self.active_tab].selected.is_empty()
                && self.tabs[self.active_tab].level.is_some();
            let has_clip = self.clipboard.is_some() && self.tabs[self.active_tab].level.is_some();
            let copy_shortcut = if is_mac { "Cmd+C" } else { "Ctrl+C" };
            let cut_shortcut = if is_mac { "Cmd+X" } else { "Ctrl+X" };
            let paste_shortcut = if is_mac { "Cmd+V" } else { "Ctrl+V" };
            let dup_shortcut = if is_mac { "Cmd+D" } else { "Ctrl+D" };
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_copy")).shortcut_text(copy_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.copy_selected();
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_cut")).shortcut_text(cut_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.cut_selected();
            }
            if ui
                .add_enabled(
                    has_clip,
                    egui::Button::new(t.get("menu_paste")).shortcut_text(paste_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.paste();
            }
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_duplicate")).shortcut_text(dup_shortcut),
                )
                .clicked()
            {
                ui.close();
                self.duplicate_selected();
            }
            ui.separator();
            if ui
                .add_enabled(
                    has_sel,
                    egui::Button::new(t.get("menu_delete")).shortcut_text("Del"),
                )
                .clicked()
            {
                ui.close();
                if !self.tabs[self.active_tab].selected.is_empty()
                    && let Some(ref level) = self.tabs[self.active_tab].level
                {
                    let indices: Vec<ObjectIndex> = self.tabs[self.active_tab]
                        .selected
                        .iter()
                        .copied()
                        .collect();
                    let label = if indices.len() == 1 {
                        level.objects[indices[0]].name().to_string()
                    } else {
                        format!("{} objects", indices.len())
                    };
                    self.tabs[self.active_tab].pending_delete = Some((indices, label));
                }
            }
            ui.separator();
            if ui.button(t.get("menu_add_object")).clicked() {
                ui.close();
                self.prepare_add_object_dialog();
            }
        });
    }

    /// Edit menu contents for save tabs (undo/redo + select all/delete all entries).
    pub(super) fn menu_edit_save(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        let is_mac = cfg!(target_os = "macos");
        let undo_shortcut = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
        let redo_shortcut = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };

        let can_undo = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.can_undo());
        let can_redo = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.can_redo());

        if ui
            .add_enabled(
                can_undo,
                egui::Button::new(t.get("menu_undo")).shortcut_text(undo_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.undo();
            }
        }
        if ui
            .add_enabled(
                can_redo,
                egui::Button::new(t.get("menu_redo")).shortcut_text(redo_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.redo();
            }
        }
        ui.separator();
        let has_data = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| sv.data.is_some());
        let has_selection = self.tabs[self.active_tab]
            .save_view
            .as_ref()
            .is_some_and(|sv| !sv.selected.is_empty());
        let select_all_shortcut = if is_mac { "Cmd+A" } else { "Ctrl+A" };
        let delete_shortcut = if is_mac { "Delete" } else { "Del" };
        if ui
            .add_enabled(
                has_data,
                egui::Button::new(t.get("menu_select_all")).shortcut_text(select_all_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.select_all();
            }
        }
        if ui.button(t.get("save_edit_deselect_all")).clicked() {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.deselect_all();
            }
        }
        ui.separator();
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(t.get("save_edit_delete_selected"))
                    .shortcut_text(delete_shortcut),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.delete_selected();
            }
        }
        if ui
            .add_enabled(
                has_selection,
                egui::Button::new(t.get("save_edit_duplicate_selected")),
            )
            .clicked()
        {
            ui.close();
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.duplicate_selected();
            }
        }
    }
}
