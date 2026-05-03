//! Dialog windows — delete confirmation, add object, shortcuts, about, tools.

mod about;
mod add_object;
mod shortcuts;
mod tool;

use eframe::egui;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::locale::I18n;
use crate::renderer::{CursorMode, TerrainPresetShape};
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
            include_bytes!("../../../assets/tool-select.svg"),
        ),
        CursorMode::BoxSelect => egui::Image::from_bytes(
            "bytes://tool-box-select.svg",
            include_bytes!("../../../assets/tool-box-select.svg"),
        ),
        CursorMode::DrawTerrain => egui::Image::from_bytes(
            "bytes://tool-draw-terrain.svg",
            include_bytes!("../../../assets/tool-draw-terrain.svg"),
        ),
        CursorMode::Pan => egui::Image::from_bytes(
            "bytes://tool-pan.svg",
            include_bytes!("../../../assets/tool-pan.svg"),
        ),
    }
    .fit_to_exact_size(egui::vec2(22.0, 22.0))
}

fn terrain_preset_icon(shape: TerrainPresetShape) -> egui::Image<'static> {
    match shape {
        TerrainPresetShape::Circle => egui::Image::from_bytes(
            "bytes://tool-terrain-ellipse.svg",
            include_bytes!("../../../assets/tool-terrain-ellipse.svg"),
        ),
        TerrainPresetShape::Rectangle => egui::Image::from_bytes(
            "bytes://tool-terrain-rectangle.svg",
            include_bytes!("../../../assets/tool-terrain-rectangle.svg"),
        ),
        TerrainPresetShape::PerfectCircle => egui::Image::from_bytes(
            "bytes://tool-terrain-perfect-circle.svg",
            include_bytes!("../../../assets/tool-terrain-perfect-circle.svg"),
        ),
        TerrainPresetShape::Square => egui::Image::from_bytes(
            "bytes://tool-terrain-square.svg",
            include_bytes!("../../../assets/tool-terrain-square.svg"),
        ),
        TerrainPresetShape::EquilateralTriangle => egui::Image::from_bytes(
            "bytes://tool-terrain-equilateral-triangle.svg",
            include_bytes!("../../../assets/tool-terrain-equilateral-triangle.svg"),
        ),
    }
    .fit_to_exact_size(egui::vec2(22.0, 22.0))
}

fn terrain_preset_label_key(shape: TerrainPresetShape) -> &'static str {
    match shape {
        TerrainPresetShape::Circle => "tool_terrain_preset_circle",
        TerrainPresetShape::Rectangle => "tool_terrain_preset_rectangle",
        TerrainPresetShape::PerfectCircle => "tool_terrain_preset_perfect_circle",
        TerrainPresetShape::Square => "tool_terrain_preset_square",
        TerrainPresetShape::EquilateralTriangle => "tool_terrain_preset_equilateral_triangle",
    }
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
        DataType::Terrain => "e2dTerrainBase",
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
