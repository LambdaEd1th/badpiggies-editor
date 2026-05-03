//! Override tree data structures and editor (parsing, rendering, mutation).

use eframe::egui;

use crate::i18n::locale::I18n;

// ── Override tree data structures and editor ──

/// A node in the parsed override tree.
pub(super) struct OverrideNode {
    node_type: String,
    name: String,
    value: Option<String>,
    children: Vec<OverrideNode>,
}

/// Parse ObjectDeserializer tab-indented text into a tree of OverrideNodes.
pub(super) fn parse_override_text(raw: &str) -> Vec<OverrideNode> {
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
pub(super) fn serialize_override_tree(nodes: &[OverrideNode], depth: usize) -> String {
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
pub(super) fn show_override_tree(
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
                if let Some(val) = node.value.as_mut() {
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
