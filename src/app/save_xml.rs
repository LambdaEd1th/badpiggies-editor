//! XML text panel — editable XML with line numbers, syntax highlighting, and scroll-to-line.

use eframe::egui;

use crate::locale::I18n;

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
        let xml_text = &mut self.xml_text;
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
                let line_count = xml_text.chars().filter(|&c| c == '\n').count() + 1;
                let gutter_digits = ((line_count as f32).log10().floor() as usize + 1).max(2);
                let gutter_char_width = ui.ctx().fonts_mut(|f| {
                    f.glyph_width(&egui::TextStyle::Monospace.resolve(ui.style()), '0')
                });
                let gutter_width = gutter_char_width * (gutter_digits as f32 + 1.0) + 8.0;

                ui.horizontal_top(|ui| {
                    let output = egui::TextEdit::multiline(xml_text)
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

                    // Scroll to target entry line
                    if let Some(entry_idx) = scroll_target {
                        // Entry i maps to XML line i+2 (0-indexed):
                        // line 0 = <?xml ...>, line 1 = <root>, line 2+ = entries
                        let target_line = entry_idx + 2;
                        let char_offset = char_offset_of_line(xml_text.as_str(), target_line);
                        let ccursor = egui::text::CCursor::new(char_offset);
                        let mut state = output.state.clone();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), output.response.id);

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
        // Track editing snapshot for undo support
        if xml_dirty {
            if self.xml_editing_snapshot.is_none() {
                self.xml_editing_snapshot = Some(xml_before_edit);
            }
        }
        if text_lost_focus {
            // Editing session ended — finalize undo
            if let Some(snap) = self.xml_editing_snapshot.take() {
                if snap != self.xml_text {
                    self.undo_stack.push(snap);
                    if self.undo_stack.len() > 100 {
                        self.undo_stack.remove(0);
                    }
                    self.redo_stack.clear();
                }
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
