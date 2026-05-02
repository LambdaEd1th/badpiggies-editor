//! Dialog windows — delete confirmation, add object, shortcuts, about, tools.

use eframe::egui;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::locale::I18n;
use crate::renderer::CursorMode;
use crate::types::*;

use super::EditorApp;

#[derive(Clone)]
pub(super) struct PrefabOption {
    pub(super) index: i16,
    pub(super) label: String,
}

fn tool_mode_icon(mode: CursorMode) -> egui::Image<'static> {
    match mode {
        CursorMode::Select => egui::Image::from_bytes(
            "bytes://tool-select.svg",
            include_bytes!("../../assets/tool-select.svg"),
        ),
        CursorMode::BoxSelect => egui::Image::from_bytes(
            "bytes://tool-box-select.svg",
            include_bytes!("../../assets/tool-box-select.svg"),
        ),
        CursorMode::DrawTerrain => egui::Image::from_bytes(
            "bytes://tool-draw-terrain.svg",
            include_bytes!("../../assets/tool-draw-terrain.svg"),
        ),
        CursorMode::Pan => egui::Image::from_bytes(
            "bytes://tool-pan.svg",
            include_bytes!("../../assets/tool-pan.svg"),
        ),
    }
    .fit_to_exact_size(egui::vec2(22.0, 22.0))
}

fn render_vec3_row(ui: &mut egui::Ui, label: &str, value: &mut Vec3) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(&mut value.x).speed(0.1).prefix("x: "));
        ui.add(egui::DragValue::new(&mut value.y).speed(0.1).prefix("y: "));
        ui.add(egui::DragValue::new(&mut value.z).speed(0.1).prefix("z: "));
    });
}

fn add_prefab_default_name(data_type: DataType) -> &'static str {
    match data_type {
        DataType::Terrain => "Terrain",
        _ => "NewObject",
    }
}

fn add_data_type_key(data_type: DataType) -> &'static str {
    match data_type {
        DataType::None => "add_data_type_none",
        DataType::Terrain => "add_data_type_terrain",
        DataType::PrefabOverrides => "add_data_type_prefab_overrides",
    }
}

pub(super) fn render_prefab_index_picker(
    ui: &mut egui::Ui,
    combo_id: &str,
    prefab_index: &mut i16,
    prefab_options: &[PrefabOption],
) -> bool {
    let mut changed = false;

    if prefab_options.is_empty() {
        return ui
            .add(egui::DragValue::new(prefab_index).range(i16::MIN..=i16::MAX))
            .changed();
    }

    let selected_text = prefab_options
        .iter()
        .find(|option| option.index == *prefab_index)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| format!("#{prefab_index}"));
    let combo_width = (ui.available_width() - 72.0).max(140.0);

    egui::ComboBox::from_id_salt(combo_id)
        .width(combo_width)
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for option in prefab_options {
                changed |= ui
                    .selectable_value(prefab_index, option.index, option.label.as_str())
                    .changed();
            }
        });

    changed
        |= ui
            .add(
                egui::DragValue::new(prefab_index)
                    .range(i16::MIN..=i16::MAX)
                    .speed(1),
            )
            .changed();
    changed
}

fn make_override_data(raw_text: String) -> PrefabOverrideData {
    PrefabOverrideData {
        raw_bytes: raw_text.as_bytes().to_vec(),
        raw_text,
    }
}

pub(super) fn build_default_terrain_data() -> TerrainData {
    let default_nodes = vec![
        crate::terrain_gen::CurveNode {
            position: Vec2 { x: -5.0, y: 0.0 },
            texture: 0,
        },
        crate::terrain_gen::CurveNode {
            position: Vec2 { x: -1.5, y: 0.5 },
            texture: 0,
        },
        crate::terrain_gen::CurveNode {
            position: Vec2 { x: 1.5, y: 0.5 },
            texture: 0,
        },
        crate::terrain_gen::CurveNode {
            position: Vec2 { x: 5.0, y: 0.0 },
            texture: 0,
        },
    ];
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
    crate::terrain_gen::regenerate_terrain(&mut td, &default_nodes);
    td
}

fn loader_file_name_for_level(file_name: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower
        .strip_suffix(".bytes")
        .or_else(|| lower.strip_suffix(".yaml"))
        .or_else(|| lower.strip_suffix(".yml"))
        .or_else(|| lower.strip_suffix(".toml"))
        .unwrap_or(&lower);
    let stem = stem.strip_suffix("_data").unwrap_or(stem);
    (!stem.is_empty()).then(|| format!("{stem}_loader.prefab"))
}

fn repo_levels_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .join("Assets/Resources/levels")
}

fn collect_matching_loaders(dir: &Path, target_name: &str, matches: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_matching_loaders(&path, target_name, matches);
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case(target_name) {
            matches.push(path);
        }
    }
}

fn loader_search_hint(source_path: Option<&str>) -> Option<String> {
    let lower = source_path?.to_ascii_lowercase();
    [
        "episode_1",
        "episode_2",
        "episode_3",
        "episode_4",
        "episode_5",
        "episode_6",
        "episode_sandbox",
        "sandbox",
    ]
    .into_iter()
    .find(|hint| lower.contains(hint))
    .map(str::to_string)
}

fn resolve_loader_prefab_path(file_name: Option<&str>, source_path: Option<&str>) -> Option<PathBuf> {
    let target_name = loader_file_name_for_level(file_name?)?;
    if let Some(source_path) = source_path {
        let source = Path::new(source_path);
        if let Some(parent) = source.parent() {
            let sibling = parent.join(&target_name);
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }

    let levels_dir = repo_levels_dir();
    if !levels_dir.is_dir() {
        return None;
    }

    let mut matches = Vec::new();
    collect_matching_loaders(&levels_dir, &target_name, &mut matches);
    match matches.len() {
        0 => None,
        1 => matches.into_iter().next(),
        _ => {
            let hint = loader_search_hint(source_path);
            if let Some(hint) = hint
                && let Some(path) = matches
                    .iter()
                    .find(|path| path.to_string_lossy().to_ascii_lowercase().contains(&hint))
            {
                return Some(path.clone());
            }
            matches.into_iter().next()
        }
    }
}

fn parse_loader_prefab_count(loader_path: &Path) -> Option<i16> {
    let text = fs::read_to_string(loader_path).ok()?;
    let mut in_prefabs = false;
    let mut base_indent = 0usize;
    let mut count = 0usize;

    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = line.len().saturating_sub(trimmed.len());
        if !in_prefabs {
            if trimmed == "m_prefabs:" {
                in_prefabs = true;
                base_indent = indent;
            }
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        if indent <= base_indent {
            break;
        }
        if trimmed.starts_with("- ") {
            count += 1;
        }
    }

    i16::try_from(count).ok().filter(|count| *count > 0)
}

fn build_used_prefab_labels(level: &LevelData) -> BTreeMap<i16, String> {
    let mut labels: BTreeMap<i16, BTreeMap<String, usize>> = BTreeMap::new();
    for object in &level.objects {
        let LevelObject::Prefab(prefab) = object else {
            continue;
        };
        if prefab.prefab_index < 0 {
            continue;
        }
        *labels
            .entry(prefab.prefab_index)
            .or_default()
            .entry(prefab.name.clone())
            .or_default() += 1;
    }

    labels
        .into_iter()
        .map(|(index, names)| {
            let mut names: Vec<(String, usize)> = names.into_iter().collect();
            names.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let primary = names[0].0.clone();
            let extra = names.len().saturating_sub(1);
            let label = if extra > 0 {
                format!("#{index} {primary} (+{extra})")
            } else {
                format!("#{index} {primary}")
            };
            (index, label)
        })
        .collect()
}

pub(super) fn current_level_prefab_options(
    level: Option<&LevelData>,
    file_name: Option<&str>,
    source_path: Option<&str>,
) -> Vec<PrefabOption> {
    let used_labels = level.map(build_used_prefab_labels).unwrap_or_default();
    let level_key = file_name.map(crate::level_refs::level_key_from_filename);
    let loader_count = resolve_loader_prefab_path(file_name, source_path)
        .as_deref()
        .and_then(parse_loader_prefab_count);
    let max_index = used_labels.keys().next_back().copied();
    let count = loader_count.or_else(|| max_index.map(|idx| idx + 1));

    if let Some(count) = count {
        return (0..count)
            .map(|index| {
                let label = used_labels
                    .get(&index)
                    .cloned()
                    .or_else(|| {
                        level_key
                            .as_deref()
                            .and_then(|key| crate::level_refs::get_prefab_override(key, index))
                            .map(|name| format!("#{index} {name}"))
                    })
                    .unwrap_or_else(|| format!("#{index}"));
                PrefabOption { index, label }
            })
            .collect();
    }

    used_labels
        .into_iter()
        .map(|(index, label)| PrefabOption { index, label })
        .collect()
}

impl EditorApp {
    /// Delete confirmation dialog.
    pub(super) fn render_delete_confirm(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if let Some((ref del_indices, ref del_name)) =
            self.tabs[self.active_tab].pending_delete.clone()
        {
            let mut action = 0u8; // 0=pending, 1=confirm, 2=cancel
            egui::Window::new(t.get("win_confirm_delete"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
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
                        let mut sorted: Vec<ObjectIndex> = del_indices.clone();
                        sorted.sort_unstable_by(|a, b| b.cmp(a));
                        for idx in sorted {
                            level.delete_object(idx);
                        }
                        tab.selected.clear();
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
    }

    /// Tool mode selector window.
    pub(super) fn render_tool_window(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_tools {
            return;
        }
        let button_size = egui::vec2(40.0, 40.0);
        let button_spacing = 6.0;
        let button_count = 4.0;
        let window_width = button_size.x * button_count + button_spacing * (button_count - 1.0);
        egui::Window::new(t.get("tool_window_title"))
            .collapsible(true)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_tools)
            .default_pos([8.0, 80.0])
            .fixed_size(egui::vec2(window_width + 16.0, button_size.y + 16.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(button_spacing, 0.0);
                ui.set_width(window_width);
                ui.horizontal(|ui| {
                    let modes = [
                        (CursorMode::Select, "tool_select", "V"),
                        (CursorMode::BoxSelect, "tool_box_select", "M"),
                        (CursorMode::DrawTerrain, "tool_draw_terrain", "P"),
                        (CursorMode::Pan, "tool_pan", "H"),
                    ];
                    for (mode, key, shortcut) in &modes {
                        let tooltip = format!("{} ({})", t.get(key), shortcut);
                        let response = ui.add(
                            egui::Button::image(tool_mode_icon(*mode))
                                .selected(self.cursor_mode == *mode)
                                .frame(true)
                                .min_size(button_size)
                                .image_tint_follows_text_color(true),
                        );
                        let response = response.on_hover_text(tooltip);
                        if response.clicked() {
                            self.cursor_mode = *mode;
                        }
                    }
                });
            });
    }

    /// Shortcuts help window.
    pub(super) fn render_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let t = self.t();
        let is_mac = cfg!(target_os = "macos");

        // Platform-aware shortcut key labels
        let cmd_click = if is_mac { "Cmd+Click" } else { "Ctrl+Click" };
        let undo_key = if is_mac { "Cmd+Z" } else { "Ctrl+Z" };
        let redo_key = if is_mac { "Shift+Cmd+Z" } else { "Ctrl+Y" };
        let copy_key = if is_mac { "Cmd+C" } else { "Ctrl+C" };
        let cut_key = if is_mac { "Cmd+X" } else { "Ctrl+X" };
        let paste_key = if is_mac { "Cmd+V" } else { "Ctrl+V" };
        let dup_key = if is_mac { "Cmd+D" } else { "Ctrl+D" };
        let delete_key = if is_mac { "Delete" } else { "Delete" };

        // Add tool mode shortcuts to the shortcuts window
        egui::Window::new(t.get("win_shortcuts"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_shortcuts)
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong(t.get("shortcuts_key"));
                        ui.strong(t.get("shortcuts_action"));
                        ui.end_row();

                        // ── Mouse ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_mouse"));
                        ui.label("");
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
                        ui.label(cmd_click);
                        ui.label(t.get("shortcuts_cmd_click_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_shift_click"));
                        ui.label(t.get("shortcuts_shift_click_action"));
                        ui.end_row();

                        // ── Keyboard Shortcuts ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_keyboard"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_b_key"));
                        ui.label(t.get("shortcuts_toggle_bg"));
                        ui.end_row();
                        ui.label(undo_key);
                        ui.label(t.get("shortcuts_undo_action"));
                        ui.end_row();
                        ui.label(redo_key);
                        ui.label(t.get("shortcuts_redo_action"));
                        ui.end_row();
                        ui.label(copy_key);
                        ui.label(t.get("shortcuts_copy_action"));
                        ui.end_row();
                        ui.label(cut_key);
                        ui.label(t.get("shortcuts_cut_action"));
                        ui.end_row();
                        ui.label(paste_key);
                        ui.label(t.get("shortcuts_paste_action"));
                        ui.end_row();
                        ui.label(dup_key);
                        ui.label(t.get("shortcuts_duplicate_action"));
                        ui.end_row();
                        ui.label(delete_key);
                        ui.label(t.get("shortcuts_delete_action"));
                        ui.end_row();

                        // ── Tool Modes ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_tools"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_select"));
                        ui.label(t.get("tool_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_box_select"));
                        ui.label(t.get("tool_box_select"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_draw_terrain"));
                        ui.label(t.get("tool_draw_terrain"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_tool_pan"));
                        ui.label(t.get("tool_pan"));
                        ui.end_row();

                        // ── Terrain Editing ──
                        ui.separator();
                        ui.separator();
                        ui.end_row();
                        ui.strong(t.get("shortcuts_section_terrain"));
                        ui.label("");
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_select"));
                        ui.label(t.get("shortcuts_terrain_select_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_drag"));
                        ui.label(t.get("shortcuts_terrain_drag_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_dblclick"));
                        ui.label(t.get("shortcuts_terrain_dblclick_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_delete"));
                        ui.label(t.get("shortcuts_terrain_delete_action"));
                        ui.end_row();
                        ui.label(t.get("shortcuts_terrain_rclick"));
                        ui.label(t.get("shortcuts_terrain_rclick_action"));
                        ui.end_row();
                    });
            });
    }

    /// About window.
    pub(super) fn render_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let t = self.t();
        egui::Window::new(t.get("win_about"))
            .collapsible(false)
            .movable(true)
            .resizable(false)
            .open(&mut self.show_about)
            .show(ctx, |ui| {
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

    /// Add Object dialog.
    pub(super) fn render_add_obj_dialog(&mut self, ctx: &egui::Context, t: &'static I18n) {
        if !self.show_add_dialog {
            return;
        }
        let prefab_options = {
            let tab = &self.tabs[self.active_tab];
            current_level_prefab_options(
                tab.level.as_ref(),
                tab.file_name.as_deref(),
                tab.source_path.as_deref(),
            )
        };
        let mut open = true;
        egui::Window::new(t.get("win_add_object"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let previous_data_type = self.add_obj_data_type;
                ui.horizontal(|ui| {
                    ui.label(t.get("add_type"));
                    ui.radio_value(&mut self.add_obj_is_parent, false, t.get("add_kind_prefab"));
                    ui.radio_value(&mut self.add_obj_is_parent, true, t.get("add_kind_parent"));
                    if !self.add_obj_is_parent {
                        ui.separator();
                        ui.label(t.get("add_data_type"));
                        egui::ComboBox::from_id_salt("add_object_data_type")
                            .selected_text(t.get(add_data_type_key(self.add_obj_data_type)))
                            .show_ui(ui, |ui| {
                                for data_type in [
                                    DataType::None,
                                    DataType::Terrain,
                                    DataType::PrefabOverrides,
                                ] {
                                    ui.selectable_value(
                                        &mut self.add_obj_data_type,
                                        data_type,
                                        t.get(add_data_type_key(data_type)),
                                    );
                                }
                            });
                    }
                });
                if !self.add_obj_is_parent
                    && self.add_obj_data_type != previous_data_type
                    && (self.add_obj_name.is_empty()
                        || self.add_obj_name == add_prefab_default_name(previous_data_type))
                {
                    self.add_obj_name = add_prefab_default_name(self.add_obj_data_type).to_string();
                }
                ui.horizontal(|ui| {
                    ui.label(t.get("add_name"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.add_obj_name)
                            .hint_text(if self.add_obj_is_parent {
                                "NewObject"
                            } else {
                                add_prefab_default_name(self.add_obj_data_type)
                            }),
                    );
                });
                if !self.add_obj_is_parent {
                    ui.horizontal(|ui| {
                        ui.label(t.get("add_prefab_index"));
                        render_prefab_index_picker(
                            ui,
                            "add_object_prefab_index",
                            &mut self.add_obj_prefab_index,
                            &prefab_options,
                        );
                    });
                    if self.add_obj_data_type == DataType::Terrain {
                        ui.small(t.get("add_data_type_terrain_help"));
                    } else if self.add_obj_data_type == DataType::PrefabOverrides {
                        ui.small(t.get("add_data_type_prefab_overrides_help"));
                    }
                }
                ui.separator();
                let position_label = t.get("prop_position");
                let rotation_label = t.get("prop_rotation");
                let scale_label = t.get("prop_scale");
                render_vec3_row(ui, &position_label, &mut self.add_obj_position);
                render_vec3_row(ui, &rotation_label, &mut self.add_obj_rotation);
                render_vec3_row(ui, &scale_label, &mut self.add_obj_scale);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t.get("btn_ok")).clicked() {
                        self.push_undo();
                        let add_name = if self.add_obj_name.trim().is_empty() {
                            if self.add_obj_is_parent {
                                "NewObject".to_string()
                            } else {
                                add_prefab_default_name(self.add_obj_data_type).to_string()
                            }
                        } else {
                            self.add_obj_name.clone()
                        };
                        let is_parent = self.add_obj_is_parent;
                        let data_type = self.add_obj_data_type;
                        let prefab_index = self.add_obj_prefab_index;
                        let position = self.add_obj_position;
                        let rotation = self.add_obj_rotation;
                        let scale = self.add_obj_scale;
                        let tab = &mut self.tabs[self.active_tab];
                        if let Some(ref mut level) = tab.level {
                            let new_idx = level.objects.len();
                            if is_parent {
                                level.objects.push(LevelObject::Parent(ParentObject {
                                    name: add_name.clone(),
                                    position,
                                    children: Vec::new(),
                                    parent: None,
                                }));
                            } else {
                                let terrain_data = (data_type == DataType::Terrain)
                                    .then(|| Box::new(build_default_terrain_data()));
                                let override_data = (data_type == DataType::PrefabOverrides)
                                    .then(|| make_override_data(String::new()));
                                level.objects.push(LevelObject::Prefab(PrefabInstance {
                                    name: add_name.clone(),
                                    position,
                                    prefab_index,
                                    rotation,
                                    scale,
                                    data_type,
                                    terrain_data,
                                    override_data,
                                    parent: None,
                                }));
                            }
                            level.roots.push(new_idx);
                            tab.selected = std::collections::BTreeSet::from([new_idx]);
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
}

/// Update (or create) `m_cameraLimits` in the LevelManager override data.
pub(super) fn update_camera_limits_in_level(level: &mut LevelData, vals: [f32; 4]) {
    // Find LevelManager with override data
    for obj in level.objects.iter_mut() {
        if let LevelObject::Prefab(p) = obj
            && p.name == "LevelManager"
            && let Some(ref mut od) = p.override_data
        {
            if let Some(pos) = od.raw_text.find("m_cameraLimits") {
                // Replace existing float values in-place
                let mut result = od.raw_text[..pos].to_string();
                let after = &od.raw_text[pos..];
                let mut remaining = after;
                let mut val_idx = 0;
                while val_idx < 4 {
                    let fx = remaining.find("Float x = ");
                    let fy = remaining.find("Float y = ");
                    let fp = match (fx, fy) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (Some(a), None) => Some(a),
                        (None, Some(b)) => Some(b),
                        (None, None) => None,
                    };
                    if let Some(fp) = fp {
                        let eq = &remaining[fp..];
                        if let Some(eq_pos) = eq.find("= ") {
                            let before_num = &remaining[..fp + eq_pos + 2];
                            result.push_str(before_num);
                            let num_start = &eq[eq_pos + 2..];
                            let end = num_start
                                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                                .unwrap_or(num_start.len());
                            // Write new value
                            result.push_str(&format!("{}", vals[val_idx]));
                            remaining = &num_start[end..];
                            val_idx += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.push_str(remaining);
                od.raw_bytes = result.as_bytes().to_vec();
                od.raw_text = result;
            }
            return;
        }
    }
}
