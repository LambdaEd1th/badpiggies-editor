//! `impl eframe::App` for EditorApp — main update loop.

use eframe::egui;

use crate::renderer::CursorMode;
use crate::domain::types::*;

use super::state::Tab;
use super::EditorApp;

impl eframe::App for EditorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use egui::{Key, KeyboardShortcut, Modifiers};

        let is_save_tab = self.tabs[self.active_tab].is_save_tab();

        // B key — toggle background visibility (only when no text widget has focus)
        if !is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::B))
        {
            self.tabs[self.active_tab].renderer.show_bg =
                !self.tabs[self.active_tab].renderer.show_bg;
        }

        // Tool mode shortcuts (V/M/P/H) — only when no text widget has focus
        if !is_save_tab && !ctx.egui_wants_keyboard_input() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::V)) {
                self.cursor_mode = CursorMode::Select;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::M)) {
                self.cursor_mode = CursorMode::BoxSelect;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::P)) {
                self.cursor_mode = CursorMode::DrawTerrain;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::H)) {
                self.cursor_mode = CursorMode::Pan;
            }
        }

        // Cmd+Shift+Z / Ctrl+Shift+Z — redo
        if ctx.input_mut(|i| {
            i.consume_shortcut(&KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Z,
            ))
        }) {
            if is_save_tab {
                if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                    sv.redo();
                }
            } else {
                self.redo();
            }
        }
        // Cmd+Z / Ctrl+Z — undo
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)))
        {
            if is_save_tab {
                if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                    sv.undo();
                }
            } else {
                self.undo();
            }
        }
        // Ctrl+Y — redo (alternative)
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y)))
        {
            if is_save_tab {
                if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                    sv.redo();
                }
            } else {
                self.redo();
            }
        }

        // Save tab: Cmd+A — select all entries
        if is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::A))
            })
        {
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.select_all();
            }
        }
        // Save tab: Delete/Backspace — delete selected entries
        if is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
            })
        {
            if let Some(ref mut sv) = self.tabs[self.active_tab].save_view {
                sv.delete_selected();
            }
        }

        // Cmd+C / Ctrl+C — copy
        if !is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                let mut found = false;
                i.events.retain(|e| {
                    if matches!(e, egui::Event::Copy) && !found {
                        found = true;
                        false
                    } else {
                        true
                    }
                });
                found
            })
        {
            self.copy_selected();
        }
        // Cmd+X / Ctrl+X — cut
        if !is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                let mut found = false;
                i.events.retain(|e| {
                    if matches!(e, egui::Event::Cut) && !found {
                        found = true;
                        false
                    } else {
                        true
                    }
                });
                found
            })
        {
            self.cut_selected();
        }
        // Cmd+V / Ctrl+V — paste
        if !is_save_tab
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| {
                let mut found = false;
                i.events.retain(|e| {
                    if matches!(e, egui::Event::Paste(_)) && !found {
                        found = true;
                        false
                    } else {
                        true
                    }
                });
                found
            })
        {
            self.paste();
        }
        // Cmd+D / Ctrl+D — duplicate
        if !is_save_tab
            && ctx.input_mut(|i| {
                i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::D))
            })
        {
            self.duplicate_selected();
        }

        // Cmd+W / Ctrl+W — close tab
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::W)))
        {
            self.close_tab(self.active_tab);
        }

        // Cmd+T / Ctrl+T — new empty tab
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::T)))
        {
            let new_renderer = self.tabs[self.active_tab].renderer.clone_for_new_tab();
            let new_tab = Tab::new(new_renderer, self.lang.i18n().get("status_welcome"));
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        }

        // Handle Delete / Backspace key — queue confirmation dialog
        if !is_save_tab
            && !self.tabs[self.active_tab].selected.is_empty()
            && self.tabs[self.active_tab]
                .renderer
                .hovered_terrain_node
                .is_none()
        {
            let delete_pressed = !ctx.egui_wants_keyboard_input()
                && ctx.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Delete)
                        || i.consume_key(Modifiers::NONE, Key::Backspace)
                });
            if delete_pressed
                && self.tabs[self.active_tab].pending_delete.is_none()
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
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = self.t();

        // The web backend ignores viewport title updates and logs a warning each frame.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let tab_title = self.tabs[self.active_tab].title(&t.get("tab_untitled"));
            let title = if self.gpu_backend.is_empty() {
                format!("Bad Piggies Editor — {tab_title}")
            } else {
                format!(
                    "Bad Piggies Editor — {tab_title} [{backend}]",
                    backend = self.gpu_backend
                )
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }

        self.handle_file_input(ui, &ctx);
        self.render_delete_confirm(&ctx, t);
        self.render_menu_bar(ui, &ctx, t);
        self.render_shortcuts_window(&ctx);
        self.render_about_window(&ctx);
        self.render_tool_window(&ctx, t);
        self.render_add_obj_dialog(&ctx, t);
        self.render_tab_bar(ui, t, &ctx);

        // ── Status bar ──
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.tabs[self.active_tab].status);
                if let Some(mw) = self.tabs[self.active_tab].renderer.mouse_world {
                    ui.separator();
                    ui.label(format!("X: {:.2}  Y: {:.2}", mw.x, mw.y));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref name) = self.tabs[self.active_tab].file_name {
                        ui.label(name);
                    }
                });
            });
        });

        if !self.tabs[self.active_tab].is_save_tab() && self.tabs[self.active_tab].level.is_some() {
            self.render_tree_panel(ui);
            self.render_properties_panel(ui);
        }
        self.render_canvas(ui);

        // Contraption preview floating window
        let tab = &mut self.tabs[self.active_tab];
        if let Some(ref mut sv) = tab.save_view {
            sv.render_contraption_preview(&ctx, t, &mut tab.renderer);
        }
    }
}
