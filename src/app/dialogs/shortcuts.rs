//! Shortcuts help window.

use eframe::egui;

use super::super::EditorApp;

impl EditorApp {
    /// Shortcuts help window.
    pub(in crate::app) fn render_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let t = self.t();
        let is_mac = cfg!(target_os = "macos");

        // Platform-aware shortcut key labels
        let cmd_click = if is_mac { "Cmd+Click" } else { "Ctrl+Click" };
        let undo_key = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
        let redo_key = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };
        let copy_key = if is_mac { "Cmd+C" } else { "Ctrl+C" };
        let cut_key = if is_mac { "Cmd+X" } else { "Ctrl+X" };
        let paste_key = if is_mac { "Cmd+V" } else { "Ctrl+V" };
        let dup_key = if is_mac { "Cmd+D" } else { "Ctrl+D" };
        let delete_key = if is_mac { "Delete" } else { "Delete" };

        // Add tool mode shortcuts to the shortcuts window
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

                        // ── Mouse ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_mouse"));
                        ui.label("");
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
                        ui.label(cmd_click);
                        ui.label(t.get("shortcuts_cmd_click_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_shift_click"));
                        ui.label(t.get("shortcuts_shift_click_action"));
                        ui.end_row();

                        // ── Keyboard Shortcuts ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_keyboard"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_b_key"));
                        ui.label(t.get("shortcuts_toggle_bg"));
                        ui.end_row();
                        ui.label(undo_key);
                        ui.label(t.get("shortcuts_undo_action"));
                        ui.end_row();
                        ui.label(redo_key);
                        ui.label(t.get("shortcuts_redo_action"));
                        ui.end_row();
                        ui.label(copy_key);
                        ui.label(t.get("shortcuts_copy_action"));
                        ui.end_row();
                        ui.label(cut_key);
                        ui.label(t.get("shortcuts_cut_action"));
                        ui.end_row();
                        ui.label(paste_key);
                        ui.label(t.get("shortcuts_paste_action"));
                        ui.end_row();
                        ui.label(dup_key);
                        ui.label(t.get("shortcuts_duplicate_action"));
                        ui.end_row();
                        ui.label(delete_key);
                        ui.label(t.get("shortcuts_delete_action"));
                        ui.end_row();

                        // ── Tool Modes ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_tools"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_select"));
                        ui.label(t.get("tool_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_box_select"));
                        ui.label(t.get("tool_box_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_draw_terrain"));
                        ui.label(t.get("tool_draw_terrain"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_pan"));
                        ui.label(t.get("tool_pan"));
                        ui.end_row();

                        // ── Terrain Editing ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_terrain"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_select"));
                        ui.label(t.get("shortcuts_terrain_select_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_drag"));
                        ui.label(t.get("shortcuts_terrain_drag_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_dblclick"));
                        ui.label(t.get("shortcuts_terrain_dblclick_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_delete"));
                        ui.label(t.get("shortcuts_terrain_delete_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_rclick"));
                        ui.label(t.get("shortcuts_terrain_rclick_action"));
                        ui.end_row();
                    });
            });
    }
}
