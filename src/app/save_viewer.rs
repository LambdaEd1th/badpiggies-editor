//! Save file editor — left: raw XML text (editable), right: structured editor.
//! Rendered inline as tab content.

use std::collections::HashSet;

use eframe::egui;

use crate::assets::TextureCache;

use crate::crypto::{self, SaveFileType};
use crate::locale::I18n;
use crate::renderer::{sprite_shader, LevelRenderer};
use crate::save_parser::*;
use crate::sprite_db::UvRect;

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
    pub error: Option<String>,
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

impl SaveViewerData {
    /// Decrypt and parse a save file, returning the editor data and a status message.
    pub fn load(file_name: &str, raw_data: &[u8]) -> (Self, String) {
        let Some(file_type) = SaveFileType::detect(file_name) else {
            return (
                Self {
                    file_type: None,
                    file_type_label: "Unknown".into(),
                    xml_text: String::new(),
                    data: None,
                    error: Some("Unknown file type".into()),
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
                "Unknown file type".into(),
            );
        };

        let file_type_label = file_type.label().to_string();
        let level_name = resolve_level_name(file_name);

        match crypto::decrypt_save_file(&file_type, raw_data) {
            Ok(xml_bytes) => {
                let xml = String::from_utf8_lossy(&xml_bytes);
                let xml_clean = xml
                    .strip_prefix('\u{feff}')
                    .unwrap_or(&xml)
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");
                let data = parse_save_data(&file_type, &xml_bytes).ok();
                let is_contraption = matches!(data, Some(SaveData::Contraption(_)));
                let status = format!("{file_type_label}: {} bytes", xml_clean.len());
                (
                    Self {
                        file_type: Some(file_type),
                        file_type_label,
                        xml_text: xml_clean,
                        data,
                        error: None,
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
            Err(e) => (
                Self {
                    file_type: Some(file_type),
                    file_type_label,
                    xml_text: String::new(),
                    data: None,
                    error: Some(e.clone()),
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
                e,
            ),
        }
    }

    /// Load a decrypted XML file directly (no decryption), detecting type from content.
    pub fn load_xml(file_name: &str, xml_bytes: &[u8]) -> (Self, String) {
        let xml_raw = String::from_utf8_lossy(xml_bytes);
        let xml_clean = xml_raw
            .strip_prefix('\u{feff}')
            .unwrap_or(&xml_raw)
            .replace("\r\n", "\n")
            .replace('\r', "\n");

        let file_type = crate::save_parser::detect_type_from_xml(&xml_clean);
        let file_type_label = file_type
            .as_ref()
            .map(|ft| ft.label().to_string())
            .unwrap_or_else(|| "Unknown".into());

        let data = file_type
            .as_ref()
            .and_then(|ft| parse_save_data(ft, xml_clean.as_bytes()).ok());

        let is_contraption = matches!(data, Some(SaveData::Contraption(_)));
        let status = format!("{file_name}: {file_type_label}, {} bytes", xml_clean.len());
        (
            Self {
                file_type,
                file_type_label,
                xml_text: xml_clean,
                data,
                error: None,
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
    pub fn export_encrypted(&self) -> Option<Vec<u8>> {
        let ft = self.file_type.as_ref()?;
        Some(crypto::encrypt_save_file(ft, self.xml_text.as_bytes()))
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
            self.data = parse_save_data(ft, self.xml_text.as_bytes()).ok();
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
                ui.colored_label(egui::Color32::RED, err);
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

/// Part type integer to display name.
fn part_type_name(part_type: i32) -> &'static str {
    match part_type {
        0 => "Unknown",
        1 => "Balloon",
        2 => "Balloons2",
        3 => "Balloons3",
        4 => "Fan",
        5 => "WoodenFrame",
        6 => "Bellows",
        7 => "CartWheel",
        8 => "Basket",
        9 => "Sandbag",
        10 => "Pig",
        11 => "Sandbag2",
        12 => "Sandbag3",
        13 => "Propeller",
        14 => "Wings",
        15 => "Tailplane",
        16 => "Engine",
        17 => "Rocket",
        18 => "MetalFrame",
        19 => "SmallWheel",
        20 => "MetalWing",
        21 => "MetalTail",
        22 => "Rotor",
        23 => "MotorWheel",
        24 => "TNT",
        25 => "EngineSmall",
        26 => "EngineBig",
        27 => "NormalWheel",
        28 => "Spring",
        29 => "Umbrella",
        30 => "Rope",
        31 => "CokeBottle",
        32 => "KingPig",
        33 => "RedRocket",
        34 => "SodaBottle",
        35 => "PoweredUmbrella",
        36 => "Egg",
        37 => "JetEngine",
        38 => "ObsoleteWheel",
        39 => "SpringBoxingGlove",
        40 => "StickyWheel",
        41 => "GrapplingHook",
        42 => "Pumpkin",
        43 => "Kicker",
        44 => "Gearbox",
        45 => "GoldenPig",
        46 => "PointLight",
        47 => "SpotLight",
        48 => "TimeBomb",
        _ => "?",
    }
}

/// Unity ChangeVisualConnections: determine if a layer should be visible based
/// on the part type, grid rotation, and neighboring parts.
///
/// Part types with overrides:
///   14 Wings          – TopFrameSprite / BottomFrameSprite (no rotation)
///   17 Rocket         – 4 cardinal attachments + bottom fallback (rotation-aware)
///   24 TNT            – 4 cardinal attachments, simple (rotation-aware)
///   39 SpringBoxingGlove – always hide SpringVisualization
///   41 GrapplingHook   – 8-directional attachments + hide SpringVisualization
///   47 SpotLight       – 8-directional attachments
fn layer_visible(
    part: &ContraptionPart,
    layer: &crate::icon_db::IconLayer,
    _occupied: &std::collections::HashSet<(i32, i32)>,
    rotated_has: &dyn Fn(i32, i32, i32, i32) -> bool,
    has_neighbor: &dyn Fn(i32, i32, i32, i32) -> bool,
) -> bool {
    let name = layer.go_name.as_str();
    let (x, y, rot) = (part.x, part.y, part.rot);

    match part.part_type {
        // --- Wings (14): no rotation applied to directions ---
        14 => {
            let has_up = has_neighbor(x, y, 0, 1);
            let has_down = has_neighbor(x, y, 0, -1);
            let has_left = has_neighbor(x, y, -1, 0);
            let has_right = has_neighbor(x, y, 1, 0);
            match name {
                "TopFrameSprite" => has_up || has_left || has_right,
                "BottomFrameSprite" => {
                    let top_connected = has_up || has_left || has_right;
                    let bot_connected = has_down || has_left || has_right;
                    bot_connected || !top_connected
                }
                _ => true,
            }
        }
        // --- Rocket (17): 4 cardinal, rotation-aware, bottom fallback ---
        17 => {
            // Direction enum: Right=0, Up=1, Left=2, Down=3
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            match name {
                "LeftAttachment" => has_left,
                "RightAttachment" => has_right,
                "TopAttachment" => has_up,
                "BottomAttachment" => has_down || (!has_up && !has_left && !has_right),
                _ => true,
            }
        }
        // --- TNT (24): 4 cardinal, rotation-aware, simple ---
        24 => {
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            match name {
                "LeftAttachment" => has_left,
                "RightAttachment" => has_right,
                "TopAttachment" => has_up,
                "BottomAttachment" => has_down,
                _ => true,
            }
        }
        // --- SpringBoxingGlove (39): always hide spring ---
        39 => name != "SpringVisualization",
        // --- GrapplingHook (41) & SpotLight (47): 8-directional ---
        41 | 47 => {
            // SpringVisualization always hidden for GrapplingHook
            if part.part_type == 41 && name == "SpringVisualization" {
                return false;
            }
            // Cardinal neighbor flags (rotation-aware)
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            // Diagonal neighbor flags: Rotate(DiagDir, rot) % 4 maps to cardinal for CanConnectTo
            // UpRight=4, UpLeft=5, DownLeft=6, DownRight=7
            let has_up_left = rotated_has(x, y, 5, rot);
            let has_down_left = rotated_has(x, y, 6, rot);
            let has_up_right = rotated_has(x, y, 4, rot);
            let has_down_right = rotated_has(x, y, 7, rot);
            // Is this a diagonal rotation? GridRotation: Deg_45=4, Deg_135=5, Deg_225=6, Deg_315=7
            let diag = rot >= 4 && rot <= 7;
            let visible = match name {
                "LeftAttachment" => has_left && !diag,
                "RightAttachment" => has_right && !diag,
                "TopAttachment" => has_up && !diag,
                "BottomAttachment" => (has_down && !diag) || (!has_up && !has_left && !has_right && !diag),
                "BottomLeftAttachment" => has_down_left && diag,
                "BottomRightAttachment" => has_down_right && diag,
                "TopLeftAttachment" => has_up_left && diag,
                "TopRightAttachment" => has_up_right && diag,
                _ => return true,
            };
            // Global fallback: if absolutely no connections, show bottom
            if !visible && name == "BottomAttachment" {
                let any = has_up || has_down_left || has_left || has_right
                    || has_up_left || has_up_right || has_down_right;
                if !any {
                    return true;
                }
            }
            visible
        }
        _ => true,
    }
}

/// Render a grid-based contraption preview using part icons from the sprite atlas.
fn render_contraption_canvas(
    ui: &mut egui::Ui,
    parts: &[ContraptionPart],
    tex_cache: &mut TextureCache,
    renderer: &mut LevelRenderer,
) {
    use crate::icon_db;

    let cell_size = 48.0_f32; // pixels per grid cell

    // Compute grid bounds
    let min_x = parts.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = parts.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = parts.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = parts.iter().map(|p| p.y).max().unwrap_or(0);
    let grid_w = (max_x - min_x + 1) as f32;
    let grid_h = (max_y - min_y + 1) as f32;
    let canvas_w = grid_w * cell_size;
    let canvas_h = grid_h * cell_size;

    // Center the grid in the available space
    let avail = ui.available_size();
    let scale = (avail.x / canvas_w).min(avail.y / canvas_h).min(1.5);
    let scaled_cell = cell_size * scale;
    let total_w = grid_w * scaled_cell;
    let total_h = grid_h * scaled_cell;

    let (response, painter) = ui.allocate_painter(
        egui::vec2(total_w.max(avail.x), total_h.max(avail.y)),
        egui::Sense::hover(),
    );
    let origin = egui::pos2(
        response.rect.center().x - total_w * 0.5,
        response.rect.center().y - total_h * 0.5,
    );

    let gpu_resources = renderer.preview_sprite_resources();
    let mut gpu_draws: Vec<sprite_shader::SpriteBatchDraw> = Vec::new();

    let draw_line_grid = || {
        let grid_line_color = ui
            .visuals()
            .widgets
            .noninteractive
            .bg_stroke
            .color
            .linear_multiply(0.3);
        for gx in 0..=(grid_w as i32) {
            let x = origin.x + gx as f32 * scaled_cell;
            painter.line_segment(
                [egui::pos2(x, origin.y), egui::pos2(x, origin.y + total_h)],
                egui::Stroke::new(1.0, grid_line_color),
            );
        }
        for gy in 0..=(grid_h as i32) {
            let y = origin.y + gy as f32 * scaled_cell;
            painter.line_segment(
                [egui::pos2(origin.x, y), egui::pos2(origin.x + total_w, y)],
                egui::Stroke::new(1.0, grid_line_color),
            );
        }
    };

    // Reuse the same GridCellLight sprite the level editor uses for the
    // construction grid, instead of drawing a synthetic line grid.
    if let Some(sprite) = crate::sprite_db::get_sprite_info("GridCellLight") {
        // Unity confirmation:
        // GridCellLight prefab localScale = 0.3
        // Sprite.CreateMesh uses half-extents = width * 10 / 768, so final
        // full world size is width * 20 / 768 * localScale.
        // From sprites.bytes: GridCellLight width=104, height=105.
        let grid_sprite_w = scaled_cell * (104.0 * 0.3 * 20.0 / 768.0);
        let grid_sprite_h = scaled_cell * (105.0 * 0.3 * 20.0 / 768.0);
        if gpu_resources.is_some()
            && let Some(atlas) = renderer.preview_sprite_atlas(&sprite.atlas)
        {
            let (uv_min, uv_max) = sprite_shader::compute_uvs(
                &sprite.uv,
                atlas.width as f32,
                atlas.height as f32,
                false,
                false,
            );

            for gx in 0..(grid_w as i32) {
                for gy in 0..(grid_h as i32) {
                    let center = egui::pos2(
                        origin.x + (gx as f32 + 0.5) * scaled_cell,
                        origin.y + (gy as f32 + 0.5) * scaled_cell,
                    );
                    gpu_draws.push(sprite_shader::SpriteBatchDraw {
                        atlas: atlas.clone(),
                        slot: gpu_draws.len() as u32,
                        uniforms: sprite_shader::SpriteUniforms {
                            screen_size: [response.rect.width(), response.rect.height()],
                            camera_center: [0.0, 0.0],
                            zoom: 1.0,
                            rotation: 0.0,
                            world_center: [
                                center.x - response.rect.center().x,
                                response.rect.center().y - center.y,
                            ],
                            half_size: [grid_sprite_w * 0.5, grid_sprite_h * 0.5],
                            uv_min,
                            uv_max,
                            mode: 0.0,
                            shine_center: 0.0,
                            tint_color: [1.0, 1.0, 1.0, 1.0],
                        },
                    });
                }
            }
        } else {
            let atlas_path = format!("sprites/{}", sprite.atlas);
            let tex_id = tex_cache.load_sprite_crop(
                ui.ctx(),
                "save_viewer_grid_cell_light_raw",
                &atlas_path,
                [sprite.uv.x, sprite.uv.y, sprite.uv.w, sprite.uv.h],
            );
            if let Some(tex_id) = tex_id {
                let uv_rect =
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                let tint = egui::Color32::WHITE;
                for gx in 0..(grid_w as i32) {
                    for gy in 0..(grid_h as i32) {
                        let center = egui::pos2(
                            origin.x + (gx as f32 + 0.5) * scaled_cell,
                            origin.y + (gy as f32 + 0.5) * scaled_cell,
                        );
                        let cell_rect = egui::Rect::from_center_size(
                            center,
                            egui::vec2(grid_sprite_w, grid_sprite_h),
                        );
                        let mut mesh = egui::Mesh::with_texture(tex_id);
                        mesh.add_rect_with_uv(cell_rect, uv_rect, tint);
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
            } else {
                draw_line_grid();
            }
        }
    } else {
        draw_line_grid();
    }

    // Sort parts by Z depth for correct draw order (back-to-front).
    // Unity formula: Z = -0.1 + m_ZOffset - (x + 2*y) / 100000
    // Camera looks along -Z, so more positive world Z = farther = drawn first.
    // Within each part, child layers have z_local offsets that add to the part Z.
    // Unity sorts ALL mesh renderers globally (interleaved across parts), not
    // per-part.  We replicate this by collecting every layer from every part,
    // computing its world Z, sorting descending (farthest first), and drawing
    // in that order.

    // Build a grid occupancy set for neighbor checking (ChangeVisualConnections).
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for p in parts {
        occupied.insert((p.x, p.y));
    }
    // Unity: Rotate(dir, rot) = (dir + rot) % 4  →  Direction enum order: Right=0 Up=1 Left=2 Down=3
    // Maps rotated direction to grid (dx,dy).
    const DIR_OFFSETS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let rotated_has = |x: i32, y: i32, dir: i32, rot: i32| -> bool {
        let (dx, dy) = DIR_OFFSETS[((dir + rot) % 4) as usize];
        occupied.contains(&(x + dx, y + dy))
    };
    let has_neighbor =
        |x: i32, y: i32, dx: i32, dy: i32| -> bool { occupied.contains(&(x + dx, y + dy)) };

    struct DrawLayer<'a> {
        part: &'a ContraptionPart,
        layer: &'a icon_db::IconLayer,
        world_z: f32,
    }

    let mut draw_layers: Vec<DrawLayer> = Vec::new();

    for part in parts {
        let part_info = icon_db::get_part_info(part.part_type, part.custom_part_index);
        if let Some(info) = part_info {
            let part_z = -0.1_f32 + info.z_offset
                - (part.x as f32 + 2.0 * part.y as f32) / 100000.0;
            for layer in &info.layers {
                if !layer_visible(part, layer, &occupied, &rotated_has, &has_neighbor) {
                    continue;
                }

                let world_z = part_z + layer.z_local;
                draw_layers.push(DrawLayer {
                    part,
                    layer,
                    world_z,
                });
            }
        }
    }

    // Sort descending by world Z: farthest (most positive) drawn first (behind),
    // nearest (most negative) drawn last (on top).
    draw_layers.sort_by(|a, b| {
        b.world_z
            .partial_cmp(&a.world_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Draw each layer
    for dl in &draw_layers {
        let part = dl.part;
        let layer = dl.layer;
        let gx = (part.x - min_x) as f32;
        let gy = (max_y - part.y) as f32; // flip Y
        let cell_center = egui::pos2(
            origin.x + (gx + 0.5) * scaled_cell,
            origin.y + (gy + 0.5) * scaled_cell,
        );

        let fixed_scale = scaled_cell;

        // v0..v3 order from TOML: BL, TL, TR, BR
        let mut verts = [
            (layer.v0_x, layer.v0_y),
            (layer.v1_x, layer.v1_y),
            (layer.v2_x, layer.v2_y),
            (layer.v3_x, layer.v3_y),
        ];

        // Apply part-level flip/rotation.
        // Unity uses if/else: flipped parts don't get grid rotation applied.
        for (x, y) in &mut verts {
            let mut px = *x;
            let py = *y;

            if part.flipped {
                // 180° Y-axis rotation: negate X (Z negation handled separately)
                px = -px;
            } else {
                let rot = part.rot % 4;
                let (rx, ry) = match rot {
                    1 => (-py, px),
                    2 => (-px, -py),
                    3 => (py, -px),
                    _ => (px, py),
                };
                px = rx;
                *y = ry;
            }

            *x = px;
        }

        let to_screen = |vx: f32, vy: f32| {
            egui::pos2(
                cell_center.x + vx * fixed_scale,
                cell_center.y - vy * fixed_scale, // world Y-up -> screen Y-down
            )
        };

        let screen_positions = [
            to_screen(verts[0].0, verts[0].1),
            to_screen(verts[1].0, verts[1].1),
            to_screen(verts[2].0, verts[2].1),
            to_screen(verts[3].0, verts[3].1),
        ];

        // Match the main sprite_shader path: Unity V-flip + half-texel inset to
        // avoid atlas bleeding. Geometry flip/rotation is already applied to
        // vertices above, so UV flip flags stay false here.
        let uv_rect = UvRect {
            x: layer.uv_x,
            y: layer.uv_y,
            w: layer.uv_w,
            h: layer.uv_h,
        };

        if gpu_resources.is_some()
            && let Some(atlas) = renderer.preview_sprite_atlas(&layer.atlas)
        {
            let to_shader_space = |p: egui::Pos2| -> [f32; 2] {
                [
                    p.x - response.rect.center().x,
                    response.rect.center().y - p.y,
                ]
            };
            let bl = to_shader_space(screen_positions[0]);
            let tl = to_shader_space(screen_positions[1]);
            let tr = to_shader_space(screen_positions[2]);
            let br = to_shader_space(screen_positions[3]);

            let center = [
                (bl[0] + tl[0] + tr[0] + br[0]) * 0.25,
                (bl[1] + tl[1] + tr[1] + br[1]) * 0.25,
            ];
            let mut x_axis = [br[0] - bl[0], br[1] - bl[1]];
            let y_axis = [tl[0] - bl[0], tl[1] - bl[1]];

            let mut flip_x = false;
            let det = x_axis[0] * y_axis[1] - x_axis[1] * y_axis[0];
            if det < 0.0 {
                x_axis = [-x_axis[0], -x_axis[1]];
                flip_x = true;
            }

            let half_w = x_axis[0].hypot(x_axis[1]) * 0.5;
            let half_h = y_axis[0].hypot(y_axis[1]) * 0.5;
            if half_w > 0.0 && half_h > 0.0 {
                let rotation = x_axis[1].atan2(x_axis[0]);
                let (uv_min, uv_max) = sprite_shader::compute_uvs(
                    &uv_rect,
                    atlas.width as f32,
                    atlas.height as f32,
                    flip_x,
                    false,
                );
                gpu_draws.push(sprite_shader::SpriteBatchDraw {
                    atlas,
                    slot: gpu_draws.len() as u32,
                    uniforms: sprite_shader::SpriteUniforms {
                        screen_size: [response.rect.width(), response.rect.height()],
                        camera_center: [0.0, 0.0],
                        zoom: 1.0,
                        rotation,
                        world_center: center,
                        half_size: [half_w, half_h],
                        uv_min,
                        uv_max,
                        mode: 0.0,
                        shine_center: 0.0,
                        tint_color: [1.0, 1.0, 1.0, 1.0],
                    },
                });
                continue;
            }
        }

        let atlas_path = format!("sprites/{}", layer.atlas);
        let tex_id = match tex_cache.load_texture(ui.ctx(), &atlas_path, &layer.atlas) {
            Some(id) => id,
            None => continue,
        };
        let [atlas_w, atlas_h] = match tex_cache.texture_size(&layer.atlas) {
            Some(size) => size,
            None => continue,
        };
        let (uv_min, uv_max) = sprite_shader::compute_uvs(
            &uv_rect,
            atlas_w as f32,
            atlas_h as f32,
            false,
            false,
        );
        let uv_bl = egui::pos2(uv_min[0], uv_max[1]);
        let uv_tl = egui::pos2(uv_min[0], uv_min[1]);
        let uv_tr = egui::pos2(uv_max[0], uv_min[1]);
        let uv_br = egui::pos2(uv_max[0], uv_max[1]);

        let mesh = part_icon_mesh_quad(tex_id, screen_positions, [uv_bl, uv_tl, uv_tr, uv_br]);
        painter.add(egui::Shape::mesh(mesh));
    }

    if let Some(resources) = gpu_resources
        && !gpu_draws.is_empty()
    {
        painter.add(sprite_shader::make_sprite_batch_callback(
            response.rect,
            resources,
            gpu_draws,
        ));
    }

    // Tooltips (based on grid cell hover)
    if let Some(pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
        let mut hovered_parts: Vec<String> = Vec::new();
        for part in parts {
            let gx = (part.x - min_x) as f32;
            let gy = (max_y - part.y) as f32;
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + gx * scaled_cell, origin.y + gy * scaled_cell),
                egui::vec2(scaled_cell, scaled_cell),
            );
            if cell_rect.contains(pos) {
                let name = part_type_name(part.part_type);
                hovered_parts.push(format!(
                    "{name} ({}, {})  rot={} flipped={}",
                    part.x, part.y, part.rot, part.flipped
                ));
            }
        }

        if !hovered_parts.is_empty() {
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                ui.id().with("part_tip"),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                for line in &hovered_parts {
                    ui.label(line);
                }
            });
        }
    }
}

/// Build a textured quad mesh with arbitrary vertex positions and UV corners.
/// Vertex order must be BL, TL, TR, BR to match Unity mesh order.
fn part_icon_mesh_quad(
    tex_id: egui::TextureId,
    positions: [egui::Pos2; 4],
    uvs: [egui::Pos2; 4],
) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(tex_id);
    let white = egui::Color32::WHITE;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[0],
        uv: uvs[0],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[1],
        uv: uvs[1],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[2],
        uv: uvs[2],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[3],
        uv: uvs[3],
        color: white,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    mesh
}

