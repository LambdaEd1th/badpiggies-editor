//! egui application — main editor UI with three-panel layout.

mod achievement_popup;
mod actions;
mod app_loop;
mod canvas;
mod dialogs;
mod fonts;
mod menu;
mod properties;
mod save_tables;
pub(crate) mod save_viewer;
mod save_xml;
mod state;
mod tab_bar;
mod text_codec;
mod tree;

use state::{Clipboard, Snapshot, Tab, UNDO_MAX};

use eframe::egui;

use crate::data::assets::TextureCache;
use crate::domain::types::*;
use crate::i18n::locale::{I18n, Language};
use crate::renderer::{CursorMode, LevelRenderer, TerrainPresetShape};
use achievement_popup::AchievementPopupPreview;

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WASM_OPEN_RESULT: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
    static WASM_OPEN_XML_SAVE: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
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
    #[cfg(not(target_arch = "wasm32"))]
    gpu_backend: String,
    /// Current UI language.
    lang: Language,
    /// Active cursor/tool mode.
    cursor_mode: CursorMode,
    /// Whether the tool panel is visible.
    show_tools: bool,
    /// Animated preview for AchievementPopupEnter.anim.
    achievement_popup: Option<AchievementPopupPreview>,
    /// Texture cache for achievement popup icon previews.
    achievement_popup_tex_cache: TextureCache,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        fonts::configure_cjk_fonts(&cc.egui_ctx);

        // Disable debug visualisations that cause red-frame flicker in dev builds
        #[cfg(debug_assertions)]
        cc.egui_ctx.global_style_mut(|s| {
            s.debug.show_unaligned = false;
            s.debug.warn_if_rect_changes_id = false;
        });

        #[cfg(not(target_arch = "wasm32"))]
        let renderer = LevelRenderer::new(cc.wgpu_render_state.as_ref());
        #[cfg(target_arch = "wasm32")]
        let renderer = LevelRenderer::new(cc.wgpu_render_state.as_ref());

        #[cfg(not(target_arch = "wasm32"))]
        let gpu_backend = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| rs.adapter.get_info().backend.to_string())
            .unwrap_or_default();

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
            #[cfg(not(target_arch = "wasm32"))]
            gpu_backend,
            lang,
            cursor_mode: CursorMode::default(),
            show_tools: false,
            achievement_popup: None,
            achievement_popup_tex_cache: TextureCache::new(),
        }
    }

    pub(super) fn prepare_add_object_dialog(&mut self) {
        self.prepare_add_object_dialog_at(None);
    }

    fn toggle_active_terrain_preset(&mut self, shape: TerrainPresetShape) {
        if self.tabs[self.active_tab].is_save_tab() {
            return;
        }

        self.tabs[self.active_tab]
            .renderer
            .toggle_terrain_preset(shape);
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
                LevelObject::Prefab(prefab) if prefab.prefab_index >= 0 => {
                    Some(prefab.prefab_index)
                }
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
                } else if crate::io::crypto::SaveFileType::detect(&name).is_some() {
                    self.load_save_into_tab(name, data);
                } else {
                    self.load_level_into_tab(name, data, None);
                }
            }
        }

        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                let file_data: Option<(String, Vec<u8>, Option<String>)> =
                    if let Some(ref bytes) = file.bytes {
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

/// Convert days since Unix epoch to (year, month, day).
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn civil_from_days(z: i64) -> (i64, u32, u32) {
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
