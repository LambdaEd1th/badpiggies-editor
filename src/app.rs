//! egui application — main editor UI with three-panel layout.

use eframe::egui;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::locale::{I18n, Language};
use crate::parser;
use crate::renderer::LevelRenderer;
use crate::types::*;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_OPEN_RESULT: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
}

/// Maximum number of undo snapshots to keep.
const UNDO_MAX: usize = 100;

/// A snapshot of the editor state for undo/redo.
struct Snapshot {
    level: LevelData,
    selected: Option<ObjectIndex>,
}

/// Undo/redo history stack.
struct UndoStack {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

/// Clipboard contents for copy/cut/paste.
#[derive(Clone)]
struct Clipboard {
    /// Cloned objects forming a self-contained subtree.
    /// objects[0] is always the root of the copied subtree.
    objects: Vec<LevelObject>,
}

/// Per-tab editor state (each tab is an independent level editor).
struct Tab {
    /// Currently loaded level data.
    level: Option<LevelData>,
    /// File name of the loaded level.
    file_name: Option<String>,
    /// Currently selected object index.
    selected: Option<ObjectIndex>,
    /// Canvas renderer state (owns per-level caches; shares GPU pipelines via Arc).
    renderer: LevelRenderer,
    /// Status message.
    status: String,
    /// Pending delete confirmation: (object_index, object_name).
    pending_delete: Option<(ObjectIndex, String)>,
    /// Undo/redo history.
    history: UndoStack,
    /// Whether properties were changed in the previous frame (for undo coalescing).
    props_changed_prev: bool,
}

impl Tab {
    fn new(renderer: LevelRenderer, welcome_status: String) -> Self {
        Self {
            level: None,
            file_name: None,
            selected: None,
            renderer,
            status: welcome_status,
            pending_delete: None,
            history: UndoStack {
                undo: Vec::new(),
                redo: Vec::new(),
            },
            props_changed_prev: false,
        }
    }

    /// Display name for the tab.
    fn title(&self) -> String {
        self.file_name
            .clone()
            .unwrap_or_else(|| "(untitled)".to_string())
    }

    fn load_level(&mut self, name: String, data: Vec<u8>, i18n: &I18n) {
        match parser::parse_level(data) {
            Ok(level) => {
                let obj_count = level.objects.len();
                let root_count = level.roots.len();
                self.renderer.set_level_key(&name);
                self.renderer.set_level(&level);
                self.level = Some(level);
                self.file_name = Some(name);
                self.selected = None;
                self.history.undo.clear();
                self.history.redo.clear();
                self.status = i18n.fmt_status_loaded(obj_count, root_count);
            }
            Err(e) => {
                self.status = format!("解析失败: {}", e);
            }
        }
    }

    fn load_level_text(&mut self, name: String, text: &str, i18n: &I18n) {
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
                self.selected = None;
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
    fn paste_target_parent(
        level: &LevelData,
        selected: Option<ObjectIndex>,
    ) -> Option<ObjectIndex> {
        let sel = selected?;
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
    add_obj_name: String,
    add_obj_prefab_index: i16,
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
}

impl LevelData {
    /// Deep-clone the subtree rooted at `root_idx` into a self-contained
    /// `Clipboard`.  All internal parent/children indices are remapped to
    /// be relative to the cloned vec (root is always index 0).
    fn clone_subtree(&self, root_idx: ObjectIndex) -> Clipboard {
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
        let objects: Vec<LevelObject> = indices
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
            .collect();
        Clipboard { objects }
    }

    /// Paste a clipboard subtree into the level.  All objects are appended to
    /// the arena.  If `parent_idx` is `Some`, the pasted root becomes a child
    /// of that parent; otherwise it becomes a new root-level object.
    /// Returns the index of the pasted root.
    fn paste_subtree(&mut self, clip: &Clipboard, parent_idx: Option<ObjectIndex>) -> ObjectIndex {
        let base = self.objects.len();
        for (i, obj) in clip.objects.iter().enumerate() {
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
                    LevelObject::Prefab(p) => p.parent = parent_idx,
                    LevelObject::Parent(p) => p.parent = parent_idx,
                }
            }
            self.objects.push(obj);
        }
        if let Some(pi) = parent_idx {
            // Add the pasted root as a child of the target parent.
            if let LevelObject::Parent(p) = &mut self.objects[pi] {
                p.children.push(base);
            }
        } else {
            self.roots.push(base);
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
            add_obj_name: String::new(),
            add_obj_prefab_index: 0,
            #[cfg(target_arch = "wasm32")]
            pending_file: None,
            show_object_tree: true,
            show_properties: true,
            show_shortcuts: false,
            show_about: false,
            clipboard: None,
            gpu_backend,
            lang,
        }
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

    /// Snapshot current state onto the undo stack (call before mutation).
    fn push_undo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(ref level) = tab.level {
            tab.history.undo.push(Snapshot {
                level: level.clone(),
                selected: tab.selected,
            });
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
        }
    }

    /// Undo the last change.
    fn undo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(snapshot) = tab.history.undo.pop() {
            if let Some(ref level) = tab.level {
                tab.history.redo.push(Snapshot {
                    level: level.clone(),
                    selected: tab.selected,
                });
            }
            tab.selected = snapshot.selected;
            let cam = tab.renderer.camera.clone();
            tab.renderer.set_level(&snapshot.level);
            tab.renderer.camera = cam;
            tab.level = Some(snapshot.level);
        }
    }

    /// Redo the last undone change.
    fn redo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(snapshot) = tab.history.redo.pop() {
            if let Some(ref level) = tab.level {
                tab.history.undo.push(Snapshot {
                    level: level.clone(),
                    selected: tab.selected,
                });
            }
            tab.selected = snapshot.selected;
            let cam = tab.renderer.camera.clone();
            tab.renderer.set_level(&snapshot.level);
            tab.renderer.camera = cam;
            tab.level = Some(snapshot.level);
        }
    }

    /// Copy the selected object (and its subtree) to the clipboard.
    fn copy_selected(&mut self) {
        let tab = &self.tabs[self.active_tab];
        if let Some(sel) = tab.selected
            && let Some(ref level) = tab.level
            && sel < level.objects.len()
        {
            self.clipboard = Some(level.clone_subtree(sel));
        }
    }

    /// Cut the selected object: copy then delete.
    fn cut_selected(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(sel) = tab.selected
            && let Some(ref level) = tab.level
            && sel < level.objects.len()
        {
            self.clipboard = Some(level.clone_subtree(sel));
            // push_undo inline (can't call self method while borrowing tab)
            let undo_snap = Snapshot {
                level: level.clone(),
                selected: tab.selected,
            };
            tab.history.undo.push(undo_snap);
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
            if let Some(ref mut level) = tab.level {
                level.delete_object(sel);
                tab.selected = None;
                let cam = tab.renderer.camera.clone();
                tab.renderer.set_level(level);
                tab.renderer.camera = cam;
            }
        }
    }

    /// Paste from the clipboard, offset slightly from the original position.
    fn paste(&mut self) {
        let clip = match self.clipboard.clone() {
            Some(c) => c,
            None => return,
        };
        let tab = &mut self.tabs[self.active_tab];
        if tab.level.is_none() {
            return;
        }
        let target = Tab::paste_target_parent(tab.level.as_ref().unwrap(), tab.selected);
        // push_undo inline
        {
            let level = tab.level.as_ref().unwrap();
            tab.history.undo.push(Snapshot {
                level: level.clone(),
                selected: tab.selected,
            });
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
        }
        let level = tab.level.as_mut().unwrap();
        let new_root = level.paste_subtree(&clip, target);
        match &mut level.objects[new_root] {
            LevelObject::Prefab(p) => {
                p.position.x += 1.0;
                p.position.y -= 1.0;
            }
            LevelObject::Parent(p) => {
                p.position.x += 1.0;
                p.position.y -= 1.0;
            }
        }
        tab.selected = Some(new_root);
        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
    }

    /// Duplicate the selected object in-place (copy + paste in one step).
    fn duplicate_selected(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        let sel = match tab.selected {
            Some(s) => s,
            None => return,
        };
        if tab.level.is_none() {
            return;
        }
        let level_ref = tab.level.as_ref().unwrap();
        let clip = level_ref.clone_subtree(sel);
        let target = match &level_ref.objects[sel] {
            LevelObject::Prefab(p) => p.parent,
            LevelObject::Parent(p) => p.parent,
        };
        // push_undo inline
        tab.history.undo.push(Snapshot {
            level: level_ref.clone(),
            selected: tab.selected,
        });
        if tab.history.undo.len() > UNDO_MAX {
            tab.history.undo.remove(0);
        }
        tab.history.redo.clear();
        let level = tab.level.as_mut().unwrap();
        let new_root = level.paste_subtree(&clip, target);
        match &mut level.objects[new_root] {
            LevelObject::Prefab(p) => {
                p.position.x += 1.0;
                p.position.y -= 1.0;
            }
            LevelObject::Parent(p) => {
                p.position.x += 1.0;
                p.position.y -= 1.0;
            }
        }
        tab.selected = Some(new_root);
        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
    }

    /// Load a level into the active tab (or a new tab if active tab already has a level).
    fn load_level_into_tab(&mut self, name: String, data: Vec<u8>) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() {
            // Active tab has a level — open in new tab
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level(name, data, i18n);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            // Active tab is empty — load here
            self.tabs[self.active_tab].load_level(name, data, i18n);
        }
    }

    /// Load a text-format level into the active tab (or new tab).
    fn load_level_text_into_tab(&mut self, name: String, text: &str) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level_text(name, text, i18n);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs[self.active_tab].load_level_text(name, text, i18n);
        }
    }

    /// Close tab at index. Returns false if last tab was closed (app keeps at least 1 tab).
    fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            // Last tab — just clear it
            let tab = &mut self.tabs[0];
            tab.level = None;
            tab.file_name = None;
            tab.selected = None;
            tab.history.undo.clear();
            tab.history.redo.clear();
            tab.status = self.lang.i18n().get("status_welcome");
            return;
        }
        self.tabs.remove(idx);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > idx {
            self.active_tab -= 1;
        }
    }
}

impl eframe::App for EditorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        use egui::{Key, KeyboardShortcut, Modifiers};

        // Handle keyboard shortcuts in logic() — runs before ui() and before
        // any widgets can consume the events. Uses consume_shortcut/consume_key
        // to prevent TextEdit widgets from also handling these keys.

        // B key — toggle background visibility (only when no text widget has focus)
        if !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::B))
        {
            self.tabs[self.active_tab].renderer.show_bg =
                !self.tabs[self.active_tab].renderer.show_bg;
        }

        // Cmd+Shift+Z / Ctrl+Shift+Z — redo (check before Cmd+Z to match most specific first)
        if ctx.input_mut(|i| {
            i.consume_shortcut(&KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Z,
            ))
        }) {
            self.redo();
        }
        // Cmd+Z / Ctrl+Z — undo
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)))
        {
            self.undo();
        }
        // Ctrl+Y — redo (alternative)
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y)))
        {
            self.redo();
        }

        // Cmd+C / Ctrl+C — copy
        // egui-winit converts Cmd+C into Event::Copy (not Event::Key), so we must
        // consume Event::Copy instead of using consume_shortcut.
        if ctx.input_mut(|i| {
            let mut found = false;
            i.events.retain(|e| {
                if matches!(e, egui::Event::Copy) && !found {
                    found = true;
                    false // remove from queue
                } else {
                    true
                }
            });
            found
        }) {
            self.copy_selected();
        }
        // Cmd+X / Ctrl+X — cut (egui-winit converts to Event::Cut)
        if ctx.input_mut(|i| {
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
        }) {
            self.cut_selected();
        }
        // Cmd+V / Ctrl+V — paste (egui-winit converts to Event::Paste)
        if ctx.input_mut(|i| {
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
        }) {
            self.paste();
        }
        // Cmd+D / Ctrl+D — duplicate
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::D)))
        {
            self.duplicate_selected();
        }

        // Cmd+W / Ctrl+W — close tab (only when multiple tabs open)
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::W)))
            && self.tabs.len() > 1
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
        // Only when no text widget has focus (avoid intercepting text editing)
        // Skip if hovering a terrain node (renderer handles node deletion instead)
        if let Some(sel) = self.tabs[self.active_tab].selected {
            let delete_pressed = !ctx.egui_wants_keyboard_input()
                && ctx.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Delete)
                        || i.consume_key(Modifiers::NONE, Key::Backspace)
                });
            if delete_pressed
                && self.tabs[self.active_tab].renderer.hovered_terrain_node.is_none()
                && self.tabs[self.active_tab].pending_delete.is_none()
                && let Some(ref level) = self.tabs[self.active_tab].level
            {
                let name = level.objects[sel].name().to_string();
                self.tabs[self.active_tab].pending_delete = Some((sel, name));
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let t = self.t();

        #[cfg(target_arch = "wasm32")]
        {
            if let Some((name, data)) = WASM_OPEN_RESULT.with(|q| q.borrow_mut().take()) {
                self.pending_file = Some((name, data));
            }
            if let Some((name, data)) = self.pending_file.take() {
                if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".toml") {
                    if let Ok(text) = String::from_utf8(data) {
                        self.load_level_text_into_tab(name, &text);
                    } else {
                        self.tabs[self.active_tab].status = "UTF-8 解码失败".to_string();
                    }
                } else {
                    self.load_level_into_tab(name, data);
                }
            }
        }

        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                // On WASM, bytes is populated; on native, path is populated
                let file_data: Option<(String, Vec<u8>)> = if let Some(ref bytes) = file.bytes {
                    Some((file.name.clone(), bytes.to_vec()))
                } else if let Some(ref path) = file.path {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        std::fs::read(path).ok().map(|data| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| file.name.clone());
                            (name, data)
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

                if let Some((name, data)) = file_data {
                    if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".toml")
                    {
                        match String::from_utf8(data) {
                            Ok(text) => self.load_level_text_into_tab(name, &text),
                            Err(_) => {
                                self.tabs[self.active_tab].status = "UTF-8 解码失败".to_string()
                            }
                        }
                    } else {
                        self.load_level_into_tab(name, data);
                    }
                }
            }
        });

        // Delete confirmation dialog
        if let Some((del_idx, ref del_name)) = self.tabs[self.active_tab].pending_delete.clone() {
            let mut action = 0u8; // 0=pending, 1=confirm, 2=cancel
            egui::Window::new(t.get("win_confirm_delete"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(&ctx, |ui| {
                    ui.label(t.fmt1("status_delete_confirm", del_name));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(t.get("btn_ok")).clicked() {
                            action = 1;
                        }
                        if ui.button(t.get("btn_cancel")).clicked() {
                            action = 2;
                        }
                    });
                });
            match action {
                1 => {
                    self.push_undo();
                    let tab = &mut self.tabs[self.active_tab];
                    if let Some(ref mut level) = tab.level {
                        level.delete_object(del_idx);
                        tab.selected = None;
                        let cam = tab.renderer.camera.clone();
                        tab.renderer.set_level(level);
                        tab.renderer.camera = cam;
                        tab.status = format!("已删除: {}", del_name);
                    }
                    tab.pending_delete = None;
                }
                2 => {
                    self.tabs[self.active_tab].pending_delete = None;
                }
                _ => {}
            }
        }

        // ── Top menu bar ──
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(t.get("menu_file"), |ui| {
                    if ui.button(t.get("menu_open_level")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Level files", &["bytes"])
                                .pick_file()
                            {
                                match std::fs::read(&path) {
                                    Ok(data) => {
                                        let name = path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().into_owned())
                                            .unwrap_or_default();
                                        self.load_level_into_tab(name, data);
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_read_error", &e.to_string());
                                    }
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            let repaint_ctx = ctx.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Some(file) = rfd::AsyncFileDialog::new()
                                    .add_filter("Level files", &["bytes"])
                                    .pick_file()
                                    .await
                                {
                                    let name = file.file_name();
                                    let data = file.read().await;
                                    WASM_OPEN_RESULT.with(|q| {
                                        q.borrow_mut().replace((name, data));
                                    });
                                    repaint_ctx.request_repaint();
                                }
                            });
                        }
                    }
                    if ui.button(t.get("menu_import_text")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("YAML / TOML", &["yaml", "yml", "toml"])
                                .pick_file()
                            {
                                let name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                match std::fs::read_to_string(&path) {
                                    Ok(text) => {
                                        self.load_level_text_into_tab(name, &text);
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_read_error", &e.to_string());
                                    }
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            let repaint_ctx = ctx.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Some(file) = rfd::AsyncFileDialog::new()
                                    .add_filter("YAML / TOML", &["yaml", "yml", "toml"])
                                    .pick_file()
                                    .await
                                {
                                    let name = file.file_name();
                                    let data = file.read().await;
                                    WASM_OPEN_RESULT.with(|q| {
                                        q.borrow_mut().replace((name, data));
                                    });
                                    repaint_ctx.request_repaint();
                                }
                            });
                        }
                    }
                    ui.separator();
                    if ui.button(t.get("menu_export_level")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let default_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .unwrap_or("level.bytes");
                            if let Some(data) = self.tabs[self.active_tab].export_level()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Level files", &["bytes"])
                                    .set_file_name(default_name)
                                    .save_file()
                            {
                                match std::fs::write(&path, data) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e.to_string());
                                    }
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(data) = self.tabs[self.active_tab].export_level() {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .clone()
                                    .unwrap_or_else(|| "level.bytes".to_string());
                                match export_bytes_wasm(&file_name, data) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e);
                                    }
                                }
                            }
                        }
                    }
                    if ui.button(t.get("menu_export_yaml")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let yaml_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .map(|n| format!("{n}.yaml"))
                                .unwrap_or_else(|| "level.yaml".into());
                            if let Some(text) = self.tabs[self.active_tab].export_yaml()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("YAML files", &["yaml"])
                                    .set_file_name(&yaml_name)
                                    .save_file()
                            {
                                match std::fs::write(&path, text.as_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e.to_string());
                                    }
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(text) = self.tabs[self.active_tab].export_yaml() {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .as_deref()
                                    .map(|n| format!("{n}.yaml"))
                                    .unwrap_or_else(|| "level.yaml".to_string());
                                match export_bytes_wasm(&file_name, text.into_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e);
                                    }
                                }
                            }
                        }
                    }
                    if ui.button(t.get("menu_export_toml")).clicked() {
                        ui.close();
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let toml_name = self.tabs[self.active_tab]
                                .file_name
                                .as_deref()
                                .map(|n| format!("{n}.toml"))
                                .unwrap_or_else(|| "level.toml".into());
                            if let Some(text) = self.tabs[self.active_tab].export_toml()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("TOML files", &["toml"])
                                    .set_file_name(&toml_name)
                                    .save_file()
                            {
                                match std::fs::write(&path, text.as_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e.to_string());
                                    }
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(text) = self.tabs[self.active_tab].export_toml() {
                                let file_name = self.tabs[self.active_tab]
                                    .file_name
                                    .as_deref()
                                    .map(|n| format!("{n}.toml"))
                                    .unwrap_or_else(|| "level.toml".to_string());
                                match export_bytes_wasm(&file_name, text.into_bytes()) {
                                    Ok(()) => {
                                        self.tabs[self.active_tab].status =
                                            t.get("status_exported");
                                    }
                                    Err(e) => {
                                        self.tabs[self.active_tab].status =
                                            t.fmt1("status_export_error", &e);
                                    }
                                }
                            }
                        }
                    }
                });
                ui.menu_button(t.get("menu_edit"), |ui| {
                    ui.set_min_width(120.0);
                    let is_mac = cfg!(target_os = "macos");
                    let undo_shortcut = if is_mac { "⌘+Z" } else { "Ctrl+Z" };
                    let redo_shortcut = if is_mac { "Shift+⌘+Z" } else { "Ctrl+Y" };
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
                    let has_sel = self.tabs[self.active_tab].selected.is_some()
                        && self.tabs[self.active_tab].level.is_some();
                    let has_clip = self.clipboard.is_some()
                        && self.tabs[self.active_tab].level.is_some();
                    let copy_shortcut = if is_mac { "⌘+C" } else { "Ctrl+C" };
                    let cut_shortcut = if is_mac { "⌘+X" } else { "Ctrl+X" };
                    let paste_shortcut = if is_mac { "⌘+V" } else { "Ctrl+V" };
                    let dup_shortcut = if is_mac { "⌘+D" } else { "Ctrl+D" };
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
                        if let Some(sel) = self.tabs[self.active_tab].selected
                            && let Some(ref level) = self.tabs[self.active_tab].level
                        {
                            let name = level.objects[sel].name().to_string();
                            self.tabs[self.active_tab].pending_delete = Some((sel, name));
                        }
                    }
                    ui.separator();
                    if ui.button(t.get("menu_add_object")).clicked() {
                        ui.close();
                        if self.tabs[self.active_tab].level.is_some() {
                            self.add_obj_name = "NewObject".into();
                            self.add_obj_prefab_index = 0;
                            self.add_obj_is_parent = false;
                            self.show_add_dialog = true;
                        }
                    }
                });
                ui.menu_button(t.get("menu_view"), |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    if ui.button(t.get("menu_fit_view")).clicked() {
                        ui.close();
                        self.tabs[self.active_tab].renderer.fit_to_level();
                    }
                    ui.separator();
                    {
                        let mut v = self.show_object_tree;
                        if ui.checkbox(&mut v, t.get("menu_object_list")).clicked() {
                            ui.close();
                            self.show_object_tree = v;
                        }
                    }
                    {
                        let mut v = self.show_properties;
                        if ui.checkbox(&mut v, t.get("menu_properties")).clicked() {
                            ui.close();
                            self.show_properties = v;
                        }
                    }
                    ui.separator();
                    {
                        let mut v = self.tabs[self.active_tab].renderer.show_bg;
                        if ui.checkbox(&mut v, t.get("menu_background")).clicked() {
                            ui.close();
                            self.tabs[self.active_tab].renderer.show_bg = v;
                        }
                    }
                    {
                        let mut v = self.tabs[self.active_tab].renderer.show_grid;
                        if ui.checkbox(&mut v, t.get("menu_grid")).clicked() {
                            ui.close();
                            self.tabs[self.active_tab].renderer.show_grid = v;
                        }
                    }
                    {
                        let mut v = self.tabs[self.active_tab].renderer.show_ground;
                        if ui.checkbox(&mut v, t.get("menu_physics_ground")).clicked() {
                            ui.close();
                            self.tabs[self.active_tab].renderer.show_ground = v;
                        }
                    }
                    {
                        let mut v = self.tabs[self.active_tab].renderer.show_level_bounds;
                        if ui.checkbox(&mut v, t.get("menu_level_bounds")).clicked() {
                            ui.close();
                            self.tabs[self.active_tab].renderer.show_level_bounds = v;
                        }
                    }
                    if self.tabs[self.active_tab].renderer.is_dark_level() {
                        let mut v = self.tabs[self.active_tab].renderer.show_dark_overlay;
                        if ui.checkbox(&mut v, t.get("menu_dark_overlay")).clicked() {
                            ui.close();
                            self.tabs[self.active_tab].renderer.show_dark_overlay = v;
                        }
                    }
                    ui.separator();
                    ui.menu_button(t.get("menu_language"), |ui| {
                        for &lang in Language::ALL {
                            if ui
                                .selectable_label(self.lang == lang, lang.display_name())
                                .clicked()
                            {
                                self.lang = lang;
                                ui.close();
                            }
                        }
                    });
                });
                ui.menu_button(t.get("menu_help"), |ui| {
                    ui.set_min_width(80.0);
                    if ui.button(t.get("menu_export_log")).clicked() {
                        ui.close();
                        let lines = crate::log_buffer::drain();
                        let content = lines.join("\n");
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let log_name = {
                                use std::time::{SystemTime, UNIX_EPOCH};
                                let secs = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                // Convert to local-ish YYYYMMdd_HHmmss (UTC)
                                let s = secs % 60;
                                let m = (secs / 60) % 60;
                                let h = (secs / 3600) % 24;
                                let days = secs / 86400;
                                // Days since 1970-01-01 → y/m/d
                                let (y, mo, d) = civil_from_days(days as i64);
                                format!("{:04}{:02}{:02}_{:02}{:02}{:02}.log", y, mo, d, h, m, s)
                            };
                            if let Some(path) =
                                rfd::FileDialog::new().set_file_name(&log_name).save_file()
                            {
                                if let Err(e) = std::fs::write(&path, &content) {
                                    self.tabs[self.active_tab].status =
                                        format!("Log export error: {e}");
                                } else {
                                    self.tabs[self.active_tab].status =
                                        format!("Log exported: {}", path.display());
                                }
                            }
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Err(e) = export_bytes_wasm("editor.log", content.into_bytes()) {
                                self.tabs[self.active_tab].status =
                                    format!("Log export error: {e}");
                            }
                        }
                    }
                    ui.separator();
                    if ui.button(t.get("menu_shortcuts")).clicked() {
                        ui.close();
                        self.show_shortcuts = true;
                    }
                    if ui.button(t.get("menu_about")).clicked() {
                        ui.close();
                        self.show_about = true;
                    }
                });
            });
        });

        // ── 说明窗口 ──
        if self.show_shortcuts {
            egui::Window::new(t.get("win_shortcuts"))
                .collapsible(false)
                .movable(true)
                .resizable(false)
                .open(&mut self.show_shortcuts)
                .show(&ctx, |ui| {
                    egui::Grid::new("shortcuts_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong(t.get("shortcuts_key"));
                            ui.strong(t.get("shortcuts_action"));
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
                            ui.label(t.get("shortcuts_b_key"));
                            ui.label(t.get("shortcuts_toggle_bg"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_undo"));
                            ui.label(t.get("shortcuts_undo_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_redo"));
                            ui.label(t.get("shortcuts_redo_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_copy"));
                            ui.label(t.get("shortcuts_copy_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_cut"));
                            ui.label(t.get("shortcuts_cut_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_paste"));
                            ui.label(t.get("shortcuts_paste_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_duplicate"));
                            ui.label(t.get("shortcuts_duplicate_action"));
                            ui.end_row();
                            ui.label(t.get("shortcuts_delete"));
                            ui.label(t.get("shortcuts_delete_action"));
                            ui.end_row();
                        });
                });
        }

        // ── 关于窗口 ──
        if self.show_about {
            egui::Window::new(t.get("win_about"))
                .collapsible(false)
                .movable(true)
                .resizable(false)
                .open(&mut self.show_about)
                .show(&ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Bad Piggies Editor");
                        ui.label(format!(
                            "{}{}",
                            t.get("about_version_prefix"),
                            env!("CARGO_PKG_VERSION")
                        ));
                        ui.separator();
                        ui.hyperlink_to(
                            env!("CARGO_PKG_AUTHORS"),
                            "https://space.bilibili.com/8217621",
                        );
                        ui.label(t.get("about_built_with"));
                        ui.label(t.get("about_license"));
                    });
                });
        }

        // ── Add Object Dialog ──
        if self.show_add_dialog {
            let mut open = true;
            egui::Window::new(t.get("win_add_object"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .open(&mut open)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_type"));
                        ui.radio_value(&mut self.add_obj_is_parent, false, "Prefab");
                        ui.radio_value(&mut self.add_obj_is_parent, true, "Parent");
                    });
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_name"));
                        ui.text_edit_singleline(&mut self.add_obj_name);
                    });
                    if !self.add_obj_is_parent {
                        ui.horizontal(|ui| {
                            ui.label(t.get("add_prefab_index"));
                            ui.add(egui::DragValue::new(&mut self.add_obj_prefab_index));
                        });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(t.get("btn_ok")).clicked() {
                            self.push_undo();
                            let add_name = if self.add_obj_name.is_empty() {
                                "NewObject".to_string()
                            } else {
                                self.add_obj_name.clone()
                            };
                            let is_parent = self.add_obj_is_parent;
                            let prefab_index = self.add_obj_prefab_index;
                            let tab = &mut self.tabs[self.active_tab];
                            if let Some(ref mut level) = tab.level {
                                let new_idx = level.objects.len();
                                if is_parent {
                                    level.objects.push(LevelObject::Parent(ParentObject {
                                        name: add_name.clone(),
                                        position: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        children: Vec::new(),
                                        parent: None,
                                    }));
                                } else {
                                    level.objects.push(LevelObject::Prefab(PrefabInstance {
                                        name: add_name.clone(),
                                        position: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        prefab_index,
                                        rotation: Vec3 {
                                            x: 0.0,
                                            y: 0.0,
                                            z: 0.0,
                                        },
                                        scale: Vec3 {
                                            x: 1.0,
                                            y: 1.0,
                                            z: 1.0,
                                        },
                                        data_type: DataType::None,
                                        terrain_data: None,
                                        override_data: None,
                                        parent: None,
                                    }));
                                }
                                level.roots.push(new_idx);
                                tab.selected = Some(new_idx);
                                let cam = tab.renderer.camera.clone();
                                tab.renderer.set_level(level);
                                tab.renderer.camera = cam;
                                tab.status = t.fmt1("status_added", &add_name);
                            }
                            self.show_add_dialog = false;
                        }
                        if ui.button(t.get("btn_cancel")).clicked() {
                            self.show_add_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_add_dialog = false;
            }
        }

        // ── Tab bar ──
        {
            egui::Panel::top("tab_bar").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut close_idx: Option<usize> = None;
                    for i in 0..self.tabs.len() {
                        let title = self.tabs[i].title();
                        let is_active = i == self.active_tab;
                        let resp = ui.selectable_label(is_active, &title);
                        if resp.clicked() {
                            self.active_tab = i;
                        }
                        // "×" close button right after the tab label
                        if self.tabs.len() > 1 {
                            let close_btn = ui.small_button("×");
                            if close_btn.clicked() {
                                close_idx = Some(i);
                            }
                        }
                        ui.add_space(4.0);
                        // Middle-click to close tab
                        if resp.middle_clicked() {
                            close_idx = Some(i);
                        }
                        // Right-click context menu
                        resp.context_menu(|ui| {
                            if ui.button(t.get("menu_close_tab")).clicked() {
                                close_idx = Some(i);
                                ui.close();
                            }
                        });
                    }
                    if let Some(idx) = close_idx {
                        self.close_tab(idx);
                    }
                    // "+" button to add a new empty tab
                    if ui.button("+").clicked() {
                        let new_renderer = self.tabs[self.active_tab].renderer.clone_for_new_tab();
                        let new_tab =
                            Tab::new(new_renderer, self.lang.i18n().get("status_welcome"));
                        self.tabs.push(new_tab);
                        self.active_tab = self.tabs.len() - 1;
                    }
                });
            });
        }

        // ── Status bar ──
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.tabs[self.active_tab].status);
                // Mouse world coordinates
                if let Some(mw) = self.tabs[self.active_tab].renderer.mouse_world {
                    ui.separator();
                    ui.label(format!("X: {:.2}  Y: {:.2}", mw.x, mw.y));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.gpu_backend.is_empty() {
                        ui.label(&self.gpu_backend);
                        ui.separator();
                    }
                    if let Some(ref name) = self.tabs[self.active_tab].file_name {
                        ui.label(name);
                    }
                });
            });
        });

        // ── Left panel: Object tree ──
        if self.show_object_tree {
            egui::Panel::left("object_tree")
                .default_size(240.0)
                .resizable(true)
                .show_inside(ui, |ui| {
                    ui.heading(t.get("panel_object_list"));
                    ui.separator();

                    let mut drop_action: Option<(ObjectIndex, DropPosition)> = None;
                    let mut new_selection = self.tabs[self.active_tab].selected;
                    if let Some(ref level) = self.tabs[self.active_tab].level {
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                for &root_idx in &level.roots {
                                    if let Some(dr) =
                                        show_object_tree(ui, level, root_idx, &mut new_selection, 0)
                                        && drop_action.is_none()
                                    {
                                        drop_action = Some(dr);
                                    }
                                }
                            });
                    }
                    self.tabs[self.active_tab].selected = new_selection;
                    // Handle drop action outside the immutable borrow of level
                    if let Some((source_idx, drop_pos)) = drop_action {
                        self.push_undo();
                        let tab = &mut self.tabs[self.active_tab];
                        if let Some(ref mut level) = tab.level {
                            let new_sel = level.move_object(source_idx, drop_pos);
                            if let Some(ns) = new_sel {
                                tab.selected = Some(ns);
                            }
                            let cam = tab.renderer.camera.clone();
                            tab.renderer.set_level(level);
                            tab.renderer.camera = cam;
                        }
                    }
                });
        }

        // ── Right panel: Properties ──
        if self.show_properties {
            egui::Panel::right("properties")
                .default_size(280.0)
                .size_range(120.0..=f32::INFINITY)
                .resizable(true)
                .show_inside(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    // TextEdits fill available width without forcing the panel
                    // wider — width_range caps the panel size.
                    ui.spacing_mut().text_edit_width = f32::INFINITY;
                    ui.heading(t.get("panel_properties"));
                    ui.separator();

                    let tab = &mut self.tabs[self.active_tab];
                    let sel = tab.selected;
                    if let (Some(level), Some(sel)) = (&mut tab.level, sel) {
                        if sel < level.objects.len() {
                            // Clone the object before editing so we can snapshot pre-state
                            let pre_obj = if !tab.props_changed_prev {
                                Some(level.objects[sel].clone())
                            } else {
                                None
                            };
                            let changed = show_properties_editable(ui, &mut level.objects[sel], t);
                            if changed {
                                if let Some(obj_backup) = pre_obj {
                                    // First change frame — push undo with pre-edit state
                                    let mut level_snapshot = level.clone();
                                    level_snapshot.objects[sel] = obj_backup;
                                    tab.history.undo.push(Snapshot {
                                        level: level_snapshot,
                                        selected: tab.selected,
                                    });
                                    if tab.history.undo.len() > UNDO_MAX {
                                        tab.history.undo.remove(0);
                                    }
                                    tab.history.redo.clear();
                                }
                                tab.props_changed_prev = true;
                                // Rebuild renderer when properties change
                                let cam = tab.renderer.camera.clone();
                                tab.renderer.set_level(level);
                                tab.renderer.camera = cam;
                            } else {
                                tab.props_changed_prev = false;
                            }
                        }
                    } else {
                        ui.label(t.get("panel_select_hint"));
                    }
                });
        }

        // ── Central panel: Canvas ──
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let tab = &mut self.tabs[self.active_tab];
            if tab.level.is_some() {
                let sel = tab.selected;
                tab.renderer.show(ui, sel, t);
                // Pick up click-to-select from renderer
                if let Some(idx) = tab.renderer.clicked_object {
                    tab.selected = Some(idx);
                }
                // Pick up drag result — update object position
                if let Some((idx, delta)) = tab.renderer.drag_result.take()
                    && let Some(ref mut level) = tab.level
                    && idx < level.objects.len()
                {
                    // Snapshot pre-drag state for undo
                    tab.history.undo.push(Snapshot {
                        level: level.clone(),
                        selected: tab.selected,
                    });
                    if tab.history.undo.len() > UNDO_MAX {
                        tab.history.undo.remove(0);
                    }
                    tab.history.redo.clear();

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
                    // Rebuild draw data but preserve camera position/zoom
                    let cam = tab.renderer.camera.clone();
                    tab.renderer.set_level(level);
                    tab.renderer.camera = cam;
                }
                // Pick up terrain node drag result — update node position & regenerate mesh
                if let Some(result) = tab.renderer.node_drag_result.take()
                    && let Some(ref mut level) = tab.level
                    && result.object_index < level.objects.len()
                {
                    // Snapshot pre-drag state for undo (before mutation)
                    tab.history.undo.push(Snapshot {
                        level: level.clone(),
                        selected: tab.selected,
                    });
                    if tab.history.undo.len() > UNDO_MAX {
                        tab.history.undo.remove(0);
                    }
                    tab.history.redo.clear();

                    if let LevelObject::Prefab(ref mut p) = level.objects[result.object_index]
                        && let Some(ref mut td) = p.terrain_data
                    {
                        // Extract current nodes, update dragged node, regenerate meshes
                        let mut nodes = crate::terrain_gen::extract_curve_nodes(td);
                        if result.node_index < nodes.len() {
                            nodes[result.node_index].position = result.new_local_pos;
                            crate::terrain_gen::regenerate_terrain(td, &nodes);
                        }
                    }

                    // Rebuild draw data but preserve camera position/zoom
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
                        // Push undo before mutation
                        tab.history.undo.push(Snapshot {
                            level: level.clone(),
                            selected: tab.selected,
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
                                    // Inherit texture from the preceding node
                                    let tex = nodes
                                        .get(after_node)
                                        .map(|n| n.texture)
                                        .unwrap_or(0);
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
                                        include_bytes!("../assets/drop-icon.svg"),
                                    )
                                    .fit_to_exact_size(egui::Vec2::splat(48.0))
                                    .tint(icon_tint),
                                );

                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(t.get("panel_drop_hint")).color(hint_color),
                                );
                                ui.label(
                                    egui::RichText::new(t.get("panel_open_hint")).color(sub_color),
                                );
                            });
                        },
                    );
                });
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn export_bytes_wasm(file_name: &str, bytes: Vec<u8>) -> Result<(), String> {
    let arr = js_sys::Array::new();
    let u8arr = js_sys::Uint8Array::from(bytes.as_slice());
    arr.push(&u8arr.buffer());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&arr).map_err(|e| format!("{:?}", e))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("{:?}", e))?;

    let window = web_sys::window().ok_or_else(|| "window 不可用".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "document 不可用".to_string())?;
    let body = document
        .body()
        .ok_or_else(|| "document.body 不可用".to_string())?;

    let anchor = document
        .create_element("a")
        .map_err(|e| format!("{:?}", e))?
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "无法创建下载链接".to_string())?;

    anchor
        .set_attribute("href", &url)
        .map_err(|e| format!("{:?}", e))?;
    anchor
        .set_attribute("download", file_name)
        .map_err(|e| format!("{:?}", e))?;
    anchor
        .set_attribute("style", "display:none")
        .map_err(|e| format!("{:?}", e))?;

    body.append_child(&anchor).map_err(|e| format!("{:?}", e))?;
    anchor.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Drag-and-drop payload for the object tree.
struct DndPayload(ObjectIndex);

/// Where to drop an item in the tree.
pub enum DropPosition {
    /// Insert before `target` in its parent's children list (or in roots).
    Before(ObjectIndex),
    /// Insert after `target` in its parent's children list (or in roots).
    After(ObjectIndex),
    /// Insert as the last child of a Parent object.
    IntoParent(ObjectIndex),
}

/// Recursively render the object tree with drag-and-drop support.
/// Returns a `DropPosition` if a drop occurred this frame.
/// Like `selectable_label` but with `Sense::click_and_drag()` so a single
/// widget handles both click-to-select and drag-to-reorder without conflicts.
fn selectable_label_draggable(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    let button_padding = ui.spacing().button_padding;
    let total_extra = button_padding + button_padding;
    let wrap_width = ui.available_width() - total_extra.x;
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        wrap_width,
        egui::TextStyle::Button,
    );
    let mut desired_size = total_extra + galley.size();
    desired_size.y = desired_size.y.max(ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::click_and_drag());
    if ui.is_rect_visible(response.rect) {
        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), rect.shrink2(button_padding))
            .min;
        let visuals = ui.style().interact_selectable(&response, selected);
        if selected || response.hovered() || response.highlighted() {
            let r = rect.expand(visuals.expansion);
            ui.painter().rect(
                r,
                visuals.corner_radius,
                visuals.bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        ui.painter().galley(text_pos, galley, visuals.text_color());
    }
    response
}

fn show_object_tree(
    ui: &mut egui::Ui,
    level: &LevelData,
    idx: ObjectIndex,
    selected: &mut Option<ObjectIndex>,
    depth: usize,
) -> Option<(ObjectIndex, DropPosition)> {
    let obj = &level.objects[idx];
    let is_selected = *selected == Some(idx);
    let mut drop_result: Option<(ObjectIndex, DropPosition)> = None;

    match obj {
        LevelObject::Parent(parent) => {
            let collapse_id = ui.make_persistent_id(format!("obj_{}", idx));
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                collapse_id,
                depth < 1,
            );
            let header_res = ui.horizontal(|ui| {
                let label_res = selectable_label_draggable(ui, is_selected, &parent.name);
                if label_res.clicked() {
                    *selected = Some(idx);
                }
                if label_res.dragged() {
                    label_res.dnd_set_drag_payload(DndPayload(idx));
                }
                state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
                label_res
            });
            let header_rect = header_res.response.rect;

            // Drop target detection on the header
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let in_rect = header_rect.contains(egui::pos2(pointer_pos.x, pointer_pos.y));
                if in_rect {
                    // Upper 25% = before, lower 25% = after, middle 50% = into parent
                    let frac = (pointer_pos.y - header_rect.top()) / header_rect.height();
                    if frac < 0.25 {
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            // Draw line above
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter()
                                .hline(header_rect.x_range(), header_rect.top(), stroke);
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::Before(idx)));
                            }
                        }
                    } else if frac > 0.75 {
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            // Draw line below
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter()
                                .hline(header_rect.x_range(), header_rect.bottom(), stroke);
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::After(idx)));
                            }
                        }
                    } else {
                        // Middle = drop into parent
                        if let Some(_payload) = header_res.inner.dnd_hover_payload::<DndPayload>() {
                            let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                            ui.painter().rect_stroke(
                                header_rect,
                                2.0,
                                stroke,
                                egui::StrokeKind::Outside,
                            );
                            if let Some(payload) =
                                header_res.inner.dnd_release_payload::<DndPayload>()
                                && payload.0 != idx
                            {
                                drop_result = Some((payload.0, DropPosition::IntoParent(idx)));
                            }
                        }
                    }
                }
            }

            // Show children
            state.show_body_indented(&header_res.response, ui, |ui| {
                for &child in &parent.children {
                    if let Some(dr) = show_object_tree(ui, level, child, selected, depth + 1)
                        && drop_result.is_none()
                    {
                        drop_result = Some(dr);
                    }
                }
            });
            state.store(ui.ctx());
        }
        LevelObject::Prefab(prefab) => {
            let label = format!("{} [{}]", prefab.name, prefab.prefab_index);
            let label_res = selectable_label_draggable(ui, is_selected, &label);
            if label_res.clicked() {
                *selected = Some(idx);
            }
            if label_res.dragged() {
                label_res.dnd_set_drag_payload(DndPayload(idx));
            }

            // Drop target: upper half = before, lower half = after
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let r = label_res.rect;
                if r.contains(egui::pos2(pointer_pos.x, pointer_pos.y)) {
                    let frac = (pointer_pos.y - r.top()) / r.height();
                    let pos = if frac < 0.5 {
                        DropPosition::Before(idx)
                    } else {
                        DropPosition::After(idx)
                    };
                    if let Some(_payload) = label_res.dnd_hover_payload::<DndPayload>() {
                        let stroke = egui::Stroke::new(2.0, ui.visuals().selection.bg_fill);
                        let y = if frac < 0.5 { r.top() } else { r.bottom() };
                        ui.painter().hline(r.x_range(), y, stroke);
                    }
                    if let Some(payload) = label_res.dnd_release_payload::<DndPayload>()
                        && payload.0 != idx
                    {
                        drop_result = Some((payload.0, pos));
                    }
                }
            }
        }
    }
    drop_result
}

/// Show editable properties. Returns true if anything changed.
fn show_properties_editable(
    ui: &mut egui::Ui,
    obj: &mut LevelObject,
    t: &'static crate::locale::I18n,
) -> bool {
    let mut changed = false;
    match obj {
        LevelObject::Prefab(p) => {
            ui.label(t.get("prop_type_prefab"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.label(format!("{} {}", t.get("prop_prefab_index"), p.prefab_index));
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "p_pos", &mut p.position);

            ui.label(t.get("prop_rotation"));
            changed |= edit_vec3(ui, "p_rot", &mut p.rotation);

            ui.label(t.get("prop_scale"));
            changed |= edit_vec3(ui, "p_scl", &mut p.scale);

            ui.separator();
            ui.label(format!("{} {:?}", t.get("prop_data_type"), p.data_type));

            if let Some(ref mut td) = p.terrain_data {
                ui.separator();
                ui.label(t.get("prop_terrain"));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_fill_vert_count"),
                    td.fill_mesh.vertices.len()
                ));
                ui.label(format!(
                    "{} {}",
                    t.get("prop_curve_vert_count"),
                    td.curve_mesh.vertices.len()
                ));
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_collider"));
                    changed |= ui.checkbox(&mut td.has_collider, "").changed();
                });
                // Fill color — editable RGBA color picker
                ui.horizontal(|ui| {
                    ui.label(format!("  {}:", t.get("prop_fill_color")));
                    let rgba = td.fill_color.to_rgba8();
                    let mut color =
                        egui::Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
                    if egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut color,
                        egui::color_picker::Alpha::OnlyBlend,
                    )
                    .changed()
                    {
                        td.fill_color = Color {
                            r: color.r() as f32 / 255.0,
                            g: color.g() as f32 / 255.0,
                            b: color.b() as f32 / 255.0,
                            a: color.a() as f32 / 255.0,
                        };
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_tex_index"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_index).range(0..=31))
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_x"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_x).speed(0.01))
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_fill_offset_y"));
                    changed |= ui
                        .add(egui::DragValue::new(&mut td.fill_texture_tile_offset_y).speed(0.01))
                        .changed();
                });
            }

            if let Some(ref mut od) = p.override_data {
                ui.separator();

                // Toggle between tree view and raw text
                let toggle_id = ui.make_persistent_id("ov_raw_toggle");
                let mut show_raw: bool = ui.data(|d| d.get_temp(toggle_id).unwrap_or(false));
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_override"));
                    if ui
                        .small_button(if show_raw {
                            t.get("btn_visual")
                        } else {
                            t.get("btn_text")
                        })
                        .clicked()
                    {
                        show_raw = !show_raw;
                        ui.data_mut(|d| d.insert_temp(toggle_id, show_raw));
                    }
                });

                if show_raw {
                    // Raw text editor
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            if ui.text_edit_multiline(&mut od.raw_text).changed() {
                                od.raw_bytes = od.raw_text.as_bytes().to_vec();
                                changed = true;
                            }
                        });
                } else {
                    // Tree view
                    let mut tree = parse_override_text(&od.raw_text);
                    let mut tree_changed = false;
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            let ovr_root_id = ui.make_persistent_id("ovr_root");
                            tree_changed = show_override_tree(ui, &mut tree, 0, ovr_root_id, t);
                        });
                    if tree_changed {
                        let new_text = serialize_override_tree(&tree, 0);
                        od.raw_text = new_text.clone();
                        od.raw_bytes = new_text.into_bytes();
                        changed = true;
                    }
                }
            }
        }
        LevelObject::Parent(p) => {
            ui.label(t.get("prop_type_parent"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.label(format!(
                "{} {}",
                t.get("prop_child_count"),
                p.children.len()
            ));
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "par_pos", &mut p.position);
        }
    }
    changed
}

/// Editable Vec3 with three DragValue fields. Returns true if changed.
fn edit_vec3(ui: &mut egui::Ui, id_prefix: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.push_id(id_prefix, |ui| {
        ui.horizontal(|ui| {
            ui.label("  X");
            changed |= ui.add(egui::DragValue::new(&mut v.x).speed(0.05)).changed();
            ui.label("Y");
            changed |= ui.add(egui::DragValue::new(&mut v.y).speed(0.05)).changed();
            ui.label("Z");
            changed |= ui.add(egui::DragValue::new(&mut v.z).speed(0.05)).changed();
        });
    });
    changed
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
    // Append as fallback after default Latin fonts
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("cjk".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("cjk".into());
    }
    ctx.set_fonts(fonts);
}

/// Convert days since Unix epoch to (year, month, day).
/// Algorithm from Howard Hinnant's `civil_from_days`.
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
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        // Linux
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

    // Fallback to bundled font so native builds still render Chinese on minimal systems.
    Some(include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    // WASM cannot read host system fonts directly; use bundled CJK fallback.
    Some(include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}

// ── Override tree data structures and editor ──

/// A node in the parsed override tree.
struct OverrideNode {
    node_type: String,
    name: String,
    value: Option<String>,
    children: Vec<OverrideNode>,
}

/// Parse ObjectDeserializer tab-indented text into a tree of OverrideNodes.
fn parse_override_text(raw: &str) -> Vec<OverrideNode> {
    let lines: Vec<&str> = raw.lines().collect();
    parse_override_range(&lines, 0, lines.len(), 0)
}

fn parse_override_range(
    lines: &[&str],
    start: usize,
    end: usize,
    base_depth: usize,
) -> Vec<OverrideNode> {
    let mut result = Vec::new();
    let mut i = start;
    while i < end {
        let line = lines[i].trim_end_matches('\r');
        let depth = line.len() - line.trim_start_matches('\t').len();
        let trimmed = line.trim();
        if trimmed.is_empty() || depth < base_depth {
            i += 1;
            continue;
        }
        if depth > base_depth {
            i += 1;
            continue;
        } // skip orphan deeper lines

        let (node_type, name, value) = parse_override_line(trimmed);

        // Find child range: all subsequent lines with depth > current
        let child_start = i + 1;
        let mut child_end = child_start;
        while child_end < end {
            let cl = lines[child_end].trim_end_matches('\r');
            let cd = cl.len() - cl.trim_start_matches('\t').len();
            if cl.trim().is_empty() {
                child_end += 1;
                continue;
            }
            if cd <= depth {
                break;
            }
            child_end += 1;
        }

        let children = if child_start < child_end {
            parse_override_range(lines, child_start, child_end, depth + 1)
        } else {
            Vec::new()
        };

        result.push(OverrideNode {
            node_type,
            name,
            value,
            children,
        });
        i = child_end;
    }
    result
}

fn parse_override_line(trimmed: &str) -> (String, String, Option<String>) {
    // Check for " = " (value present) or trailing " =" (empty value)
    if let Some(eq_pos) = trimmed.find(" = ").or_else(|| {
        if trimmed.ends_with(" =") {
            Some(trimmed.len() - 2)
        } else {
            None
        }
    }) {
        let before = &trimmed[..eq_pos];
        let after = if eq_pos + 3 <= trimmed.len() {
            &trimmed[eq_pos + 3..]
        } else {
            ""
        };
        let parts: Vec<&str> = before.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            (
                parts[0].to_string(),
                parts[1].to_string(),
                Some(after.to_string()),
            )
        } else {
            (parts[0].to_string(), String::new(), Some(after.to_string()))
        }
    } else {
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string(), None)
        } else {
            (parts[0].to_string(), String::new(), None)
        }
    }
}

/// Serialize override tree back to tab-indented text.
fn serialize_override_tree(nodes: &[OverrideNode], depth: usize) -> String {
    let mut out = String::new();
    let indent: String = "\t".repeat(depth);
    for n in nodes {
        if let Some(ref val) = n.value {
            out.push_str(&format!("{}{} {} = {}\n", indent, n.node_type, n.name, val));
        } else {
            out.push_str(&format!("{}{} {}\n", indent, n.node_type, n.name));
        }
        out.push_str(&serialize_override_tree(&n.children, depth + 1));
    }
    out
}

/// Type badge color for override node types.
fn override_type_color(t: &str) -> egui::Color32 {
    match t {
        "GameObject" => egui::Color32::from_rgb(79, 195, 247),
        "Component" => egui::Color32::from_rgb(102, 187, 106),
        "Float" | "Integer" => egui::Color32::from_rgb(206, 147, 216),
        "Boolean" => egui::Color32::from_rgb(255, 183, 77),
        "String" => egui::Color32::from_rgb(165, 214, 167),
        "Enum" => egui::Color32::from_rgb(244, 143, 177),
        "Vector2" | "Vector3" | "Quaternion" | "Rect" => egui::Color32::from_rgb(128, 222, 234),
        "Color" => egui::Color32::from_rgb(239, 154, 154),
        "Array" | "AnimationCurve" | "Generic" => egui::Color32::from_rgb(144, 164, 174),
        _ => egui::Color32::from_rgb(176, 190, 197),
    }
}

/// Show the override tree editor. Returns true if any value changed.
fn show_override_tree(
    ui: &mut egui::Ui,
    nodes: &mut Vec<OverrideNode>,
    depth: usize,
    ctx_id: egui::Id,
    t: &'static crate::locale::I18n,
) -> bool {
    let mut changed = false;
    let mut to_delete: Option<usize> = None;
    let add_id = ctx_id.with("add");

    for (i, node) in nodes.iter_mut().enumerate() {
        let has_children = !node.children.is_empty();
        let is_container = matches!(
            node.node_type.as_str(),
            "GameObject"
                | "Component"
                | "Array"
                | "AnimationCurve"
                | "Generic"
                | "Element"
                | "Vector2"
                | "Vector3"
                | "Quaternion"
                | "Color"
                | "Rect"
                | "Keyframe"
                | "Bounds"
        ) || has_children;
        let allow_add_sibling = node.node_type != "ArraySize";

        let id = ctx_id.with(i);

        if is_container {
            // Collapsible section
            let header = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                depth < 2,
            );
            header
                .show_header(ui, |ui| {
                    if allow_add_sibling
                        && ui
                            .small_button("+")
                            .on_hover_text(t.get("btn_add"))
                            .clicked()
                    {
                        ui.data_mut(|d| d.insert_temp(add_id, true));
                    }

                    let color = override_type_color(&node.node_type);
                    ui.colored_label(color, &node.node_type);

                    if ui.small_button(t.get("btn_delete")).clicked() {
                        to_delete = Some(i);
                    }

                    let w = ui.available_width().max(30.0);
                    if ui
                        .add(egui::TextEdit::singleline(&mut node.name).desired_width(w))
                        .changed()
                    {
                        changed = true;
                    }
                })
                .body(|ui| {
                    changed |=
                        show_override_tree(ui, &mut node.children, depth + 1, ctx_id.with(i), t);
                });
        } else if node.value.is_some() {
            // Leaf value — editable inline
            ui.horizontal(|ui| {
                // Fixed indent to align with collapse-triangle header content
                ui.add_space(ui.spacing().indent);
                if allow_add_sibling
                    && ui
                        .small_button("+")
                        .on_hover_text(t.get("btn_add"))
                        .clicked()
                {
                    ui.data_mut(|d| d.insert_temp(add_id, true));
                }

                let color = override_type_color(&node.node_type);
                ui.colored_label(color, &node.node_type);
                if ui.small_button(t.get("btn_delete")).clicked() {
                    to_delete = Some(i);
                }

                let avail = ui.available_width();
                let name_w = (avail * 0.4).max(20.0);
                let val_w = (avail - name_w - 12.0).max(20.0); // 12px for "="

                if ui
                    .add(egui::TextEdit::singleline(&mut node.name).desired_width(name_w))
                    .changed()
                {
                    changed = true;
                }
                ui.label("=");
                let val = node.value.as_mut().unwrap();
                if node.node_type == "Boolean" {
                    let mut checked = val.eq_ignore_ascii_case("true");
                    if ui.checkbox(&mut checked, "").changed() {
                        *val = if checked { "True" } else { "False" }.to_string();
                        changed = true;
                    }
                } else if ui
                    .add(egui::TextEdit::singleline(val).desired_width(val_w))
                    .changed()
                {
                    changed = true;
                }
            });
        } else {
            // Non-container without value
            ui.horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                if allow_add_sibling
                    && ui
                        .small_button("+")
                        .on_hover_text(t.get("btn_add"))
                        .clicked()
                {
                    ui.data_mut(|d| d.insert_temp(add_id, true));
                }
                let color = override_type_color(&node.node_type);
                ui.colored_label(color, &node.node_type);
                if ui.small_button(t.get("btn_delete")).clicked() {
                    to_delete = Some(i);
                }

                let w = ui.available_width().max(30.0);
                if ui
                    .add(egui::TextEdit::singleline(&mut node.name).desired_width(w))
                    .changed()
                {
                    changed = true;
                }
            });
        }
    }

    if let Some(idx) = to_delete {
        nodes.remove(idx);
        changed = true;
    }

    // Add-node button (add sibling at this level)
    let mut adding: bool = ui.data(|d| d.get_temp(add_id).unwrap_or(false));
    if adding {
        changed |= show_add_node_form(ui, nodes, add_id, &mut adding, depth, t);
    } else {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            if ui.small_button(t.get("btn_add")).clicked() {
                adding = true;
                ui.data_mut(|d| d.insert_temp(add_id, true));
            }
        });
    }

    changed
}

/// All supported override node types.
const OVERRIDE_ALL_TYPES: &[&str] = &[
    "GameObject",
    "Component",
    "Float",
    "Integer",
    "Boolean",
    "String",
    "Enum",
    "Vector2",
    "Vector3",
    "Quaternion",
    "Color",
    "Array",
    "Generic",
    "Element",
    "AnimationCurve",
    "Keyframe",
    "Rect",
    "Bounds",
    "ObjectReference",
];

/// Default value for a leaf override type.
fn override_default_value(t: &str) -> Option<String> {
    match t {
        "Float" => Some("0".to_string()),
        "Integer" | "Enum" | "ObjectReference" => Some("0".to_string()),
        "Boolean" => Some("False".to_string()),
        "String" => Some(String::new()),
        _ => None,
    }
}

/// Default children for compound/container override types.
fn override_default_children(t: &str) -> Vec<OverrideNode> {
    match t {
        "Array" => vec![OverrideNode {
            node_type: "ArraySize".to_string(),
            name: "size".to_string(),
            value: Some("0".to_string()),
            children: Vec::new(),
        }],
        "Vector2" => ["x", "y"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Vector3" => ["x", "y", "z"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Quaternion" => ["x", "y", "z", "w"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Color" => ["r", "g", "b", "a"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Rect" => ["x", "y", "width", "height"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        "Keyframe" => ["time", "value", "inTangent", "outTangent"]
            .iter()
            .map(|n| OverrideNode {
                node_type: "Float".to_string(),
                name: n.to_string(),
                value: Some("0".to_string()),
                children: Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Inline form for adding a new override node. Returns true if tree changed.
fn show_add_node_form(
    ui: &mut egui::Ui,
    nodes: &mut Vec<OverrideNode>,
    add_id: egui::Id,
    adding: &mut bool,
    _depth: usize,
    t: &'static crate::locale::I18n,
) -> bool {
    let type_id = add_id.with("type");
    let name_id = add_id.with("name");
    let mut selected_idx: usize = ui.data(|d| d.get_temp(type_id).unwrap_or(0));
    let mut name_buf: String = ui.data(|d| d.get_temp::<String>(name_id).unwrap_or_default());
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.add_space(ui.spacing().indent);
        egui::ComboBox::from_id_salt(add_id.with("combo"))
            .width(90.0)
            .selected_text(OVERRIDE_ALL_TYPES[selected_idx])
            .show_index(ui, &mut selected_idx, OVERRIDE_ALL_TYPES.len(), |i| {
                OVERRIDE_ALL_TYPES[i].to_string()
            });
        ui.data_mut(|d| d.insert_temp(type_id, selected_idx));

        let resp = ui.add(
            egui::TextEdit::singleline(&mut name_buf)
                .desired_width(60.0)
                .hint_text(t.get("override_name_hint")),
        );
        ui.data_mut(|d| d.insert_temp(name_id, name_buf.clone()));

        if ui.small_button(t.get("btn_confirm")).clicked()
            || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            let ty = OVERRIDE_ALL_TYPES[selected_idx];
            let name = if name_buf.trim().is_empty() {
                "unnamed".to_string()
            } else {
                name_buf.trim().to_string()
            };
            nodes.push(OverrideNode {
                node_type: ty.to_string(),
                name,
                value: override_default_value(ty),
                children: override_default_children(ty),
            });
            *adding = false;
            ui.data_mut(|d| {
                d.insert_temp(add_id, false);
                d.remove::<usize>(type_id);
                d.remove::<String>(name_id);
            });
            changed = true;
        }
        if ui.small_button(t.get("btn_cancel")).clicked() {
            *adding = false;
            ui.data_mut(|d| d.insert_temp(add_id, false));
        }
    });

    changed
}
