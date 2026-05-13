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
            let cam = tab.renderer.camera.clone();
            tab.renderer.set_level(level);
            tab.renderer.camera = cam;
        }
    }

    pub(super) fn duplicate_objects(&mut self, indices: &[ObjectIndex]) {
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
        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
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

        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
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

        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
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
            let cam = tab.renderer.camera.clone();
            tab.renderer.set_level(&snapshot.level);
            tab.renderer.camera = cam;
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
            let cam = tab.renderer.camera.clone();
            tab.renderer.set_level(&snapshot.level);
            tab.renderer.camera = cam;
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
        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
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
}
