//! Properties panel — editable object properties and override tree editor.

use eframe::egui;

use crate::locale::I18n;
use crate::types::*;

use super::{
    dialogs::{
        build_default_terrain_data, current_level_prefab_options,
        render_prefab_index_picker, PrefabOption,
    },
    EditorApp, Snapshot, UNDO_MAX,
};

impl EditorApp {
    /// Render the right properties panel.
    pub(super) fn render_properties_panel(&mut self, ui: &mut egui::Ui) {
        if !self.show_properties {
            return;
        }
        let t = self.t();
        egui::Panel::right("properties")
            .default_size(280.0)
            .size_range(120.0..=f32::INFINITY)
            .resizable(true)
            .show_inside(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().text_edit_width = f32::INFINITY;
                ui.heading(t.get("panel_properties"));
                ui.separator();

                let tab = &mut self.tabs[self.active_tab];
                let prefab_options = current_level_prefab_options(
                    tab.level.as_ref(),
                    tab.file_name.as_deref(),
                    tab.source_path.as_deref(),
                );
                let single_sel = if tab.selected.len() == 1 {
                    Some(*tab.selected.iter().next().unwrap())
                } else {
                    None
                };
                if let (Some(level), Some(sel)) = (&mut tab.level, single_sel) {
                    if sel < level.objects.len() {
                        let pre_obj = if !tab.props_changed_prev {
                            Some(level.objects[sel].clone())
                        } else {
                            None
                        };
                        let changed =
                            show_properties_editable(ui, &mut level.objects[sel], &prefab_options, t);
                        if changed {
                            if let Some(obj_backup) = pre_obj {
                                let mut level_snapshot = level.clone();
                                level_snapshot.objects[sel] = obj_backup;
                                tab.history.undo.push(Snapshot {
                                    level: level_snapshot,
                                    selected: tab.selected.clone(),
                                });
                                if tab.history.undo.len() > UNDO_MAX {
                                    tab.history.undo.remove(0);
                                }
                                tab.history.redo.clear();
                            }
                            tab.props_changed_prev = true;
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
}

/// Show editable properties. Returns true if anything changed.
fn show_properties_editable(
    ui: &mut egui::Ui,
    obj: &mut LevelObject,
    prefab_options: &[PrefabOption],
    t: &'static I18n,
) -> bool {
    let mut changed = false;
    match obj {
        LevelObject::Prefab(p) => {
            ui.label(t.get("prop_type_prefab"));
            ui.horizontal(|ui| {
                ui.label(t.get("prop_name"));
                changed |= ui.text_edit_singleline(&mut p.name).changed();
            });
            ui.horizontal(|ui| {
                ui.label(t.get("prop_prefab_index"));
                changed |= render_prefab_index_picker(
                    ui,
                    "properties_prefab_index",
                    &mut p.prefab_index,
                    prefab_options,
                );
            });
            ui.separator();

            ui.label(t.get("prop_position"));
            changed |= edit_vec3(ui, "p_pos", &mut p.position);

            ui.label(t.get("prop_rotation"));
            changed |= edit_vec3(ui, "p_rot", &mut p.rotation);

            ui.label(t.get("prop_scale"));
            changed |= edit_vec3(ui, "p_scl", &mut p.scale);

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(t.get("prop_data_type"));
                let mut data_type = p.data_type;
                egui::ComboBox::from_id_salt("properties_data_type")
                    .selected_text(data_type_label(data_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut data_type, DataType::None, data_type_label(DataType::None));
                        ui.selectable_value(
                            &mut data_type,
                            DataType::Terrain,
                            data_type_label(DataType::Terrain),
                        );
                        ui.selectable_value(
                            &mut data_type,
                            DataType::PrefabOverrides,
                            data_type_label(DataType::PrefabOverrides),
                        );
                    });
                if data_type != p.data_type {
                    apply_data_type_change(p, data_type);
                    changed = true;
                }
            });

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
                // Fill color
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

                // Closed loop toggle
                let nodes = crate::terrain_gen::extract_curve_nodes(td);
                let is_closed = nodes.len() >= 2 && crate::terrain_gen::is_closed_loop(&nodes);
                let mut closed_val = is_closed;
                ui.horizontal(|ui| {
                    ui.label(t.get("prop_terrain_closed"));
                    if ui.checkbox(&mut closed_val, "").changed() {
                        let mut nodes = nodes.clone();
                        if closed_val && !is_closed && nodes.len() >= 2 {
                            nodes.push(crate::terrain_gen::CurveNode {
                                position: nodes[0].position,
                                texture: nodes[0].texture,
                            });
                        } else if !closed_val && is_closed && nodes.len() >= 3 {
                            nodes.pop();
                        }
                        crate::terrain_gen::regenerate_terrain(td, &nodes);
                        changed = true;
                    }
                });

                // Curve textures (strip width / fade threshold)
                for ct_i in 0..td.curve_textures.len() {
                    ui.horizontal(|ui| {
                        ui.label(t.fmt_idx("prop_curve_tex", ct_i));
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("  {}", t.get("prop_strip_width")));
                        if ui
                            .add(
                                egui::DragValue::new(&mut td.curve_textures[ct_i].size.y)
                                    .speed(0.01)
                                    .range(0.01..=5.0),
                            )
                            .changed()
                        {
                            let nodes = crate::terrain_gen::extract_curve_nodes(td);
                            crate::terrain_gen::regenerate_terrain(td, &nodes);
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(format!("  {}", t.get("prop_fade_threshold")));
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut td.curve_textures[ct_i].fade_threshold)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            )
                            .changed();
                    });
                }
            }

            if let Some(ref mut od) = p.override_data {
                ui.separator();

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
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            if ui.text_edit_multiline(&mut od.raw_text).changed() {
                                od.raw_bytes = od.raw_text.as_bytes().to_vec();
                                changed = true;
                            }
                        });
                } else {
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

fn data_type_label(data_type: DataType) -> &'static str {
    match data_type {
        DataType::None => "None",
        DataType::Terrain => "Terrain",
        DataType::PrefabOverrides => "PrefabOverrides",
    }
}

fn apply_data_type_change(prefab: &mut PrefabInstance, data_type: DataType) {
    prefab.data_type = data_type;
    match data_type {
        DataType::None => {
            prefab.terrain_data = None;
            prefab.override_data = None;
        }
        DataType::Terrain => {
            if prefab.terrain_data.is_none() {
                prefab.terrain_data = Some(Box::new(build_default_terrain_data()));
            }
            prefab.override_data = None;
        }
        DataType::PrefabOverrides => {
            prefab.terrain_data = None;
            if prefab.override_data.is_none() {
                prefab.override_data = Some(PrefabOverrideData {
                    raw_text: String::new(),
                    raw_bytes: Vec::new(),
                });
            }
        }
    }
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
        }

        let (node_type, name, value) = parse_override_line(trimmed);

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
    let trimmed = trimmed.trim_start_matches('\u{feff}');
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
    t: &'static I18n,
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

                    if ui.small_button("×").clicked() {
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
                if ui.small_button("×").clicked() {
                    to_delete = Some(i);
                }

                let avail = ui.available_width();
                let name_w = (avail * 0.4).max(20.0);
                let val_w = (avail - name_w - 12.0).max(20.0);

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
                if ui.small_button("×").clicked() {
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
    t: &'static I18n,
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
