//! Editor actions: undo/redo, clipboard operations, tab management.

use crate::domain::types::*;

use super::{Clipboard, EditorApp, Snapshot, Tab, UNDO_MAX};

impl EditorApp {
    fn clipboard_root_anchor(clipboard: &Clipboard) -> Option<Vec2> {
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut count = 0usize;
        for subtree in &clipboard.subtrees {
            if let Some(root) = subtree.first() {
                let pos = root.position();
                sum_x += pos.x;
                sum_y += pos.y;
                count += 1;
            }
        }
        (count > 0).then(|| Vec2 {
            x: sum_x / count as f32,
            y: sum_y / count as f32,
        })
    }

    fn valid_object_indices(level: &LevelData, indices: &[ObjectIndex]) -> Vec<ObjectIndex> {
        let mut valid: Vec<ObjectIndex> = indices
            .iter()
            .copied()
            .filter(|&idx| idx < level.objects.len())
            .collect();
        valid.sort_unstable();
        valid.dedup();
        valid
    }

    fn flipped_scale_component(scale: f32) -> f32 {
        if scale == 0.0 { 0.0 } else { -scale }
    }

    fn mirror_value_around(value: f32, center: f32) -> f32 {
        center * 2.0 - value
    }

    fn infer_fill_boundary_from_mesh(vertices: &[Vec2]) -> [f32; 4] {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for vertex in vertices {
            min_x = min_x.min(vertex.x);
            min_y = min_y.min(vertex.y);
            max_x = max_x.max(vertex.x);
            max_y = max_y.max(vertex.y);
        }

        if min_x > max_x {
            [0.0, 0.0, 1.0, 1.0]
        } else {
            [min_x, min_y, max_x, max_y]
        }
    }

    fn terrain_nodes_axis_center(
        nodes: &[crate::domain::terrain_gen::CurveNode],
        horizontal: bool,
    ) -> f32 {
        let mut min_axis = f32::MAX;
        let mut max_axis = f32::MIN;

        for node in nodes {
            let axis = if horizontal {
                node.position.x
            } else {
                node.position.y
            };
            min_axis = min_axis.min(axis);
            max_axis = max_axis.max(axis);
        }

        if min_axis > max_axis {
            0.0
        } else {
            (min_axis + max_axis) * 0.5
        }
    }

    fn mirrored_fill_boundary(boundary: [f32; 4], center: f32, horizontal: bool) -> [f32; 4] {
        let [min_x, min_y, max_x, max_y] = boundary;
        if horizontal {
            [
                Self::mirror_value_around(max_x, center),
                min_y,
                Self::mirror_value_around(min_x, center),
                max_y,
            ]
        } else {
            [
                min_x,
                Self::mirror_value_around(max_y, center),
                max_x,
                Self::mirror_value_around(min_y, center),
            ]
        }
    }

    fn flip_terrain_data_along_axis(td: &mut TerrainData, horizontal: bool) {
        let mut nodes = crate::domain::terrain_gen::extract_curve_nodes(td);
        if nodes.is_empty() {
            return;
        }

        let axis_center = Self::terrain_nodes_axis_center(&nodes, horizontal);

        for node in &mut nodes {
            if horizontal {
                node.position.x = Self::mirror_value_around(node.position.x, axis_center);
            } else {
                node.position.y = Self::mirror_value_around(node.position.y, axis_center);
            }
        }

        let boundary = td
            .fill_boundary
            .unwrap_or_else(|| Self::infer_fill_boundary_from_mesh(&td.fill_mesh.vertices));
        td.fill_boundary = Some(Self::mirrored_fill_boundary(
            boundary,
            axis_center,
            horizontal,
        ));

        if horizontal {
            td.fill_texture_tile_offset_x =
                Self::mirror_value_around(td.fill_texture_tile_offset_x, axis_center);
        } else {
            td.fill_texture_tile_offset_y =
                Self::mirror_value_around(td.fill_texture_tile_offset_y, axis_center);
        }

        crate::domain::terrain_gen::regenerate_terrain(td, &nodes);
    }

    fn flip_prefab_along_axis(prefab: &mut PrefabInstance, horizontal: bool) {
        if let Some(td) = prefab.terrain_data.as_mut() {
            Self::flip_terrain_data_along_axis(td, horizontal);
            return;
        }

        let scale = if horizontal {
            &mut prefab.scale.x
        } else {
            &mut prefab.scale.y
        };
        *scale = Self::flipped_scale_component(*scale);
    }

    pub(super) fn copy_objects(&mut self, indices: &[ObjectIndex]) {
        let tab = &self.tabs[self.active_tab];
        let Some(level) = &tab.level else {
            return;
        };
        let valid = Self::valid_object_indices(level, indices);
        if valid.is_empty() {
            return;
        }
        let subtrees: Vec<Vec<LevelObject>> = valid
            .into_iter()
            .map(|index| level.clone_subtree(index))
            .collect();
        if !subtrees.is_empty() {
            self.clipboard = Some(Clipboard { subtrees });
        }
    }

    pub(super) fn cut_objects(&mut self, indices: &[ObjectIndex]) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(level) = &tab.level else {
            return;
        };
        let valid = Self::valid_object_indices(level, indices);
        if valid.is_empty() {
            return;
        }

        let subtrees: Vec<Vec<LevelObject>> = valid
            .iter()
            .copied()
            .map(|index| level.clone_subtree(index))
            .collect();
        self.clipboard = Some(Clipboard { subtrees });

        tab.history.undo.push(Snapshot {
            level: level.clone(),
            selected: tab.selected.clone(),
        });
        if tab.history.undo.len() > UNDO_MAX {
            tab.history.undo.remove(0);
        }
        tab.history.redo.clear();

        if let Some(level) = &mut tab.level {
            for index in valid.into_iter().rev() {
                level.delete_object(index);
            }
            tab.selected.clear();
            tab.renderer.reload_level_preserving_preview_state(level);
        }
    }

    pub(super) fn duplicate_objects(&mut self, indices: &[ObjectIndex]) {
        let previous_warnings = self.current_level_warnings();
        let tab = &mut self.tabs[self.active_tab];
        let Some(level_ref) = tab.level.as_ref() else {
            return;
        };
        let valid = Self::valid_object_indices(level_ref, indices);
        if valid.is_empty() {
            return;
        }

        let items: Vec<(Vec<LevelObject>, Option<ObjectIndex>)> = valid
            .into_iter()
            .map(|sel| {
                let subtree = level_ref.clone_subtree(sel);
                let target = match &level_ref.objects[sel] {
                    LevelObject::Prefab(prefab) => prefab.parent,
                    LevelObject::Parent(parent) => parent.parent,
                };
                (subtree, target)
            })
            .collect();
        if items.is_empty() {
            return;
        }

        tab.history.undo.push(Snapshot {
            level: level_ref.clone(),
            selected: tab.selected.clone(),
        });
        if tab.history.undo.len() > UNDO_MAX {
            tab.history.undo.remove(0);
        }
        tab.history.redo.clear();

        {
            let Some(level) = tab.level.as_mut() else {
                return;
            };
            tab.selected.clear();
            for (subtree, target) in &items {
                let new_root = level.paste_subtree(subtree, PastePosition::AppendTo(*target));
                match &mut level.objects[new_root] {
                    LevelObject::Prefab(prefab) => {
                        prefab.position.x += 1.0;
                        prefab.position.y -= 1.0;
                    }
                    LevelObject::Parent(parent) => {
                        parent.position.x += 1.0;
                        parent.position.y -= 1.0;
                    }
                }
                tab.selected.insert(new_root);
            }
            tab.renderer.reload_level_preserving_preview_state(level);
        }

        self.maybe_warn_about_new_level_risks(&previous_warnings);
    }

    pub(super) fn rotate_objects_z(&mut self, indices: &[ObjectIndex], delta_z_degrees: f32) {
        let Some(level_ref) = self.tabs[self.active_tab].level.as_ref() else {
            return;
        };
        let valid = Self::valid_object_indices(level_ref, indices);
        let rotatable: Vec<ObjectIndex> = valid
            .into_iter()
            .filter(|&idx| matches!(level_ref.objects[idx], LevelObject::Prefab(_)))
            .collect();
        if rotatable.is_empty() {
            return;
        }

        self.push_undo();

        let tab = &mut self.tabs[self.active_tab];
        let Some(level) = tab.level.as_mut() else {
            return;
        };
        for idx in rotatable {
            if let LevelObject::Prefab(prefab) = &mut level.objects[idx] {
                prefab.rotation.z += delta_z_degrees;
            }
        }

        tab.renderer.reload_level_preserving_preview_state(level);
    }

    pub(super) fn flip_objects_horizontal(&mut self, indices: &[ObjectIndex]) {
        self.flip_objects_along_axis(indices, true);
    }

    pub(super) fn flip_objects_vertical(&mut self, indices: &[ObjectIndex]) {
        self.flip_objects_along_axis(indices, false);
    }

    fn flip_objects_along_axis(&mut self, indices: &[ObjectIndex], horizontal: bool) {
        let Some(level_ref) = self.tabs[self.active_tab].level.as_ref() else {
            return;
        };
        let flippable: Vec<ObjectIndex> = Self::valid_object_indices(level_ref, indices)
            .into_iter()
            .filter(|&idx| matches!(level_ref.objects[idx], LevelObject::Prefab(_)))
            .collect();
        if flippable.is_empty() {
            return;
        }

        self.push_undo();

        let tab = &mut self.tabs[self.active_tab];
        let Some(level) = tab.level.as_mut() else {
            return;
        };
        for idx in flippable {
            if let LevelObject::Prefab(prefab) = &mut level.objects[idx] {
                Self::flip_prefab_along_axis(prefab, horizontal);
            }
        }

        tab.renderer.reload_level_preserving_preview_state(level);
    }

    pub(super) fn set_object_scale_xy(&mut self, index: ObjectIndex, scale_xy: Vec2) {
        let Some(level_ref) = self.tabs[self.active_tab].level.as_ref() else {
            return;
        };
        if index >= level_ref.objects.len()
            || !matches!(level_ref.objects[index], LevelObject::Prefab(_))
        {
            return;
        }

        self.push_undo();

        let tab = &mut self.tabs[self.active_tab];
        let Some(level) = tab.level.as_mut() else {
            return;
        };
        if let LevelObject::Prefab(prefab) = &mut level.objects[index] {
            prefab.scale.x = scale_xy.x;
            prefab.scale.y = scale_xy.y;
        }

        tab.renderer.reload_level_preserving_preview_state(level);
    }

    pub(super) fn request_delete_objects(&mut self, indices: &[ObjectIndex]) {
        let tab = &mut self.tabs[self.active_tab];
        let Some(level) = &tab.level else {
            return;
        };
        let valid = Self::valid_object_indices(level, indices);
        if valid.is_empty() {
            return;
        }
        let label = if valid.len() == 1 {
            level.objects[valid[0]].name().to_string()
        } else {
            format!("{} objects", valid.len())
        };
        tab.pending_delete = Some((valid, label));
    }

    /// Snapshot current state onto the undo stack (call before mutation).
    pub(super) fn push_undo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(ref level) = tab.level {
            tab.history.undo.push(Snapshot {
                level: level.clone(),
                selected: tab.selected.clone(),
            });
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
        }
    }

    /// Undo the last change.
    pub(super) fn undo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(snapshot) = tab.history.undo.pop() {
            if let Some(ref level) = tab.level {
                tab.history.redo.push(Snapshot {
                    level: level.clone(),
                    selected: tab.selected.clone(),
                });
            }
            tab.selected = snapshot.selected;
            tab.renderer
                .reload_level_preserving_preview_state(&snapshot.level);
            tab.level = Some(snapshot.level);
        }
    }

    /// Redo the last undone change.
    pub(super) fn redo(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(snapshot) = tab.history.redo.pop() {
            if let Some(ref level) = tab.level {
                tab.history.undo.push(Snapshot {
                    level: level.clone(),
                    selected: tab.selected.clone(),
                });
            }
            tab.selected = snapshot.selected;
            tab.renderer
                .reload_level_preserving_preview_state(&snapshot.level);
            tab.level = Some(snapshot.level);
        }
    }
    /// Copy all selected objects (and their subtrees) to the clipboard.
    pub(super) fn copy_selected(&mut self) {
        let indices: Vec<ObjectIndex> = self.tabs[self.active_tab]
            .selected
            .iter()
            .copied()
            .collect();
        self.copy_objects(&indices);
    }

    /// Cut all selected objects: copy then delete.
    pub(super) fn cut_selected(&mut self) {
        let indices: Vec<ObjectIndex> = self.tabs[self.active_tab]
            .selected
            .iter()
            .copied()
            .collect();
        self.cut_objects(&indices);
    }

    pub(super) fn paste(&mut self) {
        self.paste_with_context(&[], None, None);
    }

    /// Paste from the clipboard, optionally using a specific context target and world position.
    pub(super) fn paste_with_context(
        &mut self,
        context_indices: &[ObjectIndex],
        world_pos: Option<Vec2>,
        paste_position: Option<PastePosition>,
    ) {
        let previous_warnings = self.current_level_warnings();
        let clip = match self.clipboard.clone() {
            Some(c) => c,
            None => return,
        };
        let world_delta = world_pos.and_then(|target| {
            Self::clipboard_root_anchor(&clip).map(|anchor| Vec2 {
                x: target.x - anchor.x,
                y: target.y - anchor.y,
            })
        });
        let tab = &mut self.tabs[self.active_tab];
        if tab.level.is_none() {
            return;
        }
        let target_selection = if context_indices.is_empty() {
            tab.selected.clone()
        } else {
            context_indices.iter().copied().collect()
        };
        let Some(level_ref) = tab.level.as_ref() else {
            return;
        };
        let paste_position = paste_position.unwrap_or_else(|| {
            PastePosition::AppendTo(Tab::paste_target_parent(level_ref, &target_selection))
        });
        // push_undo inline
        {
            tab.history.undo.push(Snapshot {
                level: level_ref.clone(),
                selected: tab.selected.clone(),
            });
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
        }
        {
            let Some(level) = tab.level.as_mut() else {
                return;
            };
            tab.selected.clear();
            for subtree in &clip.subtrees {
                let new_root = level.paste_subtree(subtree, paste_position);
                match &mut level.objects[new_root] {
                    LevelObject::Prefab(p) => {
                        if let Some(delta) = world_delta {
                            p.position.x += delta.x;
                            p.position.y += delta.y;
                        } else {
                            p.position.x += 1.0;
                            p.position.y -= 1.0;
                        }
                    }
                    LevelObject::Parent(p) => {
                        if let Some(delta) = world_delta {
                            p.position.x += delta.x;
                            p.position.y += delta.y;
                        } else {
                            p.position.x += 1.0;
                            p.position.y -= 1.0;
                        }
                    }
                }
                tab.selected.insert(new_root);
            }
            tab.renderer.reload_level_preserving_preview_state(level);
        }

        self.maybe_warn_about_new_level_risks(&previous_warnings);
    }

    /// Duplicate all selected objects in-place.
    pub(super) fn duplicate_selected(&mut self) {
        let indices: Vec<ObjectIndex> = self.tabs[self.active_tab]
            .selected
            .iter()
            .copied()
            .collect();
        self.duplicate_objects(&indices);
    }

    /// Load a level into the active tab (or a new tab if active tab already has a level).
    pub(super) fn load_level_into_tab(
        &mut self,
        name: String,
        data: Vec<u8>,
        source_path: Option<String>,
    ) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() || tab.save_view.is_some() {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level(name, data, i18n, source_path);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs[self.active_tab].load_level(name, data, i18n, source_path);
        }
    }

    /// Load a text-format level into the active tab (or new tab).
    pub(super) fn load_level_text_into_tab(
        &mut self,
        name: String,
        text: &str,
        source_path: Option<String>,
    ) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() || tab.save_view.is_some() {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level_text(name, text, i18n, source_path);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs[self.active_tab].load_level_text(name, text, i18n, source_path);
        }
    }

    /// Load a save file into the active tab (or a new tab if active tab already has content).
    pub(super) fn load_save_into_tab(&mut self, name: String, data: Vec<u8>) {
        let i18n = self.lang.i18n();
        let (sv, status) = super::save_viewer::SaveViewerData::load(&name, &data, i18n);
        let tab = &self.tabs[self.active_tab];
        let is_empty = tab.level.is_none() && tab.save_view.is_none();
        if is_empty {
            self.tabs[self.active_tab].save_view = Some(sv);
            self.tabs[self.active_tab].file_name = Some(name);
            self.tabs[self.active_tab].status = status;
        } else {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, status);
            new_tab.save_view = Some(sv);
            new_tab.file_name = Some(name);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    /// Load a decrypted XML save file into the active tab (or a new tab).
    pub(super) fn load_xml_into_tab(&mut self, name: String, data: Vec<u8>) {
        let i18n = self.lang.i18n();
        let (sv, status) = super::save_viewer::SaveViewerData::load_xml(&name, &data, i18n);
        let tab = &self.tabs[self.active_tab];
        let is_empty = tab.level.is_none() && tab.save_view.is_none();
        if is_empty {
            self.tabs[self.active_tab].save_view = Some(sv);
            self.tabs[self.active_tab].file_name = Some(name);
            self.tabs[self.active_tab].status = status;
        } else {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, status);
            new_tab.save_view = Some(sv);
            new_tab.file_name = Some(name);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    /// Close tab at index. Returns false if last tab was closed (app keeps at least 1 tab).
    pub(super) fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            let tab = &mut self.tabs[0];
            tab.level = None;
            tab.save_view = None;
            tab.file_name = None;
            tab.selected.clear();
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

    /// Primary entry point for opening any file (bundle, level, or save).
    pub(super) fn open_file(&mut self, name: String, data: Vec<u8>, source_path: Option<String>) {
        if name.ends_with(".unity3d") {
            if let Ok(bundle) = crate::io::unity_bundle::UnityBundleReader::new(data) {
                self.bundle_browser = Some(super::dialogs::bundle_browser::BundleBrowserDialog::new(bundle));
            } else {
                self.tabs[self.active_tab].status = format!("解析 Bundle 失败: {}", name);
            }
        } else if name.ends_with(".yaml") || name.ends_with(".yml") || name.ends_with(".toml") {
            match String::from_utf8(data) {
                Ok(text) => self.load_level_text_into_tab(name, &text, source_path),
                Err(_) => {
                    self.tabs[self.active_tab].status = "UTF-8 解码失败".to_string();
                }
            }
        } else if crate::io::crypto::SaveFileType::detect(&name).is_some() {
            self.load_save_into_tab(name, data);
        } else {
            self.load_level_into_tab(name, data, source_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EditorApp;
    use crate::domain::terrain_gen::{CurveNode, extract_curve_nodes, regenerate_terrain};
    use crate::domain::types::{Color, CurveTexture, TerrainData, TerrainMesh, Vec2};

    fn make_terrain_data(nodes: &[CurveNode]) -> TerrainData {
        let mut td = TerrainData {
            fill_texture_tile_offset_x: 0.0,
            fill_texture_tile_offset_y: 3.0,
            fill_mesh: TerrainMesh::default(),
            fill_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            fill_texture_index: 0,
            curve_mesh: TerrainMesh::default(),
            curve_textures: vec![CurveTexture {
                texture_index: 0,
                size: Vec2 { x: 1.0, y: 0.5 },
                fixed_angle: false,
                fade_threshold: 0.0,
            }],
            control_texture_count: 0,
            control_texture_data: None,
            has_collider: true,
            fill_boundary: Some([0.0, -10.0, 10.0, 4.0]),
        };
        regenerate_terrain(&mut td, nodes);
        td.fill_boundary = Some([0.0, -10.0, 10.0, 4.0]);
        td
    }

    #[test]
    fn vertical_flip_keeps_terrain_node_bounds_centered() {
        let original_nodes = vec![
            CurveNode {
                position: Vec2 { x: -1.0, y: 2.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.5, y: 4.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 8.0 },
                texture: 0,
            },
        ];
        let mut td = make_terrain_data(&original_nodes);

        EditorApp::flip_terrain_data_along_axis(&mut td, false);

        let flipped = extract_curve_nodes(&td);
        let ys: Vec<f32> = flipped.iter().map(|node| node.position.y).collect();
        assert!((ys[0] - 8.0).abs() < 0.0001);
        assert!((ys[1] - 6.0).abs() < 0.0001);
        assert!((ys[2] - 2.0).abs() < 0.0001);

        let min_y = ys.iter().copied().fold(f32::MAX, f32::min);
        let max_y = ys.iter().copied().fold(f32::MIN, f32::max);
        assert!((((min_y + max_y) * 0.5) - 5.0).abs() < 0.0001);
        assert!((td.fill_texture_tile_offset_y - 7.0).abs() < 0.0001);
    }
}
