//! Dialog windows — delete confirmation, add object, shortcuts, about, tools.

mod about;
mod add_object;
mod shortcuts;
mod tool;
mod unity3d;

use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use crate::data::assets;
use crate::domain::prefab_override::{
    OverrideNode, find_first_node_mut, parse_override_text, serialize_override_tree,
};
use crate::domain::types::*;
use crate::i18n::locale::I18n;
use crate::renderer::{CursorMode, PreviewPlaybackState, TerrainPresetShape};

use super::EditorApp;
pub(super) use unity3d::{Unity3dExportDialogState, Unity3dImportDialogState};

#[derive(Clone)]
pub(super) struct PrefabOption {
    pub(super) index: i16,
    pub(super) label: String,
}

fn tool_mode_icon(mode: CursorMode) -> egui::Image<'static> {
    match mode {
        CursorMode::Select => egui::Image::from_bytes(
            "bytes://tool-select.svg",
            include_bytes!("../../../assets/ui/tool-select.svg"),
        ),
        CursorMode::BoxSelect => egui::Image::from_bytes(
            "bytes://tool-box-select.svg",
            include_bytes!("../../../assets/ui/tool-box-select.svg"),
        ),
        CursorMode::DrawTerrain => egui::Image::from_bytes(
            "bytes://tool-draw-terrain.svg",
            include_bytes!("../../../assets/ui/tool-draw-terrain.svg"),
        ),
        CursorMode::Pan => egui::Image::from_bytes(
            "bytes://tool-pan.svg",
            include_bytes!("../../../assets/ui/tool-pan.svg"),
        ),
    }
    .fit_to_exact_size(egui::vec2(22.0, 22.0))
}

fn terrain_preset_icon(shape: TerrainPresetShape) -> egui::Image<'static> {
    match shape {
        TerrainPresetShape::Circle => egui::Image::from_bytes(
            "bytes://tool-terrain-ellipse.svg",
            include_bytes!("../../../assets/ui/tool-terrain-ellipse.svg"),
        ),
        TerrainPresetShape::Rectangle => egui::Image::from_bytes(
            "bytes://tool-terrain-rectangle.svg",
            include_bytes!("../../../assets/ui/tool-terrain-rectangle.svg"),
        ),
        TerrainPresetShape::PerfectCircle => egui::Image::from_bytes(
            "bytes://tool-terrain-perfect-circle.svg",
            include_bytes!("../../../assets/ui/tool-terrain-perfect-circle.svg"),
        ),
        TerrainPresetShape::Square => egui::Image::from_bytes(
            "bytes://tool-terrain-square.svg",
            include_bytes!("../../../assets/ui/tool-terrain-square.svg"),
        ),
        TerrainPresetShape::EquilateralTriangle => egui::Image::from_bytes(
            "bytes://tool-terrain-equilateral-triangle.svg",
            include_bytes!("../../../assets/ui/tool-terrain-equilateral-triangle.svg"),
        ),
    }
    .fit_to_exact_size(egui::vec2(22.0, 22.0))
}

fn preview_playback_icon(state: PreviewPlaybackState) -> egui::Image<'static> {
    match state {
        PreviewPlaybackState::Build => egui::Image::from_bytes(
            "bytes://tool-preview-build.svg",
            include_bytes!("../../../assets/ui/tool-preview-build.svg"),
        ),
        PreviewPlaybackState::Play => egui::Image::from_bytes(
            "bytes://tool-preview-play.svg",
            include_bytes!("../../../assets/ui/tool-preview-play.svg"),
        ),
        PreviewPlaybackState::Pause => egui::Image::from_bytes(
            "bytes://tool-preview-pause.svg",
            include_bytes!("../../../assets/ui/tool-preview-pause.svg"),
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

    changed |= ui
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
        crate::domain::terrain_gen::CurveNode {
            position: Vec2 { x: -5.0, y: 0.0 },
            texture: 0,
        },
        crate::domain::terrain_gen::CurveNode {
            position: Vec2 { x: -1.5, y: 0.5 },
            texture: 0,
        },
        crate::domain::terrain_gen::CurveNode {
            position: Vec2 { x: 1.5, y: 0.5 },
            texture: 0,
        },
        crate::domain::terrain_gen::CurveNode {
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
    crate::domain::terrain_gen::regenerate_terrain(&mut td, &default_nodes);
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

fn collect_prefab_names(names: &mut BTreeSet<String>) {
    for relative_path in assets::list_pathnames("Assets/Prefab/", ".prefab") {
        let Some(name) = prefab_asset_display_name(&relative_path) else {
            continue;
        };
        if !name.is_empty() {
            names.insert(name);
        }
    }
}

fn prefab_asset_display_name(relative_path: &str) -> Option<String> {
    parse_prefab_root_name(relative_path).or_else(|| {
        Path::new(relative_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn parse_prefab_root_name(asset_path: &str) -> Option<String> {
    let text = assets::read_pathname_text(asset_path)?;
    let mut root_file_id = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(after) = trimmed.strip_prefix("m_RootGameObject: {fileID: ") else {
            continue;
        };
        let file_id = after.split('}').next()?.trim();
        if !file_id.is_empty() {
            root_file_id = Some(file_id.to_string());
            break;
        }
    }
    let root_file_id = root_file_id?;

    let mut in_root_game_object = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(after) = trimmed.strip_prefix("--- !u!1 &") {
            let file_id = after.split_whitespace().next().unwrap_or_default();
            in_root_game_object = file_id == root_file_id;
            continue;
        }
        if trimmed.starts_with("--- !u!") {
            in_root_game_object = false;
            continue;
        }
        if !in_root_game_object {
            continue;
        }
        let Some(name) = trimmed.strip_prefix("m_Name:") else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    None
}

fn collect_matching_loaders(target_name: &str, matches: &mut Vec<String>) {
    matches.extend(
        assets::list_pathnames("Assets/Resources/levels/", "_loader.prefab")
            .into_iter()
            .filter(|relative_path| {
                Path::new(relative_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case(target_name))
            }),
    );
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

fn resolve_loader_prefab_text(
    file_name: Option<&str>,
    source_path: Option<&str>,
) -> Option<String> {
    let target_name = loader_file_name_for_level(file_name?)?;
    if let Some(source_path) = source_path {
        let source = Path::new(source_path);
        if let Some(parent) = source.parent() {
            let sibling = parent.join(&target_name);
            if sibling.is_file() {
                return fs::read_to_string(sibling).ok();
            }
        }
    }

    let mut matches = Vec::new();
    collect_matching_loaders(&target_name, &mut matches);
    match matches.len() {
        0 => None,
        1 => matches
            .pop()
            .and_then(|path| assets::read_pathname_text(&path)),
        _ => {
            let hint = loader_search_hint(source_path);
            if let Some(hint) = hint
                && let Some(path) = matches
                    .iter()
                    .find(|path| path.to_ascii_lowercase().contains(&hint))
            {
                return assets::read_pathname_text(path);
            }
            matches
                .into_iter()
                .next()
                .and_then(|path| assets::read_pathname_text(&path))
        }
    }
}

fn parse_loader_prefab_count(text: &str) -> Option<i16> {
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
        if indent < base_indent || (indent == base_indent && !trimmed.starts_with("- ")) {
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
    let level_key = file_name.map(crate::domain::level::refs::level_key_from_filename);
    let loader_count = resolve_loader_prefab_text(file_name, source_path)
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
                            .and_then(|key| {
                                crate::domain::level::refs::get_prefab_override(key, index)
                            })
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

pub(super) fn global_prefab_name_options() -> &'static [String] {
    static INSTANCE: OnceLock<Vec<String>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            let mut names = BTreeSet::new();
            collect_prefab_names(&mut names);
            names.into_iter().collect()
        })
        .as_slice()
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
    for obj in level.objects.iter_mut() {
        if let LevelObject::Prefab(p) = obj
            && p.name == "LevelManager"
        {
            let od = p.override_data.get_or_insert_with(|| PrefabOverrideData {
                raw_text: "GameObject LevelManager\n\tComponent LevelManager\n".to_string(),
                raw_bytes: b"GameObject LevelManager\n\tComponent LevelManager\n".to_vec(),
            });

            let mut nodes = parse_override_text(&od.raw_text);
            let level_manager = ensure_level_manager_component(&mut nodes);
            let camera_limits = ensure_child_node(level_manager, "Generic", "m_cameraLimits");
            let top_left = ensure_child_node(camera_limits, "Vector2", "topLeft");
            set_float_child(top_left, "x", vals[0]);
            set_float_child(top_left, "y", vals[1]);

            let size = ensure_child_node(camera_limits, "Vector2", "size");
            set_float_child(size, "x", vals[2]);
            set_float_child(size, "y", vals[3]);

            let result = serialize_override_tree(&nodes);
            od.raw_bytes = result.as_bytes().to_vec();
            od.raw_text = result;
            return;
        }
    }
}

fn ensure_level_manager_component(nodes: &mut Vec<OverrideNode>) -> &mut OverrideNode {
    if find_first_node_mut(nodes, &|node| {
        node.node_type == "Component"
            && node
                .name
                .rsplit('.')
                .next()
                .is_some_and(|name| name == "LevelManager")
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
        node.node_type == "Component"
            && node
                .name
                .rsplit('.')
                .next()
                .is_some_and(|name| name == "LevelManager")
    })
    .expect("LevelManager component should exist")
}

fn ensure_child_node<'a>(
    parent: &'a mut OverrideNode,
    node_type: &str,
    name: &str,
) -> &'a mut OverrideNode {
    if parent.child(node_type, name).is_none() {
        parent.children.push(OverrideNode {
            node_type: node_type.to_string(),
            name: name.to_string(),
            value: None,
            children: Vec::new(),
        });
    }

    parent
        .child_mut(node_type, name)
        .expect("child node should exist")
}

fn set_float_child(parent: &mut OverrideNode, name: &str, value: f32) {
    let child = ensure_child_node(parent, "Float", name);
    child.value = Some(value.to_string());
}

#[cfg(test)]
mod tests {
    use super::{
        global_prefab_name_options, parse_loader_prefab_count, update_camera_limits_in_level,
    };
    use crate::data::assets;
    use crate::domain::prefab_override::{OverrideNode, find_first_node, parse_override_text};
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };

    const EXISTING_CAMERA_LIMITS: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tGeneric m_cameraLimits\n\t\t\tVector2 topLeft\n\t\t\t\tFloat x = 1\n\t\t\t\tFloat y = 2\n\t\t\tVector2 size\n\t\t\t\tFloat x = 3\n\t\t\t\tFloat y = 4\n";

    #[test]
    fn embedded_prefab_name_options_include_goal_area() {
        assert!(
            global_prefab_name_options()
                .iter()
                .any(|name| name == "GoalArea_01")
        );
    }

    #[test]
    fn embedded_loader_prefab_count_parses_valid_loader() {
        let has_loader_with_prefabs =
            assets::list_pathnames("Assets/Resources/levels/", "_loader.prefab")
                .into_iter()
                .filter_map(|asset_path| assets::read_pathname_text(&asset_path))
                .filter_map(|text| parse_loader_prefab_count(&text))
                .any(|count| count > 0);

        assert!(has_loader_with_prefabs);
    }

    #[test]
    fn update_camera_limits_rewrites_existing_override_tree() {
        let mut level = LevelData {
            objects: vec![LevelObject::Prefab(prefab(Some(EXISTING_CAMERA_LIMITS)))],
            roots: vec![0],
        };

        update_camera_limits_in_level(&mut level, [-10.0, 20.0, 30.5, 40.5]);

        let raw_text = level.objects[0]
            .as_prefab()
            .and_then(|prefab| prefab.override_data.as_ref())
            .map(|od| od.raw_text.as_str())
            .expect("missing override data");
        let nodes = parse_override_text(raw_text);
        let camera_limits = find_first_node(&nodes, &|node| {
            node.node_type == "Generic" && node.name == "m_cameraLimits"
        })
        .expect("missing camera limits");

        assert_eq!(
            read_vec2(camera_limits.child("Vector2", "topLeft").unwrap()),
            [-10.0, 20.0]
        );
        assert_eq!(
            read_vec2(camera_limits.child("Vector2", "size").unwrap()),
            [30.5, 40.5]
        );
    }

    #[test]
    fn update_camera_limits_creates_missing_override_data() {
        let mut level = LevelData {
            objects: vec![LevelObject::Prefab(prefab(None))],
            roots: vec![0],
        };

        update_camera_limits_in_level(&mut level, [1.0, 2.0, 3.0, 4.0]);

        let raw_text = level.objects[0]
            .as_prefab()
            .and_then(|prefab| prefab.override_data.as_ref())
            .map(|od| od.raw_text.as_str())
            .expect("missing override data");
        let nodes = parse_override_text(raw_text);
        let camera_limits = find_first_node(&nodes, &|node| {
            node.node_type == "Generic" && node.name == "m_cameraLimits"
        })
        .expect("missing camera limits");

        assert_eq!(
            read_vec2(camera_limits.child("Vector2", "topLeft").unwrap()),
            [1.0, 2.0]
        );
        assert_eq!(
            read_vec2(camera_limits.child("Vector2", "size").unwrap()),
            [3.0, 4.0]
        );
        assert_eq!(
            level.objects[0]
                .as_prefab()
                .and_then(|prefab| prefab.override_data.as_ref())
                .map(|od| od.raw_bytes.as_slice()),
            Some(raw_text.as_bytes())
        );
    }

    fn prefab(raw_text: Option<&str>) -> PrefabInstance {
        PrefabInstance {
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
            override_data: raw_text.map(|text| PrefabOverrideData {
                raw_text: text.to_string(),
                raw_bytes: text.as_bytes().to_vec(),
            }),
            parent: None,
        }
    }

    fn read_vec2(node: &OverrideNode) -> [f32; 2] {
        [
            node.child("Float", "x")
                .and_then(OverrideNode::value_as_f32)
                .expect("missing x"),
            node.child("Float", "y")
                .and_then(OverrideNode::value_as_f32)
                .expect("missing y"),
        ]
    }
}
