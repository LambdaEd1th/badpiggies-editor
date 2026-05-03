//! egui application — main editor UI with three-panel layout.

mod actions;
mod dialogs;
mod menu;
mod properties;
mod save_tables;
pub(crate) mod save_viewer;
mod save_xml;
mod tree;

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use std::collections::BTreeSet;

use crate::locale::{I18n, Language};
use crate::parser;
use crate::renderer::{CursorMode, LevelRenderer, TerrainPresetShape};
use crate::types::*;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_OPEN_RESULT: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
    static WASM_OPEN_XML_SAVE: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
}

/// Maximum number of undo snapshots to keep.
const UNDO_MAX: usize = 100;

/// A snapshot of the editor state for undo/redo.
struct Snapshot {
    level: LevelData,
    selected: BTreeSet<ObjectIndex>,
}

/// Undo/redo history stack.
struct UndoStack {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

/// Clipboard contents for copy/cut/paste.
#[derive(Clone)]
struct Clipboard {
    /// Each entry is a cloned subtree (objects[0] is root of that subtree).
    subtrees: Vec<Vec<LevelObject>>,
}

/// Per-tab editor state (each tab is an independent level editor).
struct Tab {
    /// Currently loaded level data.
    level: Option<LevelData>,
    /// File name of the loaded level.
    file_name: Option<String>,
    /// Native path of the loaded source file, if available.
    source_path: Option<String>,
    /// Currently selected object indices.
    selected: BTreeSet<ObjectIndex>,
    /// Canvas renderer state (owns per-level caches; shares GPU pipelines via Arc).
    renderer: LevelRenderer,
    /// Status message.
    status: String,
    /// Pending delete confirmation: (object_indices, display_label).
    pending_delete: Option<(Vec<ObjectIndex>, String)>,
    /// Undo/redo history.
    history: UndoStack,
    /// Whether properties were changed in the previous frame (for undo coalescing).
    props_changed_prev: bool,
    /// Anchor for Shift+click range selection in the object tree.
    select_anchor: Option<ObjectIndex>,
    /// Save file viewer data (if this tab is viewing a save file).
    pub(super) save_view: Option<save_viewer::SaveViewerData>,
}

impl Tab {
    fn new(renderer: LevelRenderer, welcome_status: String) -> Self {
        Self {
            level: None,
            file_name: None,
            source_path: None,
            selected: BTreeSet::new(),
            renderer,
            status: welcome_status,
            pending_delete: None,
            history: UndoStack {
                undo: Vec::new(),
                redo: Vec::new(),
            },
            props_changed_prev: false,
            select_anchor: None,
            save_view: None,
        }
    }

    /// Display name for the tab.
    fn title(&self, fallback: &str) -> String {
        self.file_name
            .clone()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Whether this tab is a save file viewer (not a level editor).
    fn is_save_tab(&self) -> bool {
        self.save_view.is_some()
    }

    fn load_level(&mut self, name: String, data: Vec<u8>, i18n: &I18n, source_path: Option<String>) {
        match parser::parse_level(data) {
            Ok(level) => {
                let obj_count = level.objects.len();
                let root_count = level.roots.len();
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.source_path = source_path;
                self.selected.clear();
                self.history.undo.clear();
                self.history.redo.clear();
                self.status = i18n.fmt_status_loaded(obj_count, root_count);
            }
            Err(e) => {
                self.status = format!("解析失败: {}", e);
            }
        }
    }

    fn load_level_text(
        &mut self,
        name: String,
        text: &str,
        i18n: &I18n,
        source_path: Option<String>,
    ) {
        let result: Result<LevelData, String> = if name.ends_with(".yaml") || name.ends_with(".yml")
        {
            serde_yaml::from_str(text).map_err(|e| e.to_string())
        } else if name.ends_with(".toml") {
            toml::from_str(text).map_err(|e| e.to_string())
        } else {
            Err("不支持的文件格式".to_string())
        };
        match result {
            Ok(level) => {
                let obj_count = level.objects.len();
                let root_count = level.roots.len();
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.source_path = source_path;
                self.selected.clear();
                self.history.undo.clear();
                self.history.redo.clear();
                self.status = i18n.fmt_status_loaded(obj_count, root_count);
            }
            Err(e) => {
                self.status = format!("解析失败: {}", e);
            }
        }
    }

    fn export_level(&self) -> Option<Vec<u8>> {
        self.level.as_ref().map(parser::serialize_level)
    }

    fn export_yaml(&self) -> Option<String> {
        self.level
            .as_ref()
            .and_then(|l| serde_yaml::to_string(l).ok())
    }

    fn export_toml(&self) -> Option<String> {
        self.level
            .as_ref()
            .and_then(|l| toml::to_string_pretty(l).ok())
    }

    /// Determine the target parent for a paste operation.
    /// Uses the first selected object to find the parent.
    fn paste_target_parent(
        level: &LevelData,
        selected: &BTreeSet<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let &sel = selected.iter().next()?;
        if sel >= level.objects.len() {
            return None;
        }
        match &level.objects[sel] {
            LevelObject::Parent(_) => Some(sel),
            LevelObject::Prefab(p) => p.parent,
        }
    }
}

/// Main application state.
pub struct EditorApp {
    /// Open tabs.
    tabs: Vec<Tab>,
    /// Index of the currently active tab.
    active_tab: usize,
    /// Add-object dialog state.
    show_add_dialog: bool,
    add_obj_is_parent: bool,
    add_obj_data_type: DataType,
    add_obj_name: String,
    add_obj_prefab_index: i16,
    add_obj_position: Vec3,
    add_obj_rotation: Vec3,
    add_obj_scale: Vec3,
    /// Pending file data from drag-and-drop or file picker.
    #[cfg(target_arch = "wasm32")]
    pending_file: Option<(String, Vec<u8>)>,
    /// Whether the object tree panel is visible.
    show_object_tree: bool,
    /// Whether the properties panel is visible.
    show_properties: bool,
    /// Whether the shortcuts help window is visible.
    show_shortcuts: bool,
    /// Whether the about window is visible.
    show_about: bool,
    /// Object clipboard for copy/cut/paste (shared across tabs).
    clipboard: Option<Clipboard>,
    /// Graphics API backend name (e.g. "Metal", "Vulkan").
    gpu_backend: String,
    /// Current UI language.
    lang: Language,
    /// Active cursor/tool mode.
    cursor_mode: CursorMode,
    /// Whether the tool panel is visible.
    show_tools: bool,
}

impl LevelData {
    /// Deep-clone the subtree rooted at `root_idx` into a self-contained
    /// `Clipboard`.  All internal parent/children indices are remapped to
    /// be relative to the cloned vec (root is always index 0).
    fn clone_subtree(&self, root_idx: ObjectIndex) -> Vec<LevelObject> {
        // Collect all indices in the subtree (BFS).
        let mut indices = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root_idx);
        while let Some(idx) = queue.pop_front() {
            indices.push(idx);
            if let LevelObject::Parent(p) = &self.objects[idx] {
                for &child in &p.children {
                    queue.push_back(child);
                }
            }
        }
        // Build old→new index mapping.
        let remap: std::collections::HashMap<ObjectIndex, ObjectIndex> = indices
            .iter()
            .enumerate()
            .map(|(new, &old)| (old, new))
            .collect();
        // Clone and remap.
        indices
            .iter()
            .map(|&old_idx| {
                let mut obj = self.objects[old_idx].clone();
                match &mut obj {
                    LevelObject::Prefab(p) => {
                        p.parent = p.parent.and_then(|pi| remap.get(&pi).copied());
                    }
                    LevelObject::Parent(p) => {
                        p.parent = p.parent.and_then(|pi| remap.get(&pi).copied());
                        p.children = p
                            .children
                            .iter()
                            .filter_map(|c| remap.get(c).copied())
                            .collect();
                    }
                }
                obj
            })
            .collect()
    }

    /// Paste a subtree (Vec<LevelObject>) into the level. All objects are appended to
    /// the arena, then the pasted root is inserted either as the last child/root
    /// or at an exact `DropPosition`.
    /// Returns the index of the pasted root.
    fn paste_subtree(
        &mut self,
        subtree: &[LevelObject],
        paste_position: PastePosition,
    ) -> ObjectIndex {
        let root_parent = match paste_position {
            PastePosition::AppendTo(parent_idx) => parent_idx,
            PastePosition::Exact(DropPosition::IntoParent(target)) => Some(target),
            PastePosition::Exact(DropPosition::Before(target))
            | PastePosition::Exact(DropPosition::After(target)) => {
                match self.objects.get(target) {
                    Some(LevelObject::Prefab(prefab)) => prefab.parent,
                    Some(LevelObject::Parent(parent)) => parent.parent,
                    None => None,
                }
            }
        };
        let base = self.objects.len();
        for (i, obj) in subtree.iter().enumerate() {
            let mut obj = obj.clone();
            match &mut obj {
                LevelObject::Prefab(p) => {
                    p.parent = p.parent.map(|pi| pi + base);
                }
                LevelObject::Parent(p) => {
                    p.parent = p.parent.map(|pi| pi + base);
                    p.children = p.children.iter().map(|&c| c + base).collect();
                }
            }
            // The root of the pasted subtree: set its parent.
            if i == 0 {
                match &mut obj {
                    LevelObject::Prefab(p) => p.parent = root_parent,
                    LevelObject::Parent(p) => p.parent = root_parent,
                }
            }
            self.objects.push(obj);
        }
        match paste_position {
            PastePosition::AppendTo(Some(parent_idx)) => {
                if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(parent_idx) {
                    parent.children.push(base);
                } else {
                    self.roots.push(base);
                }
            }
            PastePosition::AppendTo(None) => {
                self.roots.push(base);
            }
            PastePosition::Exact(DropPosition::IntoParent(target)) => {
                if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(target) {
                    parent.children.push(base);
                } else {
                    self.roots.push(base);
                }
            }
            PastePosition::Exact(DropPosition::Before(target))
            | PastePosition::Exact(DropPosition::After(target)) => {
                let is_before = matches!(paste_position, PastePosition::Exact(DropPosition::Before(_)));
                let target_parent = match self.objects.get(target) {
                    Some(LevelObject::Prefab(prefab)) => prefab.parent,
                    Some(LevelObject::Parent(parent)) => parent.parent,
                    None => None,
                };
                if let Some(parent_idx) = target_parent {
                    if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(parent_idx) {
                        let pos = parent.children.iter().position(|&child| child == target);
                        let insert_at = pos.map_or(parent.children.len(), |index| {
                            if is_before { index } else { index + 1 }
                        });
                        parent.children.insert(insert_at, base);
                    } else {
                        self.roots.push(base);
                    }
                } else {
                    let pos = self.roots.iter().position(|&root| root == target);
                    let insert_at = pos.map_or(self.roots.len(), |index| {
                        if is_before { index } else { index + 1 }
                    });
                    self.roots.insert(insert_at, base);
                }
            }
        }
        base
    }
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        configure_cjk_fonts(&cc.egui_ctx);

        // Disable debug visualisations that cause red-frame flicker in dev builds
        #[cfg(debug_assertions)]
        cc.egui_ctx.global_style_mut(|s| {
            s.debug.show_unaligned = false;
            s.debug.warn_if_rect_changes_id = false;
        });

        #[cfg(not(target_arch = "wasm32"))]
        let mut renderer = LevelRenderer::new(cc.wgpu_render_state.as_ref());
        #[cfg(target_arch = "wasm32")]
        let renderer = LevelRenderer::new(cc.wgpu_render_state.as_ref());

        let gpu_backend = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| rs.adapter.get_info().backend.to_string())
            .unwrap_or_default();

        // Auto-detect asset base directory relative to the executable
        #[cfg(not(target_arch = "wasm32"))]
        {
            let asset_base = Self::detect_asset_base();
            if let Some(base) = asset_base {
                renderer.asset_base = Some(base);
            }
        }

        let lang = Language::from_system();
        let initial_tab = Tab::new(renderer, lang.i18n().get("status_welcome"));

        Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            show_add_dialog: false,
            add_obj_is_parent: false,
            add_obj_data_type: DataType::None,
            add_obj_name: String::new(),
            add_obj_prefab_index: 0,
            add_obj_position: Vec3::default(),
            add_obj_rotation: Vec3::default(),
            add_obj_scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            #[cfg(target_arch = "wasm32")]
            pending_file: None,
            show_object_tree: true,
            show_properties: true,
            show_shortcuts: false,
            show_about: false,
            clipboard: None,
            gpu_backend,
            lang,
            cursor_mode: CursorMode::default(),
            show_tools: false,
        }
    }

    pub(super) fn prepare_add_object_dialog(&mut self) {
        self.prepare_add_object_dialog_at(None);
    }

    fn toggle_active_terrain_preset(&mut self, shape: TerrainPresetShape) {
        if self.tabs[self.active_tab].is_save_tab() {
            return;
        }

        self.tabs[self.active_tab].renderer.toggle_terrain_preset(shape);
    }

    pub(super) fn prepare_add_object_dialog_at(&mut self, world_pos: Option<Vec2>) {
        self.add_obj_is_parent = false;
        self.add_obj_data_type = DataType::None;
        self.add_obj_name.clear();
        self.add_obj_prefab_index = self.next_add_prefab_index();
        self.add_obj_position = world_pos.map_or_else(Vec3::default, |pos| Vec3 {
            x: pos.x,
            y: pos.y,
            z: 0.0,
        });
        self.add_obj_rotation = Vec3::default();
        self.add_obj_scale = Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        self.show_add_dialog = true;
    }

    fn next_prefab_index_for_level(level: &LevelData) -> i16 {
        level
            .objects
            .iter()
            .filter_map(|object| match object {
                LevelObject::Prefab(prefab) if prefab.prefab_index >= 0 => Some(prefab.prefab_index),
                _ => None,
            })
            .max()
            .and_then(|index| index.checked_add(1))
            .unwrap_or(0)
    }

    fn next_add_prefab_index(&self) -> i16 {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.level.as_ref())
            .map(Self::next_prefab_index_for_level)
            .unwrap_or(0)
    }

    /// Returns the current language's translations.
    fn t(&self) -> &'static I18n {
        self.lang.i18n()
    }

    /// Detect the asset base directory by searching upward from the executable.
    #[cfg(not(target_arch = "wasm32"))]
    fn detect_asset_base() -> Option<String> {
        // First try ASSET_BASE env var
        if let Ok(val) = std::env::var("ASSET_BASE") {
            let p = std::path::Path::new(&val);
            if p.is_dir() {
                return Some(val);
            }
        }
        // Walk up from the executable to find assets/ or level-editor/public/assets
        let exe = std::env::current_exe().ok()?;
        let mut dir = exe.parent()?;
        for _ in 0..6 {
            // Prefer local assets/ directory (bundled with the editor)
            let local = dir.join("assets");
            if local.join("sprites").is_dir() {
                return Some(local.to_string_lossy().into_owned());
            }
            let candidate = dir.join("level-editor/public/assets");
            if candidate.is_dir() {
                return Some(candidate.to_string_lossy().into_owned());
            }
            dir = dir.parent()?;
        }
        None
    }

    /// Handle WASM pending file and native drag-and-drop file input.
    fn handle_file_input(&mut self, _ui: &mut egui::Ui, ctx: &egui::Context) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some((name, data)) = WASM_OPEN_RESULT.with(|q| q.borrow_mut().take()) {
                self.pending_file = Some((name, data));
            }
            if let Some((name, data)) = WASM_OPEN_XML_SAVE.with(|q| q.borrow_mut().take()) {
                self.load_xml_into_tab(name, data);
            }
            if let Some((name, data)) = self.pending_file.take() {
                if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".toml") {
                    if let Ok(text) = String::from_utf8(data) {
                        self.load_level_text_into_tab(name, &text, None);
                    } else {
                        self.tabs[self.active_tab].status = "UTF-8 解码失败".to_string();
                    }
                } else {
                    self.load_level_into_tab(name, data, None);
                }
            }
        }

        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                let file_data: Option<(String, Vec<u8>, Option<String>)> = if let Some(ref bytes) = file.bytes {
                    Some((file.name.clone(), bytes.to_vec(), None))
                } else if let Some(ref path) = file.path {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        std::fs::read(path).ok().map(|data| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| file.name.clone());
                            (name, data, Some(path.to_string_lossy().into_owned()))
                        })
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        let _ = path;
                        None
                    }
                } else {
                    None
                };

                if let Some((name, data, source_path)) = file_data {
                    if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".toml")
                    {
                        match String::from_utf8(data) {
                            Ok(text) => self.load_level_text_into_tab(name, &text, source_path),
                            Err(_) => {
                                self.tabs[self.active_tab].status = "UTF-8 解码失败".to_string()
                            }
                        }
                    } else {
                        self.load_level_into_tab(name, data, source_path);
                    }
                }
            }
        });
    }
}

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

        // Update OS window title with GPU backend info
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

/// Render the tab bar with drag-and-drop reordering.
impl EditorApp {
    fn render_tab_bar(&mut self, ui: &mut egui::Ui, t: &'static I18n, ctx: &egui::Context) {
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

    /// Render the central canvas panel.
    fn render_canvas(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let cursor_mode = self.cursor_mode;
        let active_tab = self.active_tab;
        let has_clipboard = self.clipboard.is_some();
        if let Some(ref mut sv) = self.tabs[active_tab].save_view {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                sv.render_save_panels(ui, t);
            });
        } else {
            let mut canvas_context_action = None;
            let mut canvas_context_selected_object = None;
            egui::CentralPanel::default().show_inside(ui, |ui| {
                let tab = &mut self.tabs[active_tab];
                if tab.level.is_some() {
                    let sel = tab.selected.clone();
                    tab.renderer.show(ui, &sel, cursor_mode, t, has_clipboard);
                    canvas_context_action = tab.renderer.context_action.take();
                    canvas_context_selected_object = tab.renderer.context_selected_object.take();
                    // Pick up click-to-select from renderer
                    if let Some(idx) = canvas_context_selected_object {
                        tab.selected = BTreeSet::from([idx]);
                    }
                    if let Some(idx) = tab.renderer.clicked_object {
                        if tab.renderer.clicked_with_cmd {
                            if tab.selected.contains(&idx) {
                                tab.selected.remove(&idx);
                            } else {
                                tab.selected.insert(idx);
                            }
                        } else {
                            tab.selected = BTreeSet::from([idx]);
                        }
                    } else if tab.renderer.clicked_empty && !tab.renderer.clicked_with_cmd {
                        tab.selected.clear();
                    }
                    // Pick up drag result — update object position
                    if let Some((idx, delta)) = tab.renderer.drag_result.take()
                        && let Some(ref mut level) = tab.level
                        && idx < level.objects.len()
                    {
                        tab.history.undo.push(Snapshot {
                            level: level.clone(),
                            selected: tab.selected.clone(),
                        });
                        if tab.history.undo.len() > UNDO_MAX {
                            tab.history.undo.remove(0);
                        }
                        tab.history.redo.clear();

                        for &sel_idx in &tab.selected {
                            if sel_idx < level.objects.len() {
                                match &mut level.objects[sel_idx] {
                                    LevelObject::Prefab(p) => {
                                        p.position.x += delta.x;
                                        p.position.y += delta.y;
                                    }
                                    LevelObject::Parent(p) => {
                                        p.position.x += delta.x;
                                        p.position.y += delta.y;
                                    }
                                }
                            }
                        }
                        if !tab.selected.contains(&idx) {
                            match &mut level.objects[idx] {
                                LevelObject::Prefab(p) => {
                                    p.position.x += delta.x;
                                    p.position.y += delta.y;
                                }
                                LevelObject::Parent(p) => {
                                    p.position.x += delta.x;
                                    p.position.y += delta.y;
                                }
                            }
                        }
                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                    }
                    // Pick up terrain node drag result
                    if let Some(result) = tab.renderer.node_drag_result.take()
                        && let Some(ref mut level) = tab.level
                        && result.object_index < level.objects.len()
                    {
                        tab.history.undo.push(Snapshot {
                            level: level.clone(),
                            selected: tab.selected.clone(),
                        });
                        if tab.history.undo.len() > UNDO_MAX {
                            tab.history.undo.remove(0);
                        }
                        tab.history.redo.clear();

                        if let LevelObject::Prefab(ref mut p) = level.objects[result.object_index]
                            && let Some(ref mut td) = p.terrain_data
                        {
                            let mut nodes = crate::terrain_gen::extract_curve_nodes(td);
                            if result.node_index < nodes.len() {
                                nodes[result.node_index].position = result.new_local_pos;
                                crate::terrain_gen::regenerate_terrain(td, &nodes);
                            }
                        }

                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                    }
                    // Pick up terrain node edit action (add / delete)
                    if let Some(action) = tab.renderer.node_edit_action.take()
                        && let Some(ref mut level) = tab.level
                    {
                        use crate::renderer::NodeEditAction;
                        let obj_idx = match &action {
                            NodeEditAction::Delete { object_index, .. }
                            | NodeEditAction::Insert { object_index, .. }
                            | NodeEditAction::ToggleTexture { object_index, .. } => *object_index,
                        };
                        if obj_idx < level.objects.len() {
                            tab.history.undo.push(Snapshot {
                                level: level.clone(),
                                selected: tab.selected.clone(),
                            });
                            if tab.history.undo.len() > UNDO_MAX {
                                tab.history.undo.remove(0);
                            }
                            tab.history.redo.clear();

                            if let LevelObject::Prefab(ref mut p) = level.objects[obj_idx]
                                && let Some(ref mut td) = p.terrain_data
                            {
                                let mut nodes = crate::terrain_gen::extract_curve_nodes(td);
                                match action {
                                    NodeEditAction::Delete { node_index, .. } => {
                                        if node_index < nodes.len() && nodes.len() > 2 {
                                            nodes.remove(node_index);
                                        }
                                    }
                                    NodeEditAction::Insert {
                                        after_node,
                                        local_pos,
                                        ..
                                    } => {
                                        let insert_idx = (after_node + 1).min(nodes.len());
                                        let tex =
                                            nodes.get(after_node).map(|n| n.texture).unwrap_or(0);
                                        nodes.insert(
                                            insert_idx,
                                            crate::terrain_gen::CurveNode {
                                                position: local_pos,
                                                texture: tex,
                                            },
                                        );
                                    }
                                    NodeEditAction::ToggleTexture { node_index, .. } => {
                                        if let Some(node) = nodes.get_mut(node_index) {
                                            node.texture = if node.texture == 0 { 1 } else { 0 };
                                        }
                                    }
                                }
                                crate::terrain_gen::regenerate_terrain(td, &nodes);
                            }

                            let cam = tab.renderer.camera.clone();
                            tab.renderer.set_level(level);
                            tab.renderer.camera = cam;
                        }
                    }
                    // Pick up box-selection result — replace selection
                    if let Some(result) = tab.renderer.box_select_result.take() {
                        tab.selected = result.indices;
                    }
                    // Pick up bounds drag result — write back to LevelManager override data
                    if let Some(result) = tab.renderer.bounds_drag_result.take()
                        && let Some(ref mut level) = tab.level
                    {
                        tab.history.undo.push(Snapshot {
                            level: level.clone(),
                            selected: tab.selected.clone(),
                        });
                        if tab.history.undo.len() > UNDO_MAX {
                            tab.history.undo.remove(0);
                        }
                        tab.history.redo.clear();
                        dialogs::update_camera_limits_in_level(level, result.new_limits);
                    }
                    // Pick up terrain draw result — create new terrain object
                    if let Some(result) = tab.renderer.draw_terrain_result.take()
                        && let Some(ref mut level) = tab.level
                        && result.points.len() >= 2
                    {
                        tab.history.undo.push(Snapshot {
                            level: level.clone(),
                            selected: tab.selected.clone(),
                        });
                        if tab.history.undo.len() > UNDO_MAX {
                            tab.history.undo.remove(0);
                        }
                        tab.history.redo.clear();

                        // Create a new terrain Prefab from drawn points
                        let center = {
                            let mut cx = 0.0f32;
                            let mut cy = 0.0f32;
                            for p in &result.points {
                                cx += p.x;
                                cy += p.y;
                            }
                            let n = result.points.len() as f32;
                            Vec2 {
                                x: cx / n,
                                y: cy / n,
                            }
                        };
                        // Default texture = 1 (splat1)
                        let local_nodes: Vec<crate::terrain_gen::CurveNode> = result
                            .points
                            .iter()
                            .map(|p| crate::terrain_gen::CurveNode {
                                position: Vec2 {
                                    x: p.x - center.x,
                                    y: p.y - center.y,
                                },
                                texture: 1,
                            })
                            .collect();
                        let mut td = TerrainData {
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
                                    size: Vec2 { x: 1.0, y: 0.5 },
                                    fixed_angle: false,
                                    fade_threshold: 0.0,
                                },
                                CurveTexture {
                                    texture_index: 1,
                                    size: Vec2 { x: 1.0, y: 0.1 },
                                    fixed_angle: false,
                                    fade_threshold: 0.0,
                                },
                            ],
                            control_texture_count: 0,
                            control_texture_data: None,
                            has_collider: true,
                            fill_boundary: None,
                        };
                        crate::terrain_gen::regenerate_terrain(&mut td, &local_nodes);
                        let prefab_index = Self::next_prefab_index_for_level(level);
                        let new_obj = LevelObject::Prefab(PrefabInstance {
                            name: "e2dTerrainBase".to_string(),
                            prefab_index,
                            position: Vec3 {
                                x: center.x,
                                y: center.y,
                                z: 0.0,
                            },
                            rotation: Vec3::default(),
                            scale: Vec3 {
                                x: 1.0,
                                y: 1.0,
                                z: 1.0,
                            },
                            parent: None,
                            data_type: DataType::Terrain,
                            terrain_data: Some(Box::new(td)),
                            override_data: None,
                        });
                        let new_idx = level.objects.len();
                        level.objects.push(new_obj);
                        level.roots.push(new_idx);
                        tab.selected = std::collections::BTreeSet::from([new_idx]);

                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                        let label = if result.closed {
                            "Terrain (closed)"
                        } else {
                            "Terrain"
                        };
                        tab.status = t.fmt1("status_added", label);
                    }
                } else {
                    let rect = ui.available_rect_before_wrap();
                    let is_dark = ui.visuals().dark_mode;
                    let icon_tint = if is_dark {
                        egui::Color32::from_gray(160)
                    } else {
                        egui::Color32::from_gray(80)
                    };
                    let hint_color = if is_dark {
                        egui::Color32::from_gray(180)
                    } else {
                        egui::Color32::from_gray(100)
                    };
                    let sub_color = if is_dark {
                        egui::Color32::from_gray(140)
                    } else {
                        egui::Color32::from_gray(120)
                    };
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.with_layout(
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| {
                                ui.vertical_centered(|ui| {
                                    let center_y = rect.center().y - 40.0;
                                    ui.add_space((center_y - rect.top()).max(0.0));

                                    ui.add(
                                        egui::Image::from_bytes(
                                            "bytes://drop-icon.svg",
                                            include_bytes!("../../assets/drop-icon.svg"),
                                        )
                                        .fit_to_exact_size(egui::Vec2::splat(48.0))
                                        .tint(icon_tint),
                                    );

                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(t.get("panel_drop_hint"))
                                            .color(hint_color),
                                    );
                                    ui.label(
                                        egui::RichText::new(t.get("panel_open_hint"))
                                            .color(sub_color),
                                    );
                                });
                            },
                        );
                    });
                }
            });

            if let Some(action) = canvas_context_action {
                match action {
                    crate::renderer::CanvasContextAction::Copy(indices) => {
                        self.copy_objects(&indices);
                    }
                    crate::renderer::CanvasContextAction::Cut(indices) => {
                        self.cut_objects(&indices);
                    }
                    crate::renderer::CanvasContextAction::AddObject { world_pos } => {
                        self.prepare_add_object_dialog_at(world_pos);
                    }
                    crate::renderer::CanvasContextAction::Paste {
                        context_indices,
                        world_pos,
                    } => {
                        self.paste_with_context(&context_indices, world_pos, None);
                    }
                    crate::renderer::CanvasContextAction::Duplicate(indices) => {
                        self.duplicate_objects(&indices);
                    }
                    crate::renderer::CanvasContextAction::Delete(indices) => {
                        self.request_delete_objects(&indices);
                    }
                }
            }
        }
    }
}

/// Load a system CJK font and register it as a fallback for proportional + monospace.
fn configure_cjk_fonts(ctx: &egui::Context) {
    let Some(data) = load_system_cjk_font() else {
        log::warn!("No system CJK font found — Chinese text will render as squares");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".into(),
        std::sync::Arc::new(egui::FontData::from_owned(data)),
    );
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("cjk".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("cjk".into());
    }

    ctx.set_fonts(fonts);
}

/// Convert days since Unix epoch to (year, month, day).
#[cfg(not(target_arch = "wasm32"))]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            log::info!("Loaded CJK font: {}", path);
            return Some(data);
        }
    }
    Some(include_bytes!("../../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    Some(include_bytes!("../../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}
