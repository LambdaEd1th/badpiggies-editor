use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::i18n::locale::Language;
use badpiggies_editor_core::domain::level_warning::{LevelWarning, collect_level_warnings};
use badpiggies_editor_core::domain::prefab_override::{
    OverrideNode, find_first_node_mut, parse_override_text, serialize_override_tree,
};
use badpiggies_editor_core::domain::types::*;
use badpiggies_editor_core::io::crypto::SaveFileType;
#[cfg(test)]
use badpiggies_editor_core::io::save::parser::parse_save_data;
use badpiggies_editor_core::io::save::parser::{
    AchievementEntry, ContraptionPart, ProgressEntry, SaveData, serialize_save_data,
};
use badpiggies_editor_core::io::unity3d::Unity3dTextAssetEntry;

const UNDO_LIMIT: usize = 100;

#[derive(Clone)]
struct Snapshot {
    level: LevelData,
    selected: BTreeSet<ObjectIndex>,
}

#[derive(Clone, Default)]
struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

#[derive(Clone)]
struct SaveSnapshot {
    xml: String,
    data: Option<SaveData>,
    parse_error: Option<String>,
    selected: BTreeSet<usize>,
}

#[derive(Clone, Default)]
struct SaveHistory {
    undo: Vec<SaveSnapshot>,
    redo: Vec<SaveSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SaveViewMode {
    #[default]
    Table,
    Xml,
    Split,
    Preview,
}

#[derive(Clone)]
pub struct SaveDocument {
    pub xml: String,
    pub file_type: SaveFileType,
    pub data: Option<SaveData>,
    pub selected: BTreeSet<usize>,
    pub view: SaveViewMode,
    pub parse_error: Option<String>,
    pub filter: String,
    history: SaveHistory,
}

impl SaveDocument {
    pub fn len(&self) -> usize {
        match self.data.as_ref() {
            Some(SaveData::Progress(entries)) => entries.len(),
            Some(SaveData::Contraption(parts)) => parts.len(),
            Some(SaveData::Achievements(entries)) => entries.len(),
            None => 0,
        }
    }

    fn sync_xml(&mut self) {
        if let Some(data) = self.data.as_ref() {
            self.xml = serialize_save_data(data);
            self.parse_error = None;
        }
    }

    fn snapshot(&self) -> SaveSnapshot {
        SaveSnapshot {
            xml: self.xml.clone(),
            data: self.data.clone(),
            parse_error: self.parse_error.clone(),
            selected: self.selected.clone(),
        }
    }

    fn restore(&mut self, snapshot: SaveSnapshot) {
        self.xml = snapshot.xml;
        self.data = snapshot.data;
        self.parse_error = snapshot.parse_error;
        self.selected = snapshot.selected;
    }

    fn push_undo(&mut self) {
        self.history.undo.push(self.snapshot());
        if self.history.undo.len() > UNDO_LIMIT {
            self.history.undo.remove(0);
        }
        self.history.redo.clear();
    }
}

#[derive(Clone)]
pub struct Tab {
    pub level: Option<LevelData>,
    pub save: Option<SaveDocument>,
    pub file_name: String,
    pub selected: BTreeSet<ObjectIndex>,
    pub status: String,
    pub dirty: bool,
    pub revision: u64,
    workspace_placeholder: bool,
    history: History,
}

impl Tab {
    fn empty(index: usize) -> Self {
        Self {
            level: Some(LevelData::default()),
            save: None,
            file_name: format!("Untitled {index}"),
            selected: BTreeSet::new(),
            status: String::new(),
            dirty: false,
            revision: 1,
            workspace_placeholder: false,
            history: History::default(),
        }
    }

    fn workspace_placeholder() -> Self {
        Self {
            workspace_placeholder: true,
            file_name: String::new(),
            ..Self::empty(1)
        }
    }

    pub fn is_level(&self) -> bool {
        self.level.is_some()
    }

    pub fn is_workspace_placeholder(&self) -> bool {
        self.workspace_placeholder
    }

    pub fn is_empty_untitled(&self) -> bool {
        !self.workspace_placeholder
            && !self.dirty
            && self.save.is_none()
            && self.file_name.starts_with("Untitled")
            && self
                .level
                .as_ref()
                .is_some_and(|level| level.objects.is_empty())
    }

    pub fn title(&self) -> String {
        if self.dirty {
            format!("{} *", self.file_name)
        } else {
            self.file_name.clone()
        }
    }

    pub fn status_bar_file_label(&self) -> Option<String> {
        if self.is_workspace_placeholder() {
            return None;
        }
        let level_name = if self.save.is_some() {
            badpiggies_editor_core::data::level_db::contraption_display_name_for_filename(
                &self.file_name,
            )
        } else {
            badpiggies_editor_core::data::level_db::level_display_name_for_filename(&self.file_name)
        };
        Some(match level_name {
            Some(level_name) => format!("{} -> {level_name}", self.file_name),
            None => self.file_name.clone(),
        })
    }
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct CameraState {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasPointerState {
    pub world: Option<Vec2>,
}

#[derive(Clone)]
pub struct Clipboard {
    subtrees: Vec<Vec<LevelObject>>,
}

pub struct EditorState {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub language: Language,
    pub theme: ThemePreference,
    pub show_tree: bool,
    pub show_properties: bool,
    pub show_grid: bool,
    pub show_background: bool,
    pub show_construction_grid: bool,
    pub show_dark_overlay: bool,
    pub show_ground: bool,
    pub show_terrain_triangles: bool,
    pub show_preview_route: bool,
    pub show_tools: bool,
    pub show_preview_controls: bool,
    pub cursor_mode: CursorModeState,
    pub preview_state: PreviewState,
    pub night_vision: bool,
    pub terrain_draw_mode: TerrainDrawModeState,
    pub terrain_preset: Option<TerrainPresetState>,
    pub terrain_curve_segments: usize,
    pub terrain_texture_index: usize,
    pub terrain_has_collider: bool,
    pub menu_open: Option<&'static str>,
    pub mobile_panel: Option<MobilePanel>,
    pub modal: Option<Modal>,
    pub camera: CameraState,
    pub clipboard: Option<Clipboard>,
    pub camera_command: u64,
    pub add_object: AddObjectDraft,
    pub pending_delete: Vec<ObjectIndex>,
    pub pending_preview_state: Option<PreviewState>,
    pub unity_bundle: Option<UnityBundleDocument>,
    pub tree_dragging: Option<ObjectIndex>,
    pub tab_dragging: Option<usize>,
    pub pending_close: Option<usize>,
    selection_anchor: Option<ObjectIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnityBundleMode {
    ExtractLevels,
    ReplaceLevel,
}

pub struct UnityBundleDocument {
    pub name: String,
    pub bytes: Vec<u8>,
    pub entries: Vec<Unity3dTextAssetEntry>,
    pub selected: BTreeSet<usize>,
    pub mode: UnityBundleMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorModeState {
    #[default]
    Select,
    BoxSelect,
    DrawTerrain,
    Pan,
}

impl CursorModeState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::BoxSelect => "box_select",
            Self::DrawTerrain => "draw_terrain",
            Self::Pan => "pan",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewState {
    #[default]
    Build,
    Play,
    Pause,
}

impl PreviewState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Play => "play",
            Self::Pause => "pause",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainDrawModeState {
    #[default]
    Free,
    Curve,
    CircularArc,
    Horizontal,
    Vertical,
}

impl TerrainDrawModeState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Curve => "curve",
            Self::CircularArc => "circular_arc",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainPresetState {
    Circle,
    PerfectCircle,
    Rectangle,
    Square,
    EquilateralTriangle,
}

impl TerrainPresetState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::PerfectCircle => "perfect_circle",
            Self::Rectangle => "rectangle",
            Self::Square => "square",
            Self::EquilateralTriangle => "equilateral_triangle",
        }
    }
}

#[derive(Clone)]
pub struct AddObjectDraft {
    pub name: String,
    pub prefab_index: i16,
    pub data_type: DataType,
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
    pub terrain_has_collider: bool,
}

impl Default for AddObjectDraft {
    fn default() -> Self {
        Self {
            name: "NewObject".to_string(),
            prefab_index: 0,
            data_type: DataType::None,
            position: Vec3::default(),
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            terrain_has_collider: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const fn shell_class(self) -> &'static str {
        match self {
            Self::System => "app-shell system-theme",
            Self::Light => "app-shell light-theme",
            Self::Dark => "app-shell dark-theme",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MobilePanel {
    Objects,
    Properties,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Shortcuts,
    AddObject,
    Settings,
    DeleteConfirm,
    LevelWarnings,
    Unity3d,
    Logs,
    CloseConfirm,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            tabs: vec![Tab::workspace_placeholder()],
            active_tab: 0,
            language: Language::from_system(),
            theme: crate::platform::read_theme_preference(),
            show_tree: true,
            show_properties: true,
            show_grid: true,
            show_background: true,
            show_construction_grid: true,
            show_dark_overlay: true,
            show_ground: false,
            show_terrain_triangles: false,
            show_preview_route: false,
            show_tools: true,
            show_preview_controls: true,
            cursor_mode: CursorModeState::Select,
            preview_state: PreviewState::Build,
            night_vision: true,
            terrain_draw_mode: TerrainDrawModeState::Free,
            terrain_preset: None,
            terrain_curve_segments: 24,
            terrain_texture_index: 1,
            terrain_has_collider: true,
            menu_open: None,
            mobile_panel: None,
            modal: None,
            camera: CameraState {
                x: 0.0,
                y: 0.0,
                zoom: 40.0,
            },
            clipboard: None,
            camera_command: 0,
            add_object: AddObjectDraft::default(),
            pending_delete: Vec::new(),
            pending_preview_state: None,
            unity_bundle: None,
            tree_dragging: None,
            tab_dragging: None,
            pending_close: None,
            selection_anchor: None,
        }
    }
}

impl EditorState {
    pub fn active(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn active_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    pub fn t(&self) -> &'static crate::i18n::locale::I18n {
        self.language.i18n()
    }

    pub fn new_tab(&mut self) {
        let tab = Tab::empty(self.next_untitled_index());
        if self.active().is_workspace_placeholder() {
            self.tabs[self.active_tab] = tab;
        } else {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }
        self.camera = CameraState {
            zoom: 40.0,
            ..Default::default()
        };
    }

    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.tabs.push(Tab::workspace_placeholder());
        }
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
    }

    pub fn request_close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].dirty {
            self.pending_close = Some(index);
            self.modal = Some(Modal::CloseConfirm);
        } else {
            self.close_tab(index);
        }
    }

    pub fn confirm_close_tab(&mut self) {
        if let Some(index) = self.pending_close.take() {
            self.close_tab(index);
        }
        self.modal = None;
    }

    pub fn reorder_tabs(&mut self, source: usize, target: usize) {
        if source == target || source >= self.tabs.len() || target >= self.tabs.len() {
            return;
        }
        let active = self.active_tab;
        let tab = self.tabs.remove(source);
        let destination = target.min(self.tabs.len());
        self.tabs.insert(destination, tab);
        self.active_tab = if active == source {
            destination
        } else if source < active && destination >= active {
            active - 1
        } else if source > active && destination <= active {
            active + 1
        } else {
            active
        };
    }

    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    pub fn load_level(&mut self, name: String, level: LevelData) {
        let tab = Tab {
            level: Some(level),
            save: None,
            file_name: name,
            selected: BTreeSet::new(),
            status: String::new(),
            dirty: false,
            revision: 1,
            workspace_placeholder: false,
            history: History::default(),
        };
        self.open_tab(tab);
    }

    #[cfg(test)]
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn load_save(&mut self, name: String, xml: String, file_type: SaveFileType) {
        let parsed = parse_save_data(&file_type, xml.as_bytes()).map_err(|error| error.to_string());
        self.load_save_parsed(name, xml, file_type, parsed);
    }

    pub fn load_save_parsed(
        &mut self,
        name: String,
        xml: String,
        file_type: SaveFileType,
        parsed: Result<SaveData, String>,
    ) {
        let (data, parse_error) = match parsed {
            Ok(data) => (Some(data), None),
            Err(error) => (None, Some(error)),
        };
        let tab = Tab {
            level: None,
            save: Some(SaveDocument {
                xml,
                file_type,
                data,
                selected: BTreeSet::new(),
                view: SaveViewMode::Table,
                parse_error,
                filter: String::new(),
                history: SaveHistory::default(),
            }),
            file_name: name,
            selected: BTreeSet::new(),
            status: String::new(),
            dirty: false,
            revision: 1,
            workspace_placeholder: false,
            history: History::default(),
        };
        self.open_tab(tab);
    }

    fn next_untitled_index(&self) -> usize {
        self.tabs
            .iter()
            .filter_map(|tab| tab.file_name.strip_prefix("Untitled "))
            .filter_map(|index| index.parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1
    }

    fn open_tab(&mut self, tab: Tab) {
        if self.active().is_workspace_placeholder() || self.active().is_empty_untitled() {
            self.tabs[self.active_tab] = tab;
        } else {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn materialize_workspace_placeholder(&mut self) {
        if self.active().is_workspace_placeholder() {
            self.tabs[self.active_tab] = Tab::empty(self.next_untitled_index());
        }
    }

    fn push_snapshot(&mut self) {
        let tab = self.active_mut();
        let Some(level) = tab.level.clone() else {
            return;
        };
        tab.history.undo.push(Snapshot {
            level,
            selected: tab.selected.clone(),
        });
        if tab.history.undo.len() > UNDO_LIMIT {
            tab.history.undo.remove(0);
        }
        tab.history.redo.clear();
    }

    pub fn mutate_level(&mut self, update: impl FnOnce(&mut LevelData)) {
        self.materialize_workspace_placeholder();
        if self.active().level.is_none() {
            return;
        }
        self.push_snapshot();
        if let Some(level) = self.active_mut().level.as_mut() {
            update(level);
        }
        let tab = self.active_mut();
        tab.dirty = true;
        tab.revision = tab.revision.wrapping_add(1);
    }

    pub fn undo(&mut self) {
        if self.active().save.is_some() {
            self.undo_save();
            return;
        }
        let tab = self.active_mut();
        let Some(snapshot) = tab.history.undo.pop() else {
            return;
        };
        if let Some(level) = tab.level.take() {
            tab.history.redo.push(Snapshot {
                level,
                selected: tab.selected.clone(),
            });
        }
        tab.level = Some(snapshot.level);
        tab.selected = snapshot.selected;
        tab.dirty = true;
        tab.revision = tab.revision.wrapping_add(1);
    }

    pub fn redo(&mut self) {
        if self.active().save.is_some() {
            self.redo_save();
            return;
        }
        let tab = self.active_mut();
        let Some(snapshot) = tab.history.redo.pop() else {
            return;
        };
        if let Some(level) = tab.level.take() {
            tab.history.undo.push(Snapshot {
                level,
                selected: tab.selected.clone(),
            });
        }
        tab.level = Some(snapshot.level);
        tab.selected = snapshot.selected;
        tab.dirty = true;
        tab.revision = tab.revision.wrapping_add(1);
    }

    pub fn can_undo(&self) -> bool {
        self.active().save.as_ref().map_or_else(
            || !self.active().history.undo.is_empty(),
            |save| !save.history.undo.is_empty(),
        )
    }

    pub fn can_redo(&self) -> bool {
        self.active().save.as_ref().map_or_else(
            || !self.active().history.redo.is_empty(),
            |save| !save.history.redo.is_empty(),
        )
    }

    fn undo_save(&mut self) {
        let tab = self.active_mut();
        let Some(save) = tab.save.as_mut() else {
            return;
        };
        let Some(snapshot) = save.history.undo.pop() else {
            return;
        };
        let current = save.snapshot();
        save.history.redo.push(current);
        save.restore(snapshot);
        tab.dirty = true;
    }

    fn redo_save(&mut self) {
        let tab = self.active_mut();
        let Some(save) = tab.save.as_mut() else {
            return;
        };
        let Some(snapshot) = save.history.redo.pop() else {
            return;
        };
        let current = save.snapshot();
        save.history.undo.push(current);
        save.restore(snapshot);
        tab.dirty = true;
    }

    pub fn mutate_save(&mut self, update: impl FnOnce(&mut SaveData)) {
        let tab = self.active_mut();
        let Some(save) = tab.save.as_mut() else {
            return;
        };
        if save.data.is_none() {
            return;
        }
        save.push_undo();
        if let Some(data) = save.data.as_mut() {
            update(data);
        }
        save.sync_xml();
        tab.dirty = true;
    }

    #[cfg(test)]
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn set_save_xml(&mut self, xml: String) {
        let tab = self.active_mut();
        let Some(save) = tab.save.as_mut() else {
            return;
        };
        if save.xml == xml {
            return;
        }
        save.push_undo();
        save.xml = xml;
        match parse_save_data(&save.file_type, save.xml.as_bytes()) {
            Ok(data) => {
                save.data = Some(data);
                save.parse_error = None;
            }
            Err(error) => save.parse_error = Some(error.to_string()),
        }
        tab.dirty = true;
    }

    pub fn begin_save_xml_update(&mut self, xml: String) -> Option<(usize, SaveFileType)> {
        let tab_index = self.active_tab;
        let tab = self.active_mut();
        let save = tab.save.as_mut()?;
        if save.xml == xml {
            return None;
        }
        save.push_undo();
        save.xml = xml;
        save.parse_error = Some("Parsing XML...".to_string());
        tab.dirty = true;
        Some((tab_index, save.file_type))
    }

    pub fn finish_save_xml_update(
        &mut self,
        tab_index: usize,
        xml: &str,
        parsed: Result<SaveData, String>,
    ) {
        let Some(tab) = self.tabs.get_mut(tab_index) else {
            return;
        };
        let Some(save) = tab.save.as_mut() else {
            return;
        };
        if save.xml != xml {
            return;
        }
        match parsed {
            Ok(data) => {
                save.data = Some(data);
                save.parse_error = None;
            }
            Err(error) => save.parse_error = Some(error),
        }
    }

    pub fn set_save_selection(&mut self, index: usize, additive: bool) {
        let Some(save) = self.active_mut().save.as_mut() else {
            return;
        };
        if !additive {
            save.selected.clear();
        }
        if additive && save.selected.contains(&index) {
            save.selected.remove(&index);
        } else {
            save.selected.insert(index);
        }
    }

    pub fn select_all_save(&mut self) {
        let Some(save) = self.active_mut().save.as_mut() else {
            return;
        };
        save.selected = (0..save.len()).collect();
    }

    pub fn delete_selected_save(&mut self) {
        let indices = self
            .active()
            .save
            .as_ref()
            .map(|save| save.selected.clone())
            .unwrap_or_default();
        if indices.is_empty() {
            return;
        }
        self.mutate_save(|data| retain_unselected_save_rows(data, &indices));
        if let Some(save) = self.active_mut().save.as_mut() {
            save.selected.clear();
        }
    }

    pub fn duplicate_selected_save(&mut self) {
        let indices = self
            .active()
            .save
            .as_ref()
            .map(|save| save.selected.clone())
            .unwrap_or_default();
        if indices.is_empty() {
            return;
        }
        self.mutate_save(|data| duplicate_save_rows(data, &indices));
    }

    pub fn add_save_row(&mut self) {
        self.mutate_save(|data| match data {
            SaveData::Progress(entries) => entries.push(ProgressEntry {
                key: "new_key".to_string(),
                value_type: "String".to_string(),
                value: String::new(),
            }),
            SaveData::Contraption(parts) => parts.push(ContraptionPart {
                x: 0,
                y: 0,
                part_type: 0,
                custom_part_index: 0,
                rot: 0,
                flipped: false,
            }),
            SaveData::Achievements(entries) => entries.push(AchievementEntry {
                id: "new_achievement".to_string(),
                progress: 0.0,
                completed: false,
                synced: false,
            }),
        });
    }

    pub fn select(&mut self, index: ObjectIndex, additive: bool) {
        let tab = self.active_mut();
        if !additive {
            tab.selected.clear();
        }
        if additive && tab.selected.contains(&index) {
            tab.selected.remove(&index);
        } else {
            tab.selected.insert(index);
        }
        self.selection_anchor = Some(index);
    }

    pub fn select_from_tree(&mut self, index: ObjectIndex, additive: bool, range: bool) {
        if range && let Some(anchor) = self.selection_anchor {
            if !additive {
                self.active_mut().selected.clear();
            }
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            let object_count = self
                .active()
                .level
                .as_ref()
                .map_or(0, |level| level.objects.len());
            self.active_mut()
                .selected
                .extend((start..=end).filter(|value| *value < object_count));
            return;
        }
        self.select(index, additive);
    }

    pub fn move_tree_object(&mut self, source: ObjectIndex, target: ObjectIndex) {
        if source == target {
            return;
        }
        let into_parent = matches!(
            self.active()
                .level
                .as_ref()
                .and_then(|level| level.objects.get(target)),
            Some(LevelObject::Parent(_))
        );
        let mut moved = None;
        self.mutate_level(|level| {
            moved = level.move_object(
                source,
                if into_parent {
                    DropPosition::IntoParent(target)
                } else {
                    DropPosition::After(target)
                },
            );
        });
        if let Some(index) = moved {
            self.active_mut().selected = BTreeSet::from([index]);
            self.selection_anchor = Some(index);
        }
    }

    pub fn set_selection(&mut self, indices: impl IntoIterator<Item = ObjectIndex>) {
        let object_count = self
            .active()
            .level
            .as_ref()
            .map_or(0, |level| level.objects.len());
        self.active_mut().selected = indices
            .into_iter()
            .filter(|index| *index < object_count)
            .collect();
    }

    pub fn select_all(&mut self) {
        if self.active().save.is_some() {
            self.select_all_save();
            return;
        }
        let tab = self.active_mut();
        let count = tab.level.as_ref().map_or(0, |level| level.objects.len());
        tab.selected = (0..count).collect();
    }

    pub fn has_selection(&self) -> bool {
        self.active().save.as_ref().map_or_else(
            || !self.active().selected.is_empty(),
            |save| !save.selected.is_empty(),
        )
    }

    pub fn delete_selected(&mut self) {
        if self.active().save.is_some() {
            self.delete_selected_save();
            return;
        }
        let mut indices: Vec<_> = self.active().selected.iter().copied().collect();
        if indices.is_empty() {
            return;
        }
        indices.sort_unstable_by(|a, b| b.cmp(a));
        self.mutate_level(|level| {
            for index in indices {
                level.delete_object(index);
            }
        });
        self.active_mut().selected.clear();
    }

    pub fn copy_selected(&mut self) {
        let Some(level) = self.active().level.as_ref() else {
            return;
        };
        let subtrees = self
            .active()
            .selected
            .iter()
            .filter(|&&index| index < level.objects.len())
            .map(|&index| level.clone_subtree(index))
            .collect::<Vec<_>>();
        if !subtrees.is_empty() {
            self.clipboard = Some(Clipboard { subtrees });
        }
    }

    pub fn cut_selected(&mut self) {
        self.copy_selected();
        self.delete_selected();
    }

    pub fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let parent = self
            .active()
            .selected
            .iter()
            .next()
            .copied()
            .filter(|&index| {
                matches!(
                    self.active()
                        .level
                        .as_ref()
                        .and_then(|level| level.objects.get(index)),
                    Some(LevelObject::Parent(_))
                )
            });
        let mut pasted = Vec::new();
        self.mutate_level(|level| {
            for subtree in clipboard.subtrees {
                pasted.push(level.paste_subtree(&subtree, PastePosition::AppendTo(parent)));
            }
        });
        self.active_mut().selected = pasted.into_iter().collect();
    }

    pub fn duplicate_selected(&mut self) {
        if self.active().save.is_some() {
            self.duplicate_selected_save();
            return;
        }
        let Some(level) = self.active().level.as_ref() else {
            return;
        };
        let items = self
            .active()
            .selected
            .iter()
            .copied()
            .filter(|&index| index < level.objects.len())
            .map(|index| {
                let parent = match &level.objects[index] {
                    LevelObject::Prefab(prefab) => prefab.parent,
                    LevelObject::Parent(parent) => parent.parent,
                };
                (level.clone_subtree(index), parent)
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return;
        }

        let mut duplicated = Vec::with_capacity(items.len());
        self.mutate_level(|level| {
            for (subtree, parent) in items {
                let index = level.paste_subtree(&subtree, PastePosition::AppendTo(parent));
                match &mut level.objects[index] {
                    LevelObject::Prefab(prefab) => {
                        prefab.position.x += 1.0;
                        prefab.position.y -= 1.0;
                    }
                    LevelObject::Parent(parent) => {
                        parent.position.x += 1.0;
                        parent.position.y -= 1.0;
                    }
                }
                duplicated.push(index);
            }
        });
        self.selection_anchor = duplicated.last().copied();
        self.active_mut().selected = duplicated.into_iter().collect();
    }

    pub fn clear_selection(&mut self) {
        if let Some(save) = self.active_mut().save.as_mut() {
            save.selected.clear();
        } else {
            self.active_mut().selected.clear();
        }
    }

    pub fn request_delete_selected(&mut self) {
        if self.active().save.is_some() {
            self.delete_selected_save();
            return;
        }
        self.pending_delete = self.active().selected.iter().copied().collect();
        if !self.pending_delete.is_empty() {
            self.modal = Some(Modal::DeleteConfirm);
        }
    }

    pub fn confirm_delete(&mut self) {
        if self.pending_delete.is_empty() {
            self.modal = None;
            return;
        }
        self.active_mut().selected = self.pending_delete.iter().copied().collect();
        self.delete_selected();
        self.pending_delete.clear();
        self.modal = None;
    }

    pub fn cancel_delete(&mut self) {
        self.pending_delete.clear();
        self.modal = None;
    }

    pub fn add_parent(&mut self) {
        let mut new_index = None;
        self.mutate_level(|level| {
            let index = level.objects.len();
            level.objects.push(LevelObject::Parent(ParentObject {
                name: "Group".to_string(),
                position: Vec3::default(),
                children: Vec::new(),
                parent: None,
            }));
            level.roots.push(index);
            new_index = Some(index);
        });
        self.active_mut().selected = new_index.into_iter().collect();
        self.modal = None;
    }

    fn preferred_terrain_prefab_identity(&self, has_collider: bool) -> (String, i16) {
        let tab = self.active();
        tab.level.as_ref().map_or_else(
            || ("e2dTerrainBase".to_string(), 0),
            |level| {
                badpiggies_editor_core::domain::terrain_prefab::preferred_terrain_prefab_identity(
                    level,
                    Some(&tab.file_name),
                    has_collider,
                )
            },
        )
    }

    pub fn set_add_object_data_type(&mut self, data_type: DataType) {
        let previous_default = match self.add_object.data_type {
            DataType::Terrain => {
                self.preferred_terrain_prefab_identity(self.add_object.terrain_has_collider)
                    .0
            }
            _ => "NewObject".to_string(),
        };
        let use_default_name =
            self.add_object.name.trim().is_empty() || self.add_object.name == previous_default;
        self.add_object.data_type = data_type;
        if use_default_name {
            let (name, prefab_index) = match data_type {
                DataType::Terrain => {
                    self.preferred_terrain_prefab_identity(self.add_object.terrain_has_collider)
                }
                _ => ("NewObject".to_string(), 0),
            };
            self.add_object.name = name;
            self.add_object.prefab_index = prefab_index;
        }
    }

    pub fn set_add_object_terrain_has_collider(&mut self, has_collider: bool) {
        let previous_identity =
            self.preferred_terrain_prefab_identity(self.add_object.terrain_has_collider);
        let use_default_identity = self.add_object.name.trim().is_empty()
            || (self.add_object.name == previous_identity.0
                && self.add_object.prefab_index == previous_identity.1);
        self.add_object.terrain_has_collider = has_collider;
        if use_default_identity {
            let (name, prefab_index) = self.preferred_terrain_prefab_identity(has_collider);
            self.add_object.name = name;
            self.add_object.prefab_index = prefab_index;
        }
    }

    pub fn add_terrain(&mut self) {
        let (name, prefab_index) =
            self.preferred_terrain_prefab_identity(self.add_object.terrain_has_collider);
        self.add_object.name = name;
        self.add_object.prefab_index = prefab_index;
        self.add_object.data_type = DataType::Terrain;
        self.add_prefab();
    }

    pub fn add_prefab(&mut self) {
        let draft = self.add_object.clone();
        let file_name = self.active().file_name.clone();
        let mut new_index = None;
        self.mutate_level(|level| {
            let index = level.objects.len();
            let object = match draft.data_type {
                DataType::Terrain => {
                    let local_nodes = vec![
                        badpiggies_editor_core::domain::terrain_gen::CurveNode {
                            position: Vec2 { x: -5.0, y: -0.25 },
                            texture: 1,
                        },
                        badpiggies_editor_core::domain::terrain_gen::CurveNode {
                            position: Vec2 { x: -1.5, y: 0.25 },
                            texture: 1,
                        },
                        badpiggies_editor_core::domain::terrain_gen::CurveNode {
                            position: Vec2 { x: 1.5, y: 0.25 },
                            texture: 1,
                        },
                        badpiggies_editor_core::domain::terrain_gen::CurveNode {
                            position: Vec2 { x: 5.0, y: -0.25 },
                            texture: 1,
                        },
                    ];
                    let mut prefab = badpiggies_editor_core::domain::terrain_prefab::build_terrain_prefab_from_local_nodes(
                        level,
                        Some(&file_name),
                        Vec2 {
                            x: draft.position.x,
                            y: draft.position.y,
                        },
                        local_nodes,
                        draft.terrain_has_collider,
                    );
                    if !draft.name.trim().is_empty() {
                        prefab.name = draft.name.trim().to_string();
                    }
                    LevelObject::Prefab(prefab)
                }
                DataType::PrefabOverrides => LevelObject::Prefab(PrefabInstance {
                    name: draft.name.clone(),
                    position: draft.position,
                    prefab_index: draft.prefab_index,
                    rotation: draft.rotation,
                    scale: draft.scale,
                    data_type: draft.data_type,
                    terrain_data: None,
                    override_data: Some(PrefabOverrideData {
                        raw_text: format!("GameObject {}\n", draft.name),
                        raw_bytes: format!("GameObject {}\n", draft.name).into_bytes(),
                    }),
                    parent: None,
                }),
                DataType::None => LevelObject::Prefab(PrefabInstance {
                    name: draft.name.clone(),
                    position: draft.position,
                    prefab_index: draft.prefab_index,
                    rotation: draft.rotation,
                    scale: draft.scale,
                    data_type: draft.data_type,
                    terrain_data: None,
                    override_data: None,
                    parent: None,
                }),
            };
            level.objects.push(object);
            level.roots.push(index);
            new_index = Some(index);
        });
        self.active_mut().selected = new_index.into_iter().collect();
        self.add_object = AddObjectDraft::default();
        self.modal = None;
    }

    pub fn move_objects(&mut self, anchor_index: ObjectIndex, dx: f32, dy: f32) {
        let mut indices = if self.active().selected.contains(&anchor_index) {
            self.active().selected.iter().copied().collect::<Vec<_>>()
        } else {
            vec![anchor_index]
        };
        indices.sort_unstable();
        indices.dedup();
        self.mutate_level(|level| {
            for &index in &indices {
                let Some(object) = level.objects.get_mut(index) else {
                    continue;
                };
                match object {
                    LevelObject::Prefab(prefab) => {
                        prefab.position.x += dx;
                        prefab.position.y += dy;
                    }
                    LevelObject::Parent(parent) => {
                        parent.position.x += dx;
                        parent.position.y += dy;
                    }
                }
            }
            sync_override_transforms(level, &indices);
        });
    }

    pub fn rotate_objects(&mut self, anchor_index: ObjectIndex, degrees: f32) {
        let indices = if self.active().selected.contains(&anchor_index) {
            self.active().selected.iter().copied().collect::<Vec<_>>()
        } else {
            vec![anchor_index]
        };
        self.mutate_level(|level| {
            for &index in &indices {
                if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
                    prefab.rotation.z += degrees;
                }
            }
            sync_override_transforms(level, &indices);
        });
    }

    pub fn scale_object(&mut self, index: ObjectIndex, x: f32, y: f32) {
        self.mutate_level(|level| {
            if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
                prefab.scale.x = x;
                prefab.scale.y = y;
                sync_override_transforms(level, &[index]);
            }
        });
    }

    pub fn flip_objects(&mut self, indices: &[ObjectIndex], horizontal: bool) {
        let indices = indices.to_vec();
        self.mutate_level(|level| {
            for &index in &indices {
                if let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) {
                    if horizontal {
                        prefab.scale.x = -prefab.scale.x;
                    } else {
                        prefab.scale.y = -prefab.scale.y;
                    }
                }
            }
            sync_override_transforms(level, &indices);
        });
    }

    pub fn flip_selected(&mut self, horizontal: bool) {
        let indices = self.active().selected.iter().copied().collect::<Vec<_>>();
        self.flip_objects(&indices, horizontal);
    }

    pub fn move_terrain_node(
        &mut self,
        object_index: ObjectIndex,
        node_index: usize,
        position: Vec2,
    ) {
        self.mutate_level(|level| {
            let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(object_index) else {
                return;
            };
            let Some(terrain) = prefab.terrain_data.as_mut() else {
                return;
            };
            let mut nodes =
                badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
            if let Some(node) = nodes.get_mut(node_index) {
                node.position = position;
                badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(terrain, &nodes);
            }
        });
    }

    pub fn edit_terrain_node(
        &mut self,
        action: &str,
        object_index: ObjectIndex,
        node_index: usize,
        position: Option<Vec2>,
    ) {
        self.mutate_level(|level| {
            let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(object_index) else {
                return;
            };
            let Some(terrain) = prefab.terrain_data.as_mut() else {
                return;
            };
            let mut nodes =
                badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
            match action {
                "delete" if nodes.len() > 2 && node_index < nodes.len() => {
                    nodes.remove(node_index);
                }
                "insert" => {
                    let Some(position) = position else {
                        return;
                    };
                    let texture = nodes.get(node_index).map_or(0, |node| node.texture);
                    nodes.insert(
                        (node_index + 1).min(nodes.len()),
                        badpiggies_editor_core::domain::terrain_gen::CurveNode {
                            position,
                            texture,
                        },
                    );
                }
                "toggle_texture" => {
                    if let Some(node) = nodes.get_mut(node_index) {
                        node.texture = usize::from(node.texture == 0);
                    }
                }
                _ => return,
            }
            badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(terrain, &nodes);
        });
    }

    pub fn draw_terrain(
        &mut self,
        points: Vec<Vec2>,
        closed: bool,
        texture_index: usize,
        has_collider: bool,
    ) {
        if points.len() < 2 {
            return;
        }
        let file_name = self.active().file_name.clone();
        let selected_terrain = self
            .active()
            .selected
            .iter()
            .copied()
            .next()
            .filter(|_| self.active().selected.len() == 1)
            .filter(|index| {
                matches!(
                    self.active()
                        .level
                        .as_ref()
                        .and_then(|level| level.objects.get(*index)),
                    Some(LevelObject::Prefab(prefab)) if prefab.terrain_data.is_some()
                )
            });
        let mut created_index = None;
        self.mutate_level(|level| {
            if let Some(index) = selected_terrain {
                let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
                    return;
                };
                let Some(terrain) = prefab.terrain_data.as_mut() else {
                    return;
                };
                let mut nodes =
                    badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
                if badpiggies_editor_core::domain::terrain_gen::is_closed_loop(&nodes) {
                    nodes.pop();
                }
                for point in points.iter().copied() {
                    let local = Vec2 {
                        x: point.x - prefab.position.x,
                        y: point.y - prefab.position.y,
                    };
                    if nodes.last().is_some_and(|node| {
                        (node.position.x - local.x).abs() < 0.0001
                            && (node.position.y - local.y).abs() < 0.0001
                    }) {
                        continue;
                    }
                    nodes.push(badpiggies_editor_core::domain::terrain_gen::CurveNode {
                        position: local,
                        texture: texture_index.min(1),
                    });
                }
                close_terrain_nodes(&mut nodes, closed);
                badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(terrain, &nodes);
                return;
            }

            let center = Vec2 {
                x: points.iter().map(|point| point.x).sum::<f32>() / points.len() as f32,
                y: points.iter().map(|point| point.y).sum::<f32>() / points.len() as f32,
            };
            let mut nodes = points
                .iter()
                .map(
                    |point| badpiggies_editor_core::domain::terrain_gen::CurveNode {
                        position: Vec2 {
                            x: point.x - center.x,
                            y: point.y - center.y,
                        },
                        texture: texture_index.min(1),
                    },
                )
                .collect::<Vec<_>>();
            close_terrain_nodes(&mut nodes, closed);
            let index = level.objects.len();
            let prefab = badpiggies_editor_core::domain::terrain_prefab::build_terrain_prefab_from_local_nodes(
                level,
                Some(&file_name),
                center,
                nodes,
                has_collider,
            );
            level.objects.push(LevelObject::Prefab(prefab));
            level.roots.push(index);
            created_index = Some(index);
        });
        if let Some(index) = created_index {
            self.active_mut().selected = BTreeSet::from([index]);
        }
    }

    pub fn fit_view(&mut self) {
        if self.active().level.is_none() {
            return;
        }
        self.camera_command = self.camera_command.wrapping_add(1);
    }

    pub fn current_level_warnings(&self) -> Vec<LevelWarning> {
        self.active()
            .level
            .as_ref()
            .filter(|level| !level.objects.is_empty())
            .map(collect_level_warnings)
            .unwrap_or_default()
    }

    pub fn request_preview_state(&mut self, preview_state: PreviewState) {
        if preview_state == self.preview_state {
            return;
        }
        if preview_state == PreviewState::Build || self.current_level_warnings().is_empty() {
            self.preview_state = preview_state;
            return;
        }
        self.pending_preview_state = Some(preview_state);
        self.modal = Some(Modal::LevelWarnings);
    }

    pub fn confirm_level_warnings(&mut self) {
        if let Some(preview_state) = self.pending_preview_state.take() {
            self.preview_state = preview_state;
        }
        self.modal = None;
    }

    pub fn update_bounds(&mut self, target: &str, bounds: [f32; 4]) {
        self.mutate_level(|level| match target {
            "initial_view" => update_initial_view_bounds_in_level(level, bounds),
            "construction_view" => update_construction_view_bounds_in_level(level, bounds),
            _ => update_camera_limits_in_level(level, bounds),
        });
    }

    pub fn update_route_node(&mut self, index: usize, position: Vec2) {
        self.mutate_level(|level| {
            update_camera_preview_control_point_in_level(level, index, position);
        });
    }

    pub fn status_text(&self) -> String {
        let tab = self.active();
        if !tab.status.is_empty() {
            return tab.status.clone();
        }
        if tab.is_workspace_placeholder() {
            return self.t().get("status_welcome");
        }
        if let Some(level) = tab.level.as_ref() {
            return self
                .t()
                .fmt_status_loaded(level.objects.len(), level.roots.len());
        }
        self.t().get("status_welcome")
    }
}

fn retain_unselected_save_rows(data: &mut SaveData, selected: &BTreeSet<usize>) {
    match data {
        SaveData::Progress(entries) => retain_unselected(entries, selected),
        SaveData::Contraption(parts) => retain_unselected(parts, selected),
        SaveData::Achievements(entries) => retain_unselected(entries, selected),
    }
}

fn retain_unselected<T>(items: &mut Vec<T>, selected: &BTreeSet<usize>) {
    let mut index = 0;
    items.retain(|_| {
        let retain = !selected.contains(&index);
        index += 1;
        retain
    });
}

fn duplicate_save_rows(data: &mut SaveData, selected: &BTreeSet<usize>) {
    match data {
        SaveData::Progress(entries) => append_selected_clones(entries, selected),
        SaveData::Contraption(parts) => append_selected_clones(parts, selected),
        SaveData::Achievements(entries) => append_selected_clones(entries, selected),
    }
}

fn append_selected_clones<T: Clone>(items: &mut Vec<T>, selected: &BTreeSet<usize>) {
    let copies = selected
        .iter()
        .filter_map(|index| items.get(*index).cloned())
        .collect::<Vec<_>>();
    items.extend(copies);
}

fn update_camera_limits_in_level(level: &mut LevelData, vals: [f32; 4]) {
    let Some(prefab) = find_prefab_mut(level, "LevelManager") else {
        return;
    };
    let mut nodes = level_manager_override_nodes(prefab);
    let level_manager = ensure_level_manager_component(&mut nodes);
    let camera_limits = ensure_child_node(level_manager, "Generic", "m_cameraLimits");
    let top_left = ensure_child_node(camera_limits, "Vector2", "topLeft");
    set_float_child(top_left, "x", vals[0]);
    set_float_child(top_left, "y", vals[1]);
    let size = ensure_child_node(camera_limits, "Vector2", "size");
    set_float_child(size, "x", vals[2]);
    set_float_child(size, "y", vals[3]);
    write_override_nodes(prefab, &nodes);
}

fn update_initial_view_bounds_in_level(level: &mut LevelData, vals: [f32; 4]) {
    let [top_x, top_y, width, height] = vals;
    let center_x = top_x + width * 0.5;
    let center_y = top_y - height * 0.5;
    let zoom_out = (width.max(height) * 0.5).max(0.5);
    let goal_position = level
        .objects
        .iter()
        .find(|object| object.name().starts_with("GoalArea") || object.name() == "Goal")
        .map(LevelObject::position)
        .unwrap_or_default();
    let Some(prefab) = find_prefab_mut(level, "LevelManager") else {
        return;
    };
    let mut nodes = level_manager_override_nodes(prefab);
    let level_manager = ensure_level_manager_component(&mut nodes);
    let z = level_manager
        .child("Vector3", "m_previewOffset")
        .and_then(|offset| offset.child("Float", "z"))
        .and_then(|node| node.value.as_deref())
        .and_then(|value| value.parse().ok())
        .unwrap_or(-10.0);
    let offset = ensure_child_node(level_manager, "Vector3", "m_previewOffset");
    set_float_child(offset, "x", center_x - goal_position.x);
    set_float_child(offset, "y", center_y - goal_position.y);
    set_float_child(offset, "z", z);
    set_float_child(level_manager, "m_previewZoomOut", zoom_out);
    write_override_nodes(prefab, &nodes);
}

fn update_construction_view_bounds_in_level(level: &mut LevelData, vals: [f32; 4]) {
    let [top_x, top_y, width, height] = vals;
    let center_x = top_x + width * 0.5;
    let center_y = top_y - height * 0.5;
    let half = (width.max(height) * 0.5).max(0.5);
    let level_start = level
        .objects
        .iter()
        .find(|object| object.name() == "LevelStart")
        .map(LevelObject::position)
        .unwrap_or_default();
    let Some(prefab) = find_prefab_mut(level, "LevelManager") else {
        return;
    };
    let mut nodes = level_manager_override_nodes(prefab);
    let level_manager = ensure_level_manager_component(&mut nodes);
    let z = level_manager
        .child("Vector3", "m_constructionOffset")
        .and_then(|offset| offset.child("Float", "z"))
        .and_then(|node| node.value.as_deref())
        .and_then(|value| value.parse().ok())
        .unwrap_or(-half);
    let offset = ensure_child_node(level_manager, "Vector3", "m_constructionOffset");
    set_float_child(offset, "x", center_x - level_start.x);
    set_float_child(offset, "y", center_y - level_start.y);
    set_float_child(offset, "z", z);
    write_override_nodes(prefab, &nodes);
}

fn update_camera_preview_control_point_in_level(
    level: &mut LevelData,
    index: usize,
    position: Vec2,
) {
    let Some(prefab) = find_prefab_mut(level, "CameraSystem") else {
        return;
    };
    let mut nodes = prefab
        .override_data
        .as_ref()
        .map(|data| parse_override_text(&data.raw_text))
        .unwrap_or_default();
    let camera_system = ensure_game_object_node(&mut nodes, "CameraSystem");
    let owner = if camera_system.child("GameObject", "GameCamera").is_some() {
        camera_system
            .child_mut("GameObject", "GameCamera")
            .expect("GameCamera child exists")
    } else {
        camera_system
    };
    let preview = ensure_component_node(owner, "CameraPreview");
    let points = ensure_child_node(preview, "Array", "m_controlPoints");
    let existing_size = points
        .child("ArraySize", "size")
        .and_then(|node| node.value.as_deref())
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();
    ensure_child_node(points, "ArraySize", "size").value =
        Some(existing_size.max(index + 1).to_string());
    let element = ensure_child_node(points, "Element", &index.to_string());
    let data = ensure_child_node(element, "Generic", "data");
    let node_position = ensure_child_node(data, "Vector2", "position");
    set_float_child(node_position, "x", position.x);
    set_float_child(node_position, "y", position.y);
    write_override_nodes(prefab, &nodes);
}

fn find_prefab_mut<'a>(level: &'a mut LevelData, name: &str) -> Option<&'a mut PrefabInstance> {
    level.objects.iter_mut().find_map(|object| match object {
        LevelObject::Prefab(prefab) if prefab.name == name => Some(prefab),
        _ => None,
    })
}

fn level_manager_override_nodes(prefab: &PrefabInstance) -> Vec<OverrideNode> {
    prefab
        .override_data
        .as_ref()
        .map(|data| parse_override_text(&data.raw_text))
        .unwrap_or_default()
}

fn write_override_nodes(prefab: &mut PrefabInstance, nodes: &[OverrideNode]) {
    let raw_text = serialize_override_tree(nodes);
    prefab.override_data = Some(PrefabOverrideData {
        raw_bytes: raw_text.as_bytes().to_vec(),
        raw_text,
    });
    prefab.data_type = DataType::PrefabOverrides;
}

fn ensure_level_manager_component(nodes: &mut Vec<OverrideNode>) -> &mut OverrideNode {
    if find_first_node_mut(nodes, &|node| {
        node.node_type == "Component" && node.name.rsplit('.').next() == Some("LevelManager")
    })
    .is_none()
    {
        let root_index = nodes
            .iter()
            .position(|node| {
                node.node_type == "GameObject" && node.name.starts_with("LevelManager")
            })
            .unwrap_or_else(|| {
                nodes.push(OverrideNode {
                    node_type: "GameObject".to_string(),
                    name: "LevelManager".to_string(),
                    value: None,
                    children: Vec::new(),
                });
                nodes.len() - 1
            });
        nodes[root_index].children.push(OverrideNode {
            node_type: "Component".to_string(),
            name: "LevelManager".to_string(),
            value: None,
            children: Vec::new(),
        });
    }
    find_first_node_mut(nodes, &|node| {
        node.node_type == "Component" && node.name.rsplit('.').next() == Some("LevelManager")
    })
    .expect("LevelManager component exists")
}

fn ensure_game_object_node<'a>(
    nodes: &'a mut Vec<OverrideNode>,
    name: &str,
) -> &'a mut OverrideNode {
    if nodes
        .iter()
        .all(|node| node.node_type != "GameObject" || node.name != name)
    {
        nodes.push(OverrideNode {
            node_type: "GameObject".to_string(),
            name: name.to_string(),
            value: None,
            children: Vec::new(),
        });
    }
    nodes
        .iter_mut()
        .find(|node| node.node_type == "GameObject" && node.name == name)
        .expect("game object exists")
}

fn ensure_component_node<'a>(parent: &'a mut OverrideNode, name: &str) -> &'a mut OverrideNode {
    let index = parent.children.iter().position(|node| {
        node.node_type == "Component" && node.name.rsplit('.').next() == Some(name)
    });
    let index = index.unwrap_or_else(|| {
        parent.children.push(OverrideNode {
            node_type: "Component".to_string(),
            name: name.to_string(),
            value: None,
            children: Vec::new(),
        });
        parent.children.len() - 1
    });
    &mut parent.children[index]
}

fn ensure_child_node<'a>(
    parent: &'a mut OverrideNode,
    node_type: &str,
    name: &str,
) -> &'a mut OverrideNode {
    let index = parent
        .children
        .iter()
        .position(|node| node.node_type == node_type && node.name == name);
    let index = index.unwrap_or_else(|| {
        parent.children.push(OverrideNode {
            node_type: node_type.to_string(),
            name: name.to_string(),
            value: None,
            children: Vec::new(),
        });
        parent.children.len() - 1
    });
    &mut parent.children[index]
}

fn set_float_child(parent: &mut OverrideNode, name: &str, value: f32) {
    ensure_child_node(parent, "Float", name).value = Some(value.to_string());
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{
        CursorModeState, EditorState, PreviewState, SaveData, SaveFileType, TerrainDrawModeState,
        TerrainPresetState, ThemePreference, default_terrain_data,
    };
    use badpiggies_editor_core::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, Vec2, Vec3,
    };
    use std::collections::BTreeSet;
    use std::sync::Once;

    static INIT_ASSETS: Once = Once::new();

    fn editor() -> EditorState {
        INIT_ASSETS.call_once(|| {
            crate::platform::runtime_assets::preload_required_runtime_assets()
                .expect("load runtime assets for editor state tests");
        });
        EditorState::default()
    }

    #[test]
    fn theme_preferences_map_to_storage_codes_and_shell_classes() {
        assert_eq!(
            ThemePreference::from_code("system"),
            ThemePreference::System
        );
        assert_eq!(ThemePreference::from_code("LIGHT"), ThemePreference::Light);
        assert_eq!(ThemePreference::from_code("dark"), ThemePreference::Dark);
        assert_eq!(
            ThemePreference::from_code("unknown"),
            ThemePreference::System
        );
        assert_eq!(ThemePreference::Light.code(), "light");
        assert_eq!(ThemePreference::Dark.shell_class(), "app-shell dark-theme");
    }

    #[test]
    fn workspace_placeholder_and_untitled_level_are_distinct() {
        let mut editor = editor();
        assert!(editor.active().is_workspace_placeholder());
        assert!(!editor.active().is_empty_untitled());
        assert_eq!(editor.status_text(), editor.t().get("status_welcome"));

        editor.new_tab();

        assert!(!editor.active().is_workspace_placeholder());
        assert!(editor.active().is_empty_untitled());

        editor.add_parent();

        assert!(!editor.active().is_empty_untitled());
    }

    #[test]
    fn status_bar_resolves_real_level_name_without_replacing_file_name() {
        let mut editor = editor();
        editor.load_level("Level_21_data.bytes".to_string(), LevelData::default());

        assert_eq!(editor.active().file_name, "Level_21_data.bytes");
        assert_eq!(
            editor.active().status_bar_file_label().as_deref(),
            Some("Level_21_data.bytes -> Level_21 (1-1)")
        );
    }

    #[test]
    fn duplicate_group_stays_at_its_original_tree_level() {
        let mut editor = editor();
        editor.new_tab();
        editor.add_parent();

        editor.duplicate_selected();

        let level = editor.active().level.as_ref().expect("level");
        assert_eq!(level.roots, vec![0, 1]);
        assert_eq!(level.objects.len(), 2);
        let LevelObject::Parent(duplicate) = &level.objects[1] else {
            panic!("duplicate should remain a group");
        };
        assert_eq!(duplicate.parent, None);
        assert_eq!(
            duplicate.position,
            Vec3 {
                x: 1.0,
                y: -1.0,
                z: 0.0
            }
        );
        assert_eq!(editor.active().selected, BTreeSet::from([1]));
        assert!(editor.clipboard.is_none());
    }

    #[test]
    fn save_table_edits_roundtrip_through_history_and_xml() {
        let mut editor = editor();
        editor.load_save(
            "Progress.dat".to_string(),
            "<?xml version=\"1.0\"?><data><Int key=\"coins\" value=\"5\" /></data>".to_string(),
            SaveFileType::Progress,
        );

        editor.add_save_row();
        let save = editor.active().save.as_ref().expect("save document");
        assert!(matches!(&save.data, Some(SaveData::Progress(entries)) if entries.len() == 2));
        assert!(save.xml.contains("new_key"));

        editor.undo();
        assert!(matches!(
            &editor.active().save.as_ref().expect("save").data,
            Some(SaveData::Progress(entries)) if entries.len() == 1
        ));
        editor.redo();
        assert!(matches!(
            &editor.active().save.as_ref().expect("save").data,
            Some(SaveData::Progress(entries)) if entries.len() == 2
        ));
    }

    #[test]
    fn raw_save_xml_history_restores_invalid_and_valid_documents() {
        let mut editor = editor();
        let original = "<?xml version=\"1.0\"?><data><Int key=\"coins\" value=\"5\" /></data>";
        editor.load_save(
            "Progress.dat".to_string(),
            original.to_string(),
            SaveFileType::Progress,
        );

        editor.set_save_xml("<data><Int".to_string());
        assert!(
            editor
                .active()
                .save
                .as_ref()
                .expect("save")
                .parse_error
                .is_some()
        );

        editor.undo();
        let save = editor.active().save.as_ref().expect("save");
        assert_eq!(save.xml, original);
        assert!(save.parse_error.is_none());

        editor.redo();
        let save = editor.active().save.as_ref().expect("save");
        assert_eq!(save.xml, "<data><Int");
        assert!(save.parse_error.is_some());
    }

    #[test]
    fn canvas_scene_reflects_every_renderer_control() {
        let mut editor = editor();
        editor.show_construction_grid = false;
        editor.show_dark_overlay = false;
        editor.show_ground = true;
        editor.show_terrain_triangles = true;
        editor.show_preview_route = true;
        editor.cursor_mode = CursorModeState::DrawTerrain;
        editor.preview_state = PreviewState::Pause;
        editor.night_vision = false;
        editor.terrain_draw_mode = TerrainDrawModeState::CircularArc;
        editor.terrain_preset = Some(TerrainPresetState::Square);
        editor.terrain_curve_segments = 48;
        editor.terrain_texture_index = 0;
        editor.terrain_has_collider = false;

        let scene = editor.canvas_scene();
        assert!(!scene.construction_grid);
        assert!(!scene.dark_overlay);
        assert!(scene.ground);
        assert!(scene.terrain_triangles);
        assert!(scene.preview_route);
        assert_eq!(scene.cursor_mode, "draw_terrain");
        assert_eq!(scene.preview_state, "pause");
        assert!(!scene.night_vision);
        assert_eq!(scene.terrain_draw_mode, "circular_arc");
        assert_eq!(scene.terrain_preset, Some("square"));
        assert_eq!(scene.terrain_curve_segments, 48);
        assert_eq!(scene.terrain_texture_index, 0);
        assert!(!scene.terrain_has_collider);
    }

    #[test]
    fn canvas_payloads_continue_from_the_selected_terrain_tail() {
        let mut editor = editor();
        editor.load_level(
            "Level.bytes".to_string(),
            LevelData {
                objects: vec![LevelObject::Prefab(PrefabInstance {
                    name: "e2dTerrainBase".to_string(),
                    position: Vec3 {
                        x: 10.0,
                        y: -4.0,
                        z: 0.0,
                    },
                    prefab_index: 0,
                    rotation: Vec3::default(),
                    scale: Vec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                    data_type: DataType::Terrain,
                    terrain_data: Some(Box::new(default_terrain_data())),
                    override_data: None,
                    parent: None,
                })],
                roots: vec![0],
            },
        );
        editor.cursor_mode = CursorModeState::DrawTerrain;
        editor.active_mut().selected = BTreeSet::from([0]);

        let scene_anchor = editor
            .canvas_scene()
            .terrain_continuation_anchor
            .expect("scene continuation anchor");
        let view_anchor = editor
            .canvas_view()
            .terrain_continuation_anchor
            .expect("view continuation anchor");
        assert!((scene_anchor.x - 15.0).abs() < 0.0001);
        assert!((scene_anchor.y - -4.0).abs() < 0.0001);
        assert!((view_anchor.x - scene_anchor.x).abs() < 0.0001);
        assert!((view_anchor.y - scene_anchor.y).abs() < 0.0001);

        editor.cursor_mode = CursorModeState::Select;
        assert!(editor.canvas_view().terrain_continuation_anchor.is_none());
    }

    #[test]
    fn drawn_terrain_reuses_the_current_level_template_identity_and_style() {
        let mut editor = editor();
        let mut terrain = default_terrain_data();
        terrain.fill_texture_index = 32;
        terrain.curve_textures[1].texture_index = 9;
        editor.load_level(
            "Level.bytes".to_string(),
            LevelData {
                objects: vec![LevelObject::Prefab(PrefabInstance {
                    name: "e2dTerrainBase_MM_rock".to_string(),
                    position: Vec3::default(),
                    prefab_index: 12,
                    rotation: Vec3::default(),
                    scale: Vec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                    data_type: DataType::Terrain,
                    terrain_data: Some(Box::new(terrain)),
                    override_data: None,
                    parent: None,
                })],
                roots: vec![0],
            },
        );

        editor.draw_terrain(
            vec![Vec2 { x: -2.0, y: 0.0 }, Vec2 { x: 2.0, y: 0.0 }],
            false,
            1,
            true,
        );

        let LevelObject::Prefab(created) =
            &editor.active().level.as_ref().expect("level").objects[1]
        else {
            panic!("expected terrain prefab");
        };
        assert_eq!(created.name, "e2dTerrainBase_MM_rock");
        assert_eq!(created.prefab_index, 12);
        let terrain = created.terrain_data.as_ref().expect("terrain data");
        assert_eq!(terrain.fill_texture_index, 32);
        assert_eq!(terrain.curve_textures[1].texture_index, 9);
    }

    #[test]
    fn dragged_camera_bounds_are_written_to_level_manager_override() {
        let mut editor = editor();
        editor.load_level(
            "Level.bytes".to_string(),
            LevelData {
                objects: vec![LevelObject::Prefab(PrefabInstance {
                    name: "LevelManager".to_string(),
                    position: Vec3::default(),
                    prefab_index: 0,
                    rotation: Vec3::default(),
                    scale: Vec3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                    data_type: DataType::None,
                    terrain_data: None,
                    override_data: None,
                    parent: None,
                })],
                roots: vec![0],
            },
        );

        editor.update_bounds("camera_limits", [-10.0, 20.0, 30.0, 40.0]);
        let LevelObject::Prefab(prefab) =
            &editor.active().level.as_ref().expect("level").objects[0]
        else {
            panic!("expected prefab");
        };
        let raw = &prefab.override_data.as_ref().expect("override").raw_text;
        assert!(raw.contains("Generic m_cameraLimits"));
        assert!(raw.contains("Float x = -10"));
        assert!(raw.contains("Float y = 20"));
    }
}

pub(crate) fn default_terrain_data() -> TerrainData {
    let nodes = vec![
        badpiggies_editor_core::domain::terrain_gen::CurveNode {
            position: Vec2 { x: -5.0, y: 0.0 },
            texture: 0,
        },
        badpiggies_editor_core::domain::terrain_gen::CurveNode {
            position: Vec2 { x: -1.5, y: 0.5 },
            texture: 0,
        },
        badpiggies_editor_core::domain::terrain_gen::CurveNode {
            position: Vec2 { x: 1.5, y: 0.5 },
            texture: 0,
        },
        badpiggies_editor_core::domain::terrain_gen::CurveNode {
            position: Vec2 { x: 5.0, y: 0.0 },
            texture: 0,
        },
    ];
    let mut terrain = TerrainData {
        fill_texture_tile_offset_x: 0.0,
        fill_texture_tile_offset_y: 0.0,
        fill_mesh: TerrainMesh::default(),
        fill_color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        fill_texture_index: 0,
        curve_mesh: TerrainMesh::default(),
        curve_textures: vec![
            CurveTexture {
                texture_index: 0,
                size: Vec2 { x: 0.1, y: 0.5 },
                fixed_angle: false,
                fade_threshold: 0.5,
            },
            CurveTexture {
                texture_index: 1,
                size: Vec2 { x: 0.1, y: 0.1 },
                fixed_angle: false,
                fade_threshold: 0.0,
            },
        ],
        control_texture_count: 0,
        control_texture_data: None,
        has_collider: true,
        fill_boundary: None,
    };
    badpiggies_editor_core::domain::terrain_gen::regenerate_terrain(&mut terrain, &nodes);
    terrain
}

fn close_terrain_nodes(
    nodes: &mut Vec<badpiggies_editor_core::domain::terrain_gen::CurveNode>,
    closed: bool,
) {
    if !closed || nodes.len() < 2 {
        return;
    }
    let first = nodes[0].clone();
    let last = &nodes[nodes.len() - 1];
    if (first.position.x - last.position.x).abs() > 0.0001
        || (first.position.y - last.position.y).abs() > 0.0001
    {
        nodes.push(first);
    }
}

fn sync_override_transforms(level: &mut LevelData, indices: &[ObjectIndex]) {
    let to_sync = indices
        .iter()
        .filter_map(|&index| {
            let LevelObject::Prefab(prefab) = level.objects.get(index)? else {
                return None;
            };
            if prefab.data_type != DataType::PrefabOverrides || prefab.override_data.is_none() {
                return None;
            }
            let parent_position = prefab
                .parent
                .and_then(|parent| level.objects.get(parent))
                .map(LevelObject::position)
                .unwrap_or_default();
            Some((
                index,
                prefab.position,
                prefab.rotation.z,
                prefab.scale,
                parent_position,
            ))
        })
        .collect::<Vec<_>>();

    for (index, position, rotation, scale, parent_position) in to_sync {
        let Some(LevelObject::Prefab(prefab)) = level.objects.get_mut(index) else {
            continue;
        };
        let Some(override_data) = prefab.override_data.as_mut() else {
            continue;
        };
        let (text, bytes) =
            badpiggies_editor_core::domain::object_deserializer::sync_override_transform(
                &override_data.raw_text,
                position,
                rotation,
                scale,
                parent_position,
                0.0,
            );
        override_data.raw_text = text;
        override_data.raw_bytes = bytes;
    }
}

#[derive(Serialize)]
pub struct CanvasScene {
    pub document_key: String,
    pub revision: u64,
    pub file_name: String,
    pub level: Option<LevelData>,
    pub selected: Vec<ObjectIndex>,
    pub camera_command: u64,
    pub grid: bool,
    pub background: bool,
    pub construction_grid: bool,
    pub dark_overlay: bool,
    pub ground: bool,
    pub terrain_triangles: bool,
    pub preview_route: bool,
    pub cursor_mode: &'static str,
    pub preview_state: &'static str,
    pub night_vision: bool,
    pub terrain_draw_mode: &'static str,
    pub terrain_preset: Option<&'static str>,
    pub terrain_curve_segments: usize,
    pub terrain_texture_index: usize,
    pub terrain_has_collider: bool,
    pub terrain_continuation_anchor: Option<Vec2>,
    pub has_clipboard: bool,
}

#[derive(Serialize)]
pub struct CanvasView {
    pub selected: Vec<ObjectIndex>,
    pub camera_command: u64,
    pub grid: bool,
    pub background: bool,
    pub construction_grid: bool,
    pub dark_overlay: bool,
    pub ground: bool,
    pub terrain_triangles: bool,
    pub preview_route: bool,
    pub cursor_mode: &'static str,
    pub preview_state: &'static str,
    pub night_vision: bool,
    pub terrain_draw_mode: &'static str,
    pub terrain_preset: Option<&'static str>,
    pub terrain_curve_segments: usize,
    pub terrain_texture_index: usize,
    pub terrain_has_collider: bool,
    pub terrain_continuation_anchor: Option<Vec2>,
    pub has_clipboard: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl CanvasScene {
    pub fn into_renderer_payload(self) -> badpiggies_editor_renderer::ScenePayload {
        badpiggies_editor_renderer::ScenePayload {
            document_key: self.document_key,
            revision: self.revision,
            file_name: self.file_name,
            level: self.level,
            view: badpiggies_editor_renderer::ViewPayload {
                selected: self.selected,
                grid: self.grid,
                background: self.background,
                construction_grid: self.construction_grid,
                dark_overlay: self.dark_overlay,
                ground: self.ground,
                terrain_triangles: self.terrain_triangles,
                preview_route: self.preview_route,
                cursor_mode: self.cursor_mode.to_string(),
                preview_state: self.preview_state.to_string(),
                night_vision: self.night_vision,
                terrain_draw_mode: self.terrain_draw_mode.to_string(),
                terrain_preset: self.terrain_preset.map(str::to_string),
                terrain_curve_segments: self.terrain_curve_segments,
                terrain_texture_index: self.terrain_texture_index,
                terrain_has_collider: self.terrain_has_collider,
                terrain_continuation_anchor: self.terrain_continuation_anchor,
                has_clipboard: self.has_clipboard,
                camera_command: self.camera_command,
            },
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl CanvasView {
    pub fn into_renderer_payload(self) -> badpiggies_editor_renderer::ViewPayload {
        badpiggies_editor_renderer::ViewPayload {
            selected: self.selected,
            grid: self.grid,
            background: self.background,
            construction_grid: self.construction_grid,
            dark_overlay: self.dark_overlay,
            ground: self.ground,
            terrain_triangles: self.terrain_triangles,
            preview_route: self.preview_route,
            cursor_mode: self.cursor_mode.to_string(),
            preview_state: self.preview_state.to_string(),
            night_vision: self.night_vision,
            terrain_draw_mode: self.terrain_draw_mode.to_string(),
            terrain_preset: self.terrain_preset.map(str::to_string),
            terrain_curve_segments: self.terrain_curve_segments,
            terrain_texture_index: self.terrain_texture_index,
            terrain_has_collider: self.terrain_has_collider,
            terrain_continuation_anchor: self.terrain_continuation_anchor,
            has_clipboard: self.has_clipboard,
            camera_command: self.camera_command,
        }
    }
}

impl EditorState {
    fn terrain_continuation_anchor(&self) -> Option<Vec2> {
        if self.cursor_mode != CursorModeState::DrawTerrain {
            return None;
        }

        let tab = self.active();
        let index = tab.selected.iter().copied().next()?;
        if tab.selected.len() != 1 {
            return None;
        }
        let LevelObject::Prefab(prefab) = tab.level.as_ref()?.objects.get(index)? else {
            return None;
        };
        let terrain = prefab.terrain_data.as_ref()?;
        let mut nodes = badpiggies_editor_core::domain::terrain_gen::extract_curve_nodes(terrain);
        if badpiggies_editor_core::domain::terrain_gen::is_closed_loop(&nodes) {
            nodes.pop();
        }
        let last = nodes.last()?;
        Some(Vec2 {
            x: prefab.position.x + last.position.x,
            y: prefab.position.y + last.position.y,
        })
    }

    pub fn canvas_scene_identity(&self) -> (String, u64) {
        let tab = self.active();
        (
            format!("{}:{}", self.active_tab, tab.file_name),
            tab.revision,
        )
    }

    pub fn canvas_view(&self) -> CanvasView {
        let tab = self.active();
        CanvasView {
            selected: tab.selected.iter().copied().collect(),
            camera_command: self.camera_command,
            grid: self.show_grid,
            background: self.show_background,
            construction_grid: self.show_construction_grid,
            dark_overlay: self.show_dark_overlay,
            ground: self.show_ground,
            terrain_triangles: self.show_terrain_triangles,
            preview_route: self.show_preview_route,
            cursor_mode: self.cursor_mode.code(),
            preview_state: self.preview_state.code(),
            night_vision: self.night_vision,
            terrain_draw_mode: self.terrain_draw_mode.code(),
            terrain_preset: self.terrain_preset.map(TerrainPresetState::code),
            terrain_curve_segments: self.terrain_curve_segments,
            terrain_texture_index: self.terrain_texture_index,
            terrain_has_collider: self.terrain_has_collider,
            terrain_continuation_anchor: self.terrain_continuation_anchor(),
            has_clipboard: self.clipboard.is_some(),
        }
    }

    pub fn canvas_scene(&self) -> CanvasScene {
        let tab = self.active();
        CanvasScene {
            document_key: format!("{}:{}", self.active_tab, tab.file_name),
            revision: tab.revision,
            file_name: tab.file_name.clone(),
            level: tab.level.clone(),
            selected: tab.selected.iter().copied().collect(),
            camera_command: self.camera_command,
            grid: self.show_grid,
            background: self.show_background,
            construction_grid: self.show_construction_grid,
            dark_overlay: self.show_dark_overlay,
            ground: self.show_ground,
            terrain_triangles: self.show_terrain_triangles,
            preview_route: self.show_preview_route,
            cursor_mode: self.cursor_mode.code(),
            preview_state: self.preview_state.code(),
            night_vision: self.night_vision,
            terrain_draw_mode: self.terrain_draw_mode.code(),
            terrain_preset: self.terrain_preset.map(TerrainPresetState::code),
            terrain_curve_segments: self.terrain_curve_segments,
            terrain_texture_index: self.terrain_texture_index,
            terrain_has_collider: self.terrain_has_collider,
            terrain_continuation_anchor: self.terrain_continuation_anchor(),
            has_clipboard: self.clipboard.is_some(),
        }
    }
}
