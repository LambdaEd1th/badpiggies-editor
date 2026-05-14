//! Central canvas panel rendering.

use std::collections::BTreeSet;

use eframe::egui;

use crate::domain::types::*;

use super::EditorApp;
use super::dialogs;
use super::state::{Snapshot, UNDO_MAX};

impl EditorApp {
    pub(super) fn render_canvas(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        let cursor_mode = self.cursor_mode;
        let active_tab = self.active_tab;
        let has_clipboard = self.clipboard.is_some();
        if self.tabs[active_tab].save_view.is_some() {
            let mut achievement_popup_request = None;
            egui::CentralPanel::default().show_inside(ui, |ui| {
                if let Some(sv) = self.tabs[active_tab].save_view.as_mut() {
                    sv.render_save_panels(ui, t, &mut achievement_popup_request);
                }
            });
            if let Some(achievement_id) = achievement_popup_request {
                self.show_achievement_popup(achievement_id, ui.ctx().input(|i| i.time));
            }
        } else {
            let mut canvas_context_action = None;
            let mut canvas_context_selected_object = None;
            let mut canvas_rotation_drag_result = None;
            let mut canvas_scale_drag_result = None;
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
                    // Pick up rotation drag result — update prefab rotation.z
                    if let Some((idx, delta_z_degrees)) = tab.renderer.rotation_drag_result.take() {
                        let mut indices: Vec<usize> = if tab.selected.contains(&idx) {
                            tab.selected.iter().copied().collect()
                        } else {
                            vec![idx]
                        };
                        indices.sort_unstable();
                        indices.dedup();
                        canvas_rotation_drag_result = Some((indices, delta_z_degrees));
                    }
                    // Pick up scale drag result — update prefab scale.x/scale.y
                    if let Some((idx, scale_xy)) = tab.renderer.scale_drag_result.take() {
                        canvas_scale_drag_result = Some((idx, scale_xy));
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
                            let mut nodes = crate::domain::terrain_gen::extract_curve_nodes(td);
                            if result.node_index < nodes.len() {
                                nodes[result.node_index].position = result.new_local_pos;
                                crate::domain::terrain_gen::regenerate_terrain(td, &nodes);
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
                                let mut nodes = crate::domain::terrain_gen::extract_curve_nodes(td);
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
                                            crate::domain::terrain_gen::CurveNode {
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
                                crate::domain::terrain_gen::regenerate_terrain(td, &nodes);
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
                        let local_nodes: Vec<crate::domain::terrain_gen::CurveNode> = result
                            .points
                            .iter()
                            .map(|p| crate::domain::terrain_gen::CurveNode {
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
                        crate::domain::terrain_gen::regenerate_terrain(&mut td, &local_nodes);
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
                                            include_bytes!("../../editor_assets/ui/drop-icon.svg"),
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

            if let Some((indices, delta_z_degrees)) = canvas_rotation_drag_result {
                self.rotate_objects_z(&indices, delta_z_degrees);
            }
            if let Some((index, scale_xy)) = canvas_scale_drag_result {
                self.set_object_scale_xy(index, scale_xy);
            }

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
