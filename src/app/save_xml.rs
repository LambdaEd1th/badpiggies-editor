//! XML text panel — editable XML with line numbers, syntax highlighting, and scroll-to-line.

use eframe::egui;
use eframe::egui::containers::{Popup, PopupCloseBehavior, PopupKind};
use eframe::egui::widgets::TextBuffer;

use crate::i18n::locale::I18n;

use super::save_viewer::{Filter, SaveViewerData};

/// Result of rendering the XML panel.
pub(super) struct XmlPanelResult {
    /// Whether the XML text was modified by the user.
    pub xml_dirty: bool,
}

impl SaveViewerData {
    /// Render the left XML text panel with line numbers, highlighting, and scroll-to support.
    pub(super) fn render_xml_panel(
        &mut self,
        ui: &mut egui::Ui,
        t: &'static I18n,
        filter: &Filter,
    ) -> XmlPanelResult {
        let xml_before_edit = self.xml_text.clone();
        let scroll_target = self.scroll_to_xml_entry.take();
        ui.heading(t.get("save_viewer_raw_xml"));
        let xml_context_menu_id = ui.make_persistent_id("save_xml_context_menu");
        let text_edit_id = ui.make_persistent_id("save_xml_text_edit");
        let xml_entry_line_offset = self.xml_entry_line(0);
        let mut context_menu_layer_id: Option<egui::LayerId> = None;
        let mut xml_context_menu_open = self.xml_context_menu_open;
        let mut xml_context_menu_pos = self.xml_context_menu_pos;
        let mut xml_context_menu_wait_for_release = self.xml_context_menu_wait_for_release;
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
            let layout_job = highlight_xml(ui, text.as_str(), wrap_width, filter);
            ui.ctx().fonts_mut(|f| f.layout_job(layout_job))
        };
        let mut xml_dirty = false;
        let mut text_lost_focus = false;
        let highlighted_line = self.highlighted_xml_line;
        egui::ScrollArea::both()
            .id_salt("save_xml_scroll")
            .show(ui, |ui| {
                let xml_text = &mut self.xml_text;
                let line_count = xml_text.chars().filter(|&c| c == '\n').count() + 1;
                let gutter_digits = ((line_count as f32).log10().floor() as usize + 1).max(2);
                let gutter_char_width = ui.ctx().fonts_mut(|f| {
                    f.glyph_width(&egui::TextStyle::Monospace.resolve(ui.style()), '0')
                });
                let gutter_width = gutter_char_width * (gutter_digits as f32 + 1.0) + 8.0;
                let previous_text_state = egui::TextEdit::load_state(ui.ctx(), text_edit_id);

                ui.horizontal_top(|ui| {
                    let output = egui::TextEdit::multiline(xml_text)
                        .id(text_edit_id)
                        .code_editor()
                        .layouter(&mut layouter)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin {
                            left: gutter_width as i32 as i8,
                            ..Default::default()
                        })
                        .show(ui);

                    let galley = &output.galley;
                    let row_height = if galley.rows.len() > 1 {
                        galley.rows[1].rect().min.y - galley.rows[0].rect().min.y
                    } else {
                        galley.rows.first().map_or(14.0, |r| r.rect().height())
                    };

                    let text_rect = output.response.rect;
                    let painter = ui.painter();
                    let gutter_color = ui.visuals().weak_text_color();
                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let clip = painter.clip_rect();

                    // Highlight the active line
                    if let Some(hl) = highlighted_line {
                        let hl_y = text_rect.min.y + (hl as f32) * row_height;
                        if hl_y + row_height >= clip.min.y && hl_y <= clip.max.y {
                            let hl_rect = egui::Rect::from_min_size(
                                egui::pos2(text_rect.min.x, hl_y),
                                egui::vec2(text_rect.width(), row_height),
                            );
                            painter.rect_filled(
                                hl_rect,
                                0.0,
                                egui::Color32::from_rgba_unmultiplied(255, 200, 0, 30),
                            );
                        }
                    }

                    for i in 0..line_count {
                        let y = text_rect.min.y + (i as f32) * row_height;
                        if y > clip.max.y {
                            break;
                        }
                        if y + row_height < clip.min.y {
                            continue;
                        }
                        let num_str = format!("{}", i + 1);
                        let text_x = text_rect.min.x + gutter_width
                            - 8.0
                            - gutter_char_width * num_str.len() as f32;
                        painter.text(
                            egui::pos2(text_x, y),
                            egui::Align2::LEFT_TOP,
                            &num_str,
                            font_id.clone(),
                            gutter_color,
                        );
                    }

                    if output.response.changed() {
                        xml_dirty = true;
                    }
                    if output.response.lost_focus() {
                        text_lost_focus = true;
                    }

                    let secondary_pressed_on_text = output.response.response.hovered()
                        && ui.input(|input| {
                            input.pointer.button_pressed(egui::PointerButton::Secondary)
                        });

                    if secondary_pressed_on_text {
                        if let Some(previous_state) = previous_text_state.clone()
                            && let Some(range) = previous_state.cursor.char_range()
                            && !range.is_empty()
                        {
                            let mut restored_state = output.state.clone();
                            restored_state.cursor = previous_state.cursor;
                            restored_state.store(ui.ctx(), text_edit_id);
                            output.response.response.request_focus();

                            if let Some(previous_range) = previous_state.cursor.range(galley) {
                                let mut overlay_galley = output.galley.clone();
                                egui::text_selection::visuals::paint_text_selection(
                                    &mut overlay_galley,
                                    ui.visuals(),
                                    &previous_range,
                                    None,
                                );
                                ui.painter()
                                    .with_clip_rect(output.text_clip_rect.expand(1.0))
                                    .galley(
                                        output.galley_pos
                                            - egui::vec2(overlay_galley.rect.left(), 0.0),
                                        overlay_galley,
                                        ui.visuals().text_color(),
                                    );
                            }
                        }

                        xml_context_menu_open = true;
                        xml_context_menu_pos = ui.ctx().pointer_interact_pos();
                        xml_context_menu_wait_for_release = true;
                        ui.ctx().request_repaint();
                    }

                    context_menu_layer_id = Some(output.response.response.layer_id);

                    // Scroll to target entry line
                    if let Some(entry_idx) = scroll_target {
                        let target_line = entry_idx + xml_entry_line_offset;
                        let char_offset = char_offset_of_line(xml_text.as_str(), target_line);
                        let ccursor = egui::text::CCursor::new(char_offset);
                        let mut state = output.state.clone();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), text_edit_id);

                        // Scroll the ScrollArea to the target line
                        let y = text_rect.min.y + (target_line as f32) * row_height;
                        let target_rect = egui::Rect::from_min_size(
                            egui::pos2(text_rect.min.x, y),
                            egui::vec2(1.0, row_height),
                        );
                        ui.scroll_to_rect(target_rect, Some(egui::Align::Center));
                    }
                });
            });
        if xml_context_menu_wait_for_release && !ui.input(|input| input.pointer.secondary_down()) {
            xml_context_menu_wait_for_release = false;
        }

        let xml_selection = egui::TextEdit::load_state(ui.ctx(), text_edit_id)
            .and_then(|state| state.cursor.char_range())
            .filter(|range| !range.is_empty());
        let has_xml_selection = xml_selection.is_some();

        if let (true, Some(menu_pos), Some(menu_layer_id)) = (
            xml_context_menu_open,
            xml_context_menu_pos,
            context_menu_layer_id,
        ) {
            let popup_response = Popup::new(
                xml_context_menu_id,
                ui.ctx().clone(),
                menu_pos,
                menu_layer_id,
            )
            .kind(PopupKind::Menu)
            .layout(egui::Layout::top_down_justified(egui::Align::Min))
            .close_behavior(PopupCloseBehavior::IgnoreClicks)
            .open_bool(&mut xml_context_menu_open)
            .show(|ui| {
                if ui
                    .add_enabled(has_xml_selection, egui::Button::new(t.get("menu_copy")))
                    .clicked()
                {
                    if let Some(range) = xml_selection {
                        ui.ctx()
                            .copy_text(range.slice_str(&self.xml_text).to_owned());
                    }
                    ui.close();
                }
                if ui
                    .add_enabled(has_xml_selection, egui::Button::new(t.get("menu_cut")))
                    .clicked()
                {
                    if let Some(range) = xml_selection {
                        ui.ctx()
                            .copy_text(range.slice_str(&self.xml_text).to_owned());
                        let cursor = self.xml_text.delete_selected(&range);
                        let mut state =
                            egui::TextEdit::load_state(ui.ctx(), text_edit_id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                        state.store(ui.ctx(), text_edit_id);
                        ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
                        xml_dirty = true;
                        ui.ctx().request_repaint();
                    }
                    ui.close();
                }
                if ui.button(t.get("menu_paste")).clicked() {
                    ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    ui.close();
                }
                ui.separator();
                if ui.button(t.get("menu_select_all")).clicked() {
                    let mut state =
                        egui::TextEdit::load_state(ui.ctx(), text_edit_id).unwrap_or_default();
                    let end = egui::text::CCursor::new(self.xml_text.chars().count());
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            end,
                        )));
                    state.store(ui.ctx(), text_edit_id);
                    ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
                    ui.ctx().request_repaint();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(self.can_undo(), egui::Button::new(t.get("menu_undo")))
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(self.can_redo(), egui::Button::new(t.get("menu_redo")))
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.file_type.is_some(),
                        egui::Button::new(t.get("save_editor_parse_xml")),
                    )
                    .clicked()
                {
                    self.parse_current_xml();
                    ui.close();
                }
                ui.separator();
                self.render_view_toggles_menu(ui, t);
            });

            if xml_context_menu_open
                && !xml_context_menu_wait_for_release
                && ui.input(|input| input.pointer.any_pressed())
                && let (Some(pointer_pos), Some(popup_response)) =
                    (ui.ctx().pointer_interact_pos(), popup_response.as_ref())
                && !popup_response.response.rect.contains(pointer_pos)
            {
                xml_context_menu_open = false;
            }
        } else if xml_context_menu_open {
            xml_context_menu_open = false;
        }

        if !xml_context_menu_open {
            xml_context_menu_pos = None;
            xml_context_menu_wait_for_release = false;
        }
        self.xml_context_menu_open = xml_context_menu_open;
        self.xml_context_menu_pos = xml_context_menu_pos;
        self.xml_context_menu_wait_for_release = xml_context_menu_wait_for_release;
        // Track editing snapshot for undo support
        if xml_dirty && self.xml_editing_snapshot.is_none() {
            self.xml_editing_snapshot = Some(xml_before_edit);
        }
        if text_lost_focus {
            // Editing session ended — finalize undo
            if let Some(snap) = self.xml_editing_snapshot.take()
                && snap != self.xml_text
            {
                self.undo_stack.push(snap);
                if self.undo_stack.len() > 100 {
                    self.undo_stack.remove(0);
                }
                self.redo_stack.clear();
            }
        }
        XmlPanelResult { xml_dirty }
    }
}

/// Return the char offset of the start of the given 0-indexed line.
fn char_offset_of_line(text: &str, target_line: usize) -> usize {
    let mut line = 0;
    for (char_idx, c) in text.chars().enumerate() {
        if line == target_line {
            return char_idx;
        }
        if c == '\n' {
            line += 1;
        }
    }
    text.chars().count()
}

/// Build a `LayoutJob` that highlights filter matches in the XML text.
fn highlight_xml(
    ui: &egui::Ui,
    text: &str,
    wrap_width: f32,
    filter: &Filter,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let base_color = ui.visuals().text_color();
    let base_fmt = egui::TextFormat::simple(egui::FontId::monospace(14.0), base_color);

    if matches!(filter, Filter::Empty) {
        job.append(text, 0.0, base_fmt);
        return job;
    }

    let highlight_fmt = egui::TextFormat {
        background: egui::Color32::from_rgba_unmultiplied(255, 255, 0, 60),
        color: egui::Color32::YELLOW,
        ..base_fmt.clone()
    };

    let matches = filter.find_iter(text);
    let mut cursor = 0;

    for (match_start, match_end) in matches {
        if match_start > cursor {
            job.append(&text[cursor..match_start], 0.0, base_fmt.clone());
        }
        job.append(&text[match_start..match_end], 0.0, highlight_fmt.clone());
        cursor = match_end;
    }

    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, base_fmt);
    }

    job
}
