//! Save file editor — left: raw XML text (editable), right: structured editor.
//! Rendered inline as tab content.

mod contraption_preview;

use std::collections::HashSet;

use eframe::egui;

use crate::assets::TextureCache;

use crate::crypto::{self, SaveFileType};
use crate::error::AppError;
use crate::locale::I18n;
use crate::renderer::LevelRenderer;
use crate::save_parser::*;

use contraption_preview::render_contraption_canvas;

/// Compiled filter — either a regex or a plain substring.
pub(super) enum Filter {
    Regex(regex::Regex),
    Plain(String),
    Empty,
}

impl Filter {
    pub(super) fn compile(pattern: &str) -> (Self, bool) {
        if pattern.is_empty() {
            return (Self::Empty, true);
        }
        match regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
        {
            Ok(re) => (Self::Regex(re), true),
            Err(_) => (Self::Plain(pattern.to_lowercase()), false),
        }
    }

    pub(super) fn is_match(&self, text: &str) -> bool {
        match self {
            Self::Empty => true,
            Self::Regex(re) => re.is_match(text),
            Self::Plain(p) => text.to_lowercase().contains(p),
        }
    }

    pub(super) fn find_iter<'a>(&'a self, text: &'a str) -> Vec<(usize, usize)> {
        match self {
            Self::Empty => vec![],
            Self::Regex(re) => re.find_iter(text).map(|m| (m.start(), m.end())).collect(),
            Self::Plain(p) => {
                let lower = text.to_lowercase();
                lower
                    .match_indices(p)
                    .map(|(start, m)| (start, start + m.len()))
                    .collect()
            }
        }
    }
}

/// Per-tab save editor data.
pub struct SaveViewerData {
    /// Detected file type (needed for re-encryption on export).
    pub file_type: Option<SaveFileType>,
    /// File type label (Progress / Contraption / Achievements).
    pub file_type_label: String,
    /// Decrypted XML text (left panel, editable).
    pub xml_text: String,
    /// Parsed structured data (right panel, editable).
    pub data: Option<SaveData>,
    /// Error message if decryption/parsing failed.
    pub error: Option<AppError>,
    /// Filter string for searching entries.
    pub filter: String,
    /// Whether data has been modified since load.
    pub dirty: bool,
    /// Left/right split ratio (0.0–1.0, fraction of width for left panel).
    pub split_ratio: f32,
    /// Show the raw XML panel.
    pub show_xml: bool,
    /// Show the structured view panel.
    pub show_table: bool,
    /// Undo history (XML snapshots).
    pub(super) undo_stack: Vec<String>,
    /// Redo history (XML snapshots).
    pub(super) redo_stack: Vec<String>,
    /// Selected entry indices in the structured view.
    pub selected: HashSet<usize>,
    /// Last clicked row index (for shift-click range select).
    pub(super) last_clicked: Option<usize>,
    /// Snapshot of xml_text captured when the user starts editing in the TextEdit.
    pub(super) xml_editing_snapshot: Option<String>,
    /// Pending scroll target: entry index in structured data → scroll XML to that line.
    pub(super) scroll_to_xml_entry: Option<usize>,
    /// Currently highlighted XML line (0-indexed), set when clicking a structured entry.
    pub(super) highlighted_xml_line: Option<usize>,
    /// Whether the XML context menu is currently open.
    pub(super) xml_context_menu_open: bool,
    /// Fixed screen position for the XML context menu.
    pub(super) xml_context_menu_pos: Option<egui::Pos2>,
    /// Ignore the right-button release that immediately follows opening the XML context menu.
    pub(super) xml_context_menu_wait_for_release: bool,
    /// Show the contraption preview floating window.
    pub show_preview: bool,
    /// Texture cache for atlas textures used in the contraption preview.
    preview_tex_cache: TextureCache,
    /// Resolved level name for .contraption files (e.g. "Level_21 (1-1)").
    pub level_name: Option<String>,
}

/// Resolve the level scene name from a `.contraption` filename using SHA1 reverse lookup.
fn resolve_level_name(file_name: &str) -> Option<String> {
    let stem = file_name
        .strip_suffix(".contraption")
        .or_else(|| file_name.strip_suffix(".CONTRAPTION"))?;
    let (label, scene) = crate::level_db::contraption_level_name(stem)?;
    if label.is_empty() {
        Some(scene.to_string())
    } else {
        Some(format!("{scene} ({label})"))
    }
}

fn localized_file_type_label(file_type: Option<&SaveFileType>, i18n: &I18n) -> String {
    match file_type {
        Some(file_type) => file_type.localized_label(i18n),
        None => i18n.get("save_file_type_unknown"),
    }
}

impl SaveViewerData {
    /// Decrypt and parse a save file, returning the editor data and a status message.
    pub fn load(file_name: &str, raw_data: &[u8], i18n: &I18n) -> (Self, String) {
        let Some(file_type) = SaveFileType::detect(file_name) else {
            let error = AppError::invalid_data_key("error_unknown_file_type");
            let status = error.localized(i18n);
            return (
                Self {
                    file_type: None,
                    file_type_label: localized_file_type_label(None, i18n),
                    xml_text: String::new(),
                    data: None,
                    error: Some(error),
                    filter: String::new(),
                    dirty: false,
                    split_ratio: 0.5,
                    show_xml: true,
                    show_table: true,
                    undo_stack: Vec::new(),
                    redo_stack: Vec::new(),
                    selected: HashSet::new(),
                    last_clicked: None,
                    xml_editing_snapshot: None,
                    scroll_to_xml_entry: None,
                    highlighted_xml_line: None,
                    xml_context_menu_open: false,
                    xml_context_menu_pos: None,
                    xml_context_menu_wait_for_release: false,
                    show_preview: false,
                    preview_tex_cache: TextureCache::new(),
                    level_name: None,
                },
                status,
            );
        };

        let file_type_label = localized_file_type_label(Some(&file_type), i18n);
        let level_name = resolve_level_name(file_name);

        match crypto::decrypt_save_file(&file_type, raw_data) {
            Ok(xml_bytes) => {
                let xml = String::from_utf8_lossy(&xml_bytes);
                let xml_clean = xml
                    .strip_prefix('\u{feff}')
                    .unwrap_or(&xml)
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");
                let (data, parse_error) = match parse_save_data(&file_type, &xml_bytes) {
                    Ok(data) => (Some(data), None),
                    Err(error) => (None, Some(error)),
                };
                let is_contraption = matches!(data, Some(SaveData::Contraption(_)));
                let status = parse_error
                    .as_ref()
                    .map(|error| error.localized(i18n))
                    .unwrap_or_else(|| {
                        i18n.fmt_save_viewer_type_bytes(&file_type_label, xml_clean.len())
                    });
                (
                    Self {
                        file_type: Some(file_type),
                        file_type_label,
                        xml_text: xml_clean,
                        data,
                        error: parse_error,
                        filter: String::new(),
                        dirty: false,
                        split_ratio: 0.5,
                        show_xml: true,
                        show_table: true,
                        undo_stack: Vec::new(),
                        redo_stack: Vec::new(),
                        selected: HashSet::new(),
                        last_clicked: None,
                        xml_editing_snapshot: None,
                        scroll_to_xml_entry: None,
                        highlighted_xml_line: None,
                        xml_context_menu_open: false,
                        xml_context_menu_pos: None,
                        xml_context_menu_wait_for_release: false,
                        show_preview: is_contraption,
                        preview_tex_cache: TextureCache::new(),
                        level_name: level_name.clone(),
                    },
                    status,
                )
            }
            Err(e) => {
                let message = e.localized(i18n);
                (
                    Self {
                        file_type: Some(file_type),
                        file_type_label,
                        xml_text: String::new(),
                        data: None,
                        error: Some(e),
                        filter: String::new(),
                        dirty: false,
                        split_ratio: 0.5,
                        show_xml: true,
                        show_table: true,
                        undo_stack: Vec::new(),
                        redo_stack: Vec::new(),
                        selected: HashSet::new(),
                        last_clicked: None,
                        xml_editing_snapshot: None,
                        scroll_to_xml_entry: None,
                        highlighted_xml_line: None,
                        xml_context_menu_open: false,
                        xml_context_menu_pos: None,
                        xml_context_menu_wait_for_release: false,
                        show_preview: false,
                        preview_tex_cache: TextureCache::new(),
                        level_name,
                    },
                    message,
                )
            }
        }
    }

    /// Load a decrypted XML file directly (no decryption), detecting type from content.
    pub fn load_xml(file_name: &str, xml_bytes: &[u8], i18n: &I18n) -> (Self, String) {
        let xml_raw = String::from_utf8_lossy(xml_bytes);
        let xml_clean = xml_raw
            .strip_prefix('\u{feff}')
            .unwrap_or(&xml_raw)
            .replace("\r\n", "\n")
            .replace('\r', "\n");

        let file_type = crate::save_parser::detect_type_from_xml(&xml_clean);
        let file_type_label = localized_file_type_label(file_type.as_ref(), i18n);

        let (data, parse_error) = match file_type.as_ref() {
            Some(ft) => match parse_save_data(ft, xml_clean.as_bytes()) {
                Ok(data) => (Some(data), None),
                Err(error) => (None, Some(error)),
            },
            None => (None, None),
        };
        let is_contraption = matches!(data, Some(SaveData::Contraption(_)));
        let status = parse_error
            .as_ref()
            .map(|error| error.localized(i18n))
            .unwrap_or_else(|| {
                i18n.fmt_save_viewer_file_type_bytes(file_name, &file_type_label, xml_clean.len())
            });
        (
            Self {
                file_type,
                file_type_label,
                xml_text: xml_clean,
                data,
                error: parse_error,
                filter: String::new(),
                dirty: false,
                split_ratio: 0.5,
                show_xml: true,
                show_table: true,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                selected: HashSet::new(),
                last_clicked: None,
                xml_editing_snapshot: None,
                scroll_to_xml_entry: None,
                highlighted_xml_line: None,
                xml_context_menu_open: false,
                xml_context_menu_pos: None,
                xml_context_menu_wait_for_release: false,
                show_preview: is_contraption,
                preview_tex_cache: TextureCache::new(),
                level_name: resolve_level_name(file_name),
            },
            status,
        )
    }

    /// Encrypt the current XML text back to the save file format.
    pub fn export_encrypted(&self) -> crate::error::AppResult<Option<Vec<u8>>> {
        let Some(ft) = self.file_type.as_ref() else {
            return Ok(None);
        };
        crypto::encrypt_save_file(ft, self.xml_text.as_bytes()).map(Some)
    }

    /// Push current XML text onto the undo stack (call before mutation).
    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.xml_text.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Undo the last change.
    pub fn undo(&mut self) {
        // If mid-edit in the TextEdit, restore the editing snapshot first.
        if let Some(snap) = self.xml_editing_snapshot.take() {
            if snap != self.xml_text {
                self.redo_stack.push(self.xml_text.clone());
                self.xml_text = snap;
                self.reparse();
                self.dirty = true;
                return;
            }
        }
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.xml_text.clone());
            self.xml_text = prev;
            self.reparse();
            self.dirty = true;
        }
    }

    /// Redo the last undone change.
    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.xml_text.clone());
            self.xml_text = next;
            self.reparse();
            self.dirty = true;
        }
    }

    pub fn can_undo(&self) -> bool {
        self.xml_editing_snapshot.is_some() || !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub(super) fn xml_entry_line(&self, entry_idx: usize) -> usize {
        let line_offset = match self.file_type {
            Some(SaveFileType::Contraption) => 3,
            _ => 2,
        };
        entry_idx + line_offset
    }

    pub(super) fn parse_current_xml(&mut self) {
        if let Some(ft) = self.file_type.clone() {
            self.push_undo();
            match parse_save_data(&ft, self.xml_text.as_bytes()) {
                Ok(data) => {
                    self.data = Some(data);
                    self.error = None;
                }
                Err(error) => {
                    self.data = None;
                    self.error = Some(error);
                }
            }
        }
    }

    pub(super) fn render_view_toggles_menu(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        let mut show_structured = self.show_table;
        if ui
            .checkbox(&mut show_structured, t.get("save_viewer_structured"))
            .clicked()
        {
            self.show_table = show_structured;
        }

        if matches!(self.file_type, Some(SaveFileType::Contraption)) {
            let mut show_preview = self.show_preview;
            if ui
                .checkbox(&mut show_preview, t.get("contraption_preview_title"))
                .clicked()
            {
                self.show_preview = show_preview;
            }
        }
    }

    /// Re-parse structured data from current xml_text.
    fn reparse(&mut self) {
        if let Some(ref ft) = self.file_type {
            match parse_save_data(ft, self.xml_text.as_bytes()) {
                Ok(data) => {
                    self.data = Some(data);
                    self.error = None;
                }
                Err(error) => {
                    self.data = None;
                    self.error = Some(error);
                }
            }
        }
        self.selected.clear();
        self.last_clicked = None;
    }

    /// Number of entries in the current data.
    pub fn entry_count(&self) -> usize {
        match &self.data {
            Some(SaveData::Progress(v)) => v.len(),
            Some(SaveData::Contraption(v)) => v.len(),
            Some(SaveData::Achievements(v)) => v.len(),
            None => 0,
        }
    }

    /// Select all entries.
    pub fn select_all(&mut self) {
        let count = self.entry_count();
        self.selected = (0..count).collect();
    }

    /// Deselect all entries.
    pub fn deselect_all(&mut self) {
        self.selected.clear();
        self.last_clicked = None;
    }

    /// Delete selected entries and regenerate XML.
    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let sel = &self.selected;
        if let Some(ref mut data) = self.data {
            match data {
                SaveData::Progress(v) => super::save_tables::remove_indices(v, sel),
                SaveData::Contraption(v) => super::save_tables::remove_indices(v, sel),
                SaveData::Achievements(v) => super::save_tables::remove_indices(v, sel),
            }
        }
        self.selected.clear();
        self.last_clicked = None;
        self.xml_text = self
            .data
            .as_ref()
            .map(serialize_save_data)
            .unwrap_or_default();
        self.dirty = true;
    }

    /// Duplicate selected entries and regenerate XML.
    pub fn duplicate_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        if let Some(ref mut data) = self.data {
            match data {
                SaveData::Progress(v) => super::save_tables::duplicate_indices(v, &self.selected),
                SaveData::Contraption(v) => {
                    super::save_tables::duplicate_indices(v, &self.selected)
                }
                SaveData::Achievements(v) => {
                    super::save_tables::duplicate_indices(v, &self.selected)
                }
            }
        }
        self.selected.clear();
        self.last_clicked = None;
        self.xml_text = self
            .data
            .as_ref()
            .map(serialize_save_data)
            .unwrap_or_default();
        self.dirty = true;
    }

    /// Render the save editor using top-level panels so left/right split works correctly.
    pub fn render_save_panels(&mut self, ui: &mut egui::Ui, t: &'static I18n) {
        // Top: header + toolbar
        egui::Panel::top("save_top_bar").show_inside(ui, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{}: {}",
                    t.get("save_viewer_type"),
                    self.file_type_label
                ));
                if let Some(ref name) = self.level_name {
                    ui.separator();
                    ui.label(name);
                }
                ui.separator();
                ui.label(format!(
                    "{}: {} bytes",
                    t.get("save_viewer_size"),
                    self.xml_text.len()
                ));
                if self.dirty {
                    ui.separator();
                    ui.colored_label(egui::Color32::YELLOW, t.get("save_editor_modified"));
                }
            });

            if let Some(ref err) = self.error {
                ui.colored_label(egui::Color32::RED, err.localized(t));
            }

            ui.separator();

            // Toolbar: Parse XML button + filter
            ui.horizontal(|ui| {
                let can_parse = self.file_type.is_some();
                if ui
                    .add_enabled(can_parse, egui::Button::new(t.get("save_editor_parse_xml")))
                    .clicked()
                {
                    self.parse_current_xml();
                }
                ui.separator();
                ui.label(t.get("save_viewer_filter"));
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text(t.get("save_filter_hint"))
                    .show(ui);
                if !self.filter.is_empty() {
                    let (_, valid) = Filter::compile(&self.filter);
                    if valid {
                        ui.colored_label(egui::Color32::LIGHT_GREEN, "(.*)");
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_RED, t.get("save_editor_regex_err"));
                    }
                }
            });
        });

        let available = ui.available_size();
        let (compiled_filter, _) = Filter::compile(&self.filter);

        let mut xml_dirty = false;
        let mut data_changed = false;

        // Right panel: editable structured view (real Panel so width is correct)
        let default_right = available.x * (1.0 - self.split_ratio);
        let old_last_clicked = self.last_clicked;
        if self.show_table {
            let xml_entry_line_offset = self.xml_entry_line(0);
            let data = &mut self.data;
            let selected = &mut self.selected;
            let last_clicked = &mut self.last_clicked;
            egui::Panel::right("save_table_panel")
                .resizable(true)
                .default_size(default_right)
                .size_range(80.0..=available.x - 80.0)
                .show_inside(ui, |ui| {
                    ui.heading(t.get("save_viewer_structured"));
                    match data {
                        Some(SaveData::Progress(entries)) => {
                            data_changed = super::save_tables::edit_progress(
                                &compiled_filter,
                                ui,
                                entries,
                                selected,
                                last_clicked,
                                &mut self.scroll_to_xml_entry,
                                &mut self.highlighted_xml_line,
                                xml_entry_line_offset,
                                t,
                            );
                        }
                        Some(SaveData::Contraption(parts)) => {
                            data_changed = super::save_tables::edit_contraption(
                                &compiled_filter,
                                ui,
                                parts,
                                selected,
                                last_clicked,
                                &mut self.scroll_to_xml_entry,
                                &mut self.highlighted_xml_line,
                                xml_entry_line_offset,
                                t,
                            );
                        }
                        Some(SaveData::Achievements(entries)) => {
                            data_changed = super::save_tables::edit_achievements(
                                &compiled_filter,
                                ui,
                                entries,
                                selected,
                                last_clicked,
                                &mut self.scroll_to_xml_entry,
                                &mut self.highlighted_xml_line,
                                xml_entry_line_offset,
                                t,
                            );
                        }
                        None => {
                            ui.label(t.get("save_viewer_no_data"));
                        }
                    }
                });
        }
        // When a new row is clicked, queue scroll-to in the XML panel
        if self.last_clicked != old_last_clicked {
            if let Some(entry_idx) = self.last_clicked {
                self.scroll_to_xml_entry = Some(entry_idx);
                self.highlighted_xml_line = Some(self.xml_entry_line(entry_idx));
            }
        }

        // Left panel: editable XML (fills remaining area)
        if self.show_xml {
            let result = self.render_xml_panel(ui, t, &compiled_filter);
            xml_dirty = result.xml_dirty;
        }

        // If structured data changed, regenerate XML text
        if data_changed {
            self.push_undo();
            if let Some(ref d) = self.data {
                self.xml_text = serialize_save_data(d);
            }
            self.dirty = true;
        }
        if xml_dirty {
            self.dirty = true;
        }
    }

    /// Render the contraption preview floating window (if applicable and enabled).
    pub fn render_contraption_preview(
        &mut self,
        ctx: &egui::Context,
        t: &'static I18n,
        renderer: &mut LevelRenderer,
    ) {
        if !self.show_preview {
            return;
        }
        let parts = match &self.data {
            Some(SaveData::Contraption(p)) => p,
            _ => return,
        };
        if parts.is_empty() {
            return;
        }

        let mut open = self.show_preview;
        egui::Window::new(t.get("contraption_preview_title"))
            .open(&mut open)
            .default_size([520.0, 560.0])
            .min_size([460.0, 500.0])
            .resizable(true)
            .show(ctx, |ui| {
                render_contraption_canvas(ui, parts, &mut self.preview_tex_cache, renderer);
            });
        self.show_preview = open;
    }
}

