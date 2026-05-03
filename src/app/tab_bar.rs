//! Tab bar rendering with drag-and-drop reordering.

use eframe::egui;

use crate::i18n::locale::I18n;

use super::state::Tab;
use super::tree;
use super::EditorApp;

/// Render the tab bar with drag-and-drop reordering.
impl EditorApp {
    pub(super) fn render_tab_bar(&mut self, ui: &mut egui::Ui, t: &'static I18n, ctx: &egui::Context) {
        /// Drag-and-drop payload for tab reordering.
        struct TabDndPayload(usize);

        let _ = ctx;
        egui::Panel::top("tab_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let mut close_idx: Option<usize> = None;
                let mut tab_swap: Option<(usize, usize)> = None;
                for i in 0..self.tabs.len() {
                    let title = self.tabs[i].title(&t.get("tab_untitled"));
                    let is_active = i == self.active_tab;

                    let resp = tree::selectable_label_draggable(ui, is_active, &title);
                    if resp.clicked() {
                        self.active_tab = i;
                    }
                    if resp.dragged() {
                        resp.dnd_set_drag_payload(TabDndPayload(i));
                    }

                    // Drop target: reorder tabs
                    if let Some(_payload) = resp.dnd_hover_payload::<TabDndPayload>() {
                        let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                        let mid_x = resp.rect.center().x;
                        let hover_right =
                            ui.input(|inp| inp.pointer.hover_pos().map_or(false, |p| p.x > mid_x));
                        let x = if hover_right {
                            resp.rect.right()
                        } else {
                            resp.rect.left()
                        };
                        ui.painter().vline(x, resp.rect.y_range(), stroke);
                    }
                    if let Some(payload) = resp.dnd_release_payload::<TabDndPayload>() {
                        if payload.0 != i {
                            let mid_x = resp.rect.center().x;
                            let drop_right = ui.input(|inp| {
                                inp.pointer.hover_pos().map_or(false, |p| p.x > mid_x)
                            });
                            let target = if drop_right { i + 1 } else { i };
                            tab_swap = Some((payload.0, target));
                        }
                    }

                    let close_btn = ui.small_button("×");
                    if close_btn.clicked() {
                        close_idx = Some(i);
                    }
                    ui.add_space(4.0);
                    if resp.middle_clicked() {
                        close_idx = Some(i);
                    }
                    resp.context_menu(|ui| {
                        if ui.button(t.get("menu_close_tab")).clicked() {
                            close_idx = Some(i);
                            ui.close();
                        }
                    });
                }
                // Apply tab reorder
                if let Some((from, to)) = tab_swap {
                    let insert_at = if from < to {
                        (to - 1).min(self.tabs.len() - 1)
                    } else {
                        to
                    };
                    if insert_at != from {
                        let tab = self.tabs.remove(from);
                        let insert_at = if from < to {
                            (to - 1).min(self.tabs.len())
                        } else {
                            to
                        };
                        self.tabs.insert(insert_at, tab);
                        if self.active_tab == from {
                            self.active_tab = insert_at;
                        } else if from < self.active_tab && self.active_tab <= insert_at {
                            self.active_tab -= 1;
                        } else if insert_at <= self.active_tab && self.active_tab < from {
                            self.active_tab += 1;
                        }
                    }
                }
                if let Some(idx) = close_idx {
                    self.close_tab(idx);
                }
                // "+" button to add a new empty tab
                if ui.button("+").clicked() {
                    let new_renderer = self.tabs[self.active_tab].renderer.clone_for_new_tab();
                    let new_tab = Tab::new(new_renderer, self.lang.i18n().get("status_welcome"));
                    self.tabs.push(new_tab);
                    self.active_tab = self.tabs.len() - 1;
                }
            });
        });
    }

}
