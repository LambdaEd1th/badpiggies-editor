//! egui application — main editor UI with three-panel layout.

mod achievement_popup;
mod actions;
mod app_loop;
mod canvas;
mod dialogs;
mod fonts;
mod level_warnings;
mod menu;
mod properties;
mod save_tables;
pub(crate) mod save_viewer;
mod save_xml;
mod state;
mod tab_bar;
mod text_codec;
mod tree;

use dialogs::{Unity3dExportDialogState, Unity3dImportDialogState};
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
    static WASM_OPEN_UNITY3D_EXPORT: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
    static WASM_OPEN_UNITY3D_IMPORT: std::cell::RefCell<Option<(String, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
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
    unity3d_export_dialog: Option<Unity3dExportDialogState>,
    unity3d_import_dialog: Option<Unity3dImportDialogState>,
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
    /// Whether the preview controls panel is visible.
    show_preview_controls_panel: bool,
    /// Animated preview for AchievementPopupEnter.anim.
    achievement_popup: Option<AchievementPopupPreview>,
    /// Texture cache for achievement popup icon previews.
    achievement_popup_tex_cache: TextureCache,
}

impl EditorApp {
    fn terrain_template_for_level(
        level: &LevelData,
        wants_collider: bool,
    ) -> Option<&PrefabInstance> {
        let mut terrain_name_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for object in &level.objects {
            let LevelObject::Prefab(prefab) = object else {
                continue;
            };
            let Some(existing_td) = prefab.terrain_data.as_deref() else {
                continue;
            };
            if existing_td.has_collider == wants_collider {
                *terrain_name_counts.entry(prefab.name.as_str()).or_default() += 1;
            }
        }

        let dominant_terrain_name = terrain_name_counts
            .into_iter()
            .max_by(|(name_a, count_a), (name_b, count_b)| {
                count_a.cmp(count_b).then_with(|| name_b.cmp(name_a))
            })
            .map(|(name, _)| name);

        dominant_terrain_name
            .and_then(|dominant_name| {
                level.objects.iter().find_map(|object| {
                    let LevelObject::Prefab(prefab) = object else {
                        return None;
                    };
                    let td = prefab.terrain_data.as_deref()?;
                    (td.has_collider == wants_collider && prefab.name == dominant_name)
                        .then_some(prefab)
                })
            })
            .or_else(|| {
                level.objects.iter().find_map(|object| {
                    let LevelObject::Prefab(prefab) = object else {
                        return None;
                    };
                    let td = prefab.terrain_data.as_deref()?;
                    (td.has_collider == wants_collider).then_some(prefab)
                })
            })
            .or_else(|| {
                level.objects.iter().find_map(|object| {
                    let LevelObject::Prefab(prefab) = object else {
                        return None;
                    };
                    prefab.terrain_data.as_ref()?;
                    Some(prefab)
                })
            })
    }

    fn terrain_prefab_option_name(label: &str) -> &str {
        let name = label
            .split_once(' ')
            .filter(|(prefix, _)| prefix.starts_with('#'))
            .map(|(_, name)| name)
            .unwrap_or(label);
        name.split(" (+").next().unwrap_or(name)
    }

    fn fallback_terrain_prefab_index_for_level(
        level: &LevelData,
        file_name: Option<&str>,
        source_path: Option<&str>,
        preferred_name: &str,
    ) -> Option<i16> {
        let prefab_options =
            dialogs::current_level_prefab_options(Some(level), file_name, source_path);

        prefab_options
            .iter()
            .find(|option| Self::terrain_prefab_option_name(&option.label) == preferred_name)
            .or_else(|| {
                prefab_options.iter().find(|option| {
                    Self::terrain_prefab_option_name(&option.label).starts_with("e2dTerrain")
                })
            })
            .map(|option| option.index)
    }

    pub(super) fn preferred_terrain_name_for_level(
        level: Option<&LevelData>,
        wants_collider: bool,
    ) -> String {
        level
            .and_then(|level| Self::terrain_template_for_level(level, wants_collider))
            .map(|prefab| prefab.name.clone())
            .unwrap_or_else(|| "e2dTerrainBase".to_string())
    }

    fn dropped_file_name_and_source(file: &egui::DroppedFile) -> (String, Option<String>) {
        if let Some(path) = file.path.as_ref() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                // Some portals/backends can provide a path without a usable file name.
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| file.name.clone());
            (name, Some(path.to_string_lossy().into_owned()))
        } else {
            (file.name.clone(), None)
        }
    }

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
        let initial_tab = Tab::new(renderer);

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
            unity3d_export_dialog: None,
            unity3d_import_dialog: None,
            clipboard: None,
            #[cfg(not(target_arch = "wasm32"))]
            gpu_backend,
            lang,
            cursor_mode: CursorMode::default(),
            show_tools: false,
            show_preview_controls_panel: true,
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

    pub(super) fn build_terrain_prefab_from_local_nodes(
        level: &LevelData,
        file_name: Option<&str>,
        source_path: Option<&str>,
        center: Vec2,
        local_nodes: Vec<crate::domain::terrain_gen::CurveNode>,
        wants_collider: bool,
    ) -> PrefabInstance {
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

        let terrain_template = Self::terrain_template_for_level(level, wants_collider);

        let mut terrain_name = "e2dTerrainBase".to_string();
        let mut prefab_index = None;
        if let Some(template_prefab) = terrain_template {
            terrain_name = template_prefab.name.clone();
            prefab_index = Some(template_prefab.prefab_index);
            let template = template_prefab
                .terrain_data
                .as_deref()
                .expect("terrain template prefab should have terrain_data");
            td.fill_texture_tile_offset_x = template.fill_texture_tile_offset_x;
            td.fill_texture_tile_offset_y = template.fill_texture_tile_offset_y;
            td.fill_color = template.fill_color;
            td.fill_texture_index = template.fill_texture_index;
            if !template.curve_textures.is_empty() {
                td.curve_textures = template.curve_textures.clone();
            }
        }

        td.has_collider = wants_collider;
        crate::domain::terrain_gen::regenerate_terrain(&mut td, &local_nodes);

        let prefab_index = prefab_index
            .or_else(|| {
                Self::fallback_terrain_prefab_index_for_level(
                    level,
                    file_name,
                    source_path,
                    &terrain_name,
                )
            })
            .unwrap_or(0);

        PrefabInstance {
            name: terrain_name,
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
        }
    }

    /// Returns the current language's translations.
    fn t(&self) -> &'static I18n {
        self.lang.i18n()
    }

    /// Clear status overrides that are actually locale-dependent defaults.
    ///
    /// This makes language switching update the status bar immediately
    /// even for tabs that still hold old pre-localized status text.
    pub(super) fn clear_default_status_overrides_on_language_switch(&mut self) {
        for tab in &mut self.tabs {
            if tab.status.is_empty() {
                continue;
            }

            let status = tab.status.clone();
            let mut is_default = false;

            for &lang in Language::all() {
                let i18n = lang.i18n();

                if status == i18n.get("status_welcome") {
                    is_default = true;
                    break;
                }

                if let Some(level) = tab.level.as_ref()
                    && status == i18n.fmt_status_loaded(level.objects.len(), level.roots.len())
                {
                    is_default = true;
                    break;
                }

                if let Some(save_view) = tab.save_view.as_ref()
                    && status == save_view.status_bar_text(tab.file_name.as_deref(), i18n)
                {
                    is_default = true;
                    break;
                }
            }

            if is_default {
                tab.status.clear();
            }
        }
    }

    /// Re-translate simple no-argument status overrides when language changes.
    pub(super) fn relocalize_simple_status_overrides_on_language_switch(&mut self) {
        const KEYS: &[&str] = &[
            "status_exported",
            "status_unity3d_imported",
            "status_unity3d_no_text_assets",
            "status_utf8_decode_failed",
        ];

        let target = self.t();
        for tab in &mut self.tabs {
            if tab.status.is_empty() {
                continue;
            }

            let status = tab.status.clone();
            let mut matched_key = None;
            'find: for key in KEYS {
                for &lang in Language::all() {
                    if status == lang.i18n().get(key) {
                        matched_key = Some(*key);
                        break 'find;
                    }
                }
            }

            if let Some(key) = matched_key {
                tab.status = target.get(key);
            }
        }
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
            self.handle_pending_unity3d_file_dialogs(self.t());
            if let Some((name, data)) = self.pending_file.take() {
                if name.ends_with(".unity3d") {
                    self.open_unity3d_export_with_bytes(name.clone(), name.clone(), data, self.t());
                } else if name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || name.ends_with(".toml")
                {
                    if let Ok(text) = String::from_utf8(data) {
                        self.load_level_text_into_tab(name, &text, None);
                    } else {
                        self.tabs[self.active_tab].status =
                            self.t().get("status_utf8_decode_failed");
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
                        let (name, source_path) = Self::dropped_file_name_and_source(file);
                        Some((name, bytes.to_vec(), source_path))
                    } else if let Some(ref path) = file.path {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            std::fs::read(path).ok().map(|data| {
                                let (name, source_path) = Self::dropped_file_name_and_source(file);
                                (name, data, source_path)
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
                    if name.ends_with(".unity3d") {
                        let t = self.t();
                        self.open_unity3d_export_with_bytes(
                            name.clone(),
                            source_path.as_deref().unwrap_or(&name).to_string(),
                            data,
                            #[cfg(not(target_arch = "wasm32"))]
                            source_path.map(std::path::PathBuf::from),
                            t,
                        );
                    } else if name.ends_with(".yaml")
                        || name.ends_with(".yml")
                        || name.ends_with(".toml")
                    {
                        match String::from_utf8(data) {
                            Ok(text) => self.load_level_text_into_tab(name, &text, source_path),
                            Err(_) => {
                                self.tabs[self.active_tab].status =
                                    self.t().get("status_utf8_decode_failed")
                            }
                        }
                    } else if crate::io::crypto::SaveFileType::detect(&name).is_some() {
                        self.load_save_into_tab(name, data);
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

#[cfg(test)]
mod tests {
    use super::EditorApp;
    use crate::domain::types::{
        Color, CurveTexture, DataType, LevelData, LevelObject, PrefabInstance, TerrainData,
        TerrainMesh, Vec2, Vec3,
    };

    fn sample_terrain_data(has_collider: bool) -> TerrainData {
        TerrainData {
            fill_texture_tile_offset_x: 12.0,
            fill_texture_tile_offset_y: -4.0,
            fill_mesh: TerrainMesh::default(),
            fill_color: Color {
                r: 0.5,
                g: 0.6,
                b: 0.7,
                a: 1.0,
            },
            fill_texture_index: 32,
            curve_mesh: TerrainMesh::default(),
            curve_textures: vec![
                CurveTexture {
                    texture_index: 7,
                    size: Vec2 { x: 0.5, y: 0.5 },
                    fixed_angle: false,
                    fade_threshold: 0.25,
                },
                CurveTexture {
                    texture_index: 9,
                    size: Vec2 { x: 0.1, y: 0.1 },
                    fixed_angle: false,
                    fade_threshold: 0.0,
                },
            ],
            control_texture_count: 0,
            control_texture_data: None,
            has_collider,
            fill_boundary: None,
        }
    }

    #[test]
    fn dropped_file_name_prefers_native_path_filename() {
        let file = egui::DroppedFile {
            path: Some(std::path::PathBuf::from("/tmp/Level_01.bytes")),
            name: String::new(),
            ..Default::default()
        };
        let (name, source) = EditorApp::dropped_file_name_and_source(&file);
        assert_eq!(name, "Level_01.bytes");
        assert_eq!(source.as_deref(), Some("/tmp/Level_01.bytes"));
    }

    #[test]
    fn dropped_file_name_uses_web_name_when_path_absent() {
        let file = egui::DroppedFile {
            name: "web-drop.toml".to_string(),
            ..Default::default()
        };
        let (name, source) = EditorApp::dropped_file_name_and_source(&file);
        assert_eq!(name, "web-drop.toml");
        assert!(source.is_none());
    }

    #[test]
    fn build_terrain_prefab_reuses_existing_template_prefab_index() {
        let terrain_prefab = PrefabInstance {
            name: "e2dTerrainBase_MM_rock".to_string(),
            position: Vec3::default(),
            prefab_index: 12,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            parent: None,
            data_type: DataType::Terrain,
            terrain_data: Some(Box::new(sample_terrain_data(true))),
            override_data: None,
        };
        let other_prefab = PrefabInstance {
            name: "SomeOtherPrefab".to_string(),
            position: Vec3::default(),
            prefab_index: 47,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            parent: None,
            data_type: DataType::None,
            terrain_data: None,
            override_data: None,
        };
        let level = LevelData {
            objects: vec![
                LevelObject::Prefab(terrain_prefab),
                LevelObject::Prefab(other_prefab),
            ],
            roots: vec![0, 1],
        };

        let created = EditorApp::build_terrain_prefab_from_local_nodes(
            &level,
            None,
            None,
            Vec2 { x: 10.0, y: 20.0 },
            vec![
                crate::domain::terrain_gen::CurveNode {
                    position: Vec2 { x: -1.0, y: 0.0 },
                    texture: 0,
                },
                crate::domain::terrain_gen::CurveNode {
                    position: Vec2 { x: 1.0, y: 0.0 },
                    texture: 1,
                },
            ],
            true,
        );

        assert_eq!(created.name, "e2dTerrainBase_MM_rock");
        assert_eq!(created.prefab_index, 12);
        let terrain_data = created.terrain_data.expect("created terrain data");
        assert_eq!(terrain_data.fill_texture_index, 32);
        assert_eq!(terrain_data.curve_textures[1].texture_index, 9);
    }
}
