//! Editor actions: undo/redo, clipboard operations, tab management.

use crate::types::*;

use super::{Clipboard, EditorApp, Snapshot, Tab, UNDO_MAX};

impl EditorApp {
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
        let tab = &self.tabs[self.active_tab];
        if tab.selected.is_empty() {
            return;
        }
        if let Some(ref level) = tab.level {
            let subtrees: Vec<Vec<LevelObject>> = tab
                .selected
                .iter()
                .filter(|&&sel| sel < level.objects.len())
                .map(|&sel| level.clone_subtree(sel))
                .collect();
            if !subtrees.is_empty() {
                self.clipboard = Some(Clipboard { subtrees });
            }
        }
    }

    /// Cut all selected objects: copy then delete.
    pub(super) fn cut_selected(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if tab.selected.is_empty() {
            return;
        }
        if let Some(ref level) = tab.level {
            let subtrees: Vec<Vec<LevelObject>> = tab
                .selected
                .iter()
                .filter(|&&sel| sel < level.objects.len())
                .map(|&sel| level.clone_subtree(sel))
                .collect();
            if subtrees.is_empty() {
                return;
            }
            self.clipboard = Some(Clipboard { subtrees });
            // push_undo inline
            let undo_snap = Snapshot {
                level: level.clone(),
                selected: tab.selected.clone(),
            };
            tab.history.undo.push(undo_snap);
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
            // Delete in reverse order to keep indices valid
            let mut to_delete: Vec<ObjectIndex> = tab.selected.iter().copied().collect();
            to_delete.sort_unstable_by(|a, b| b.cmp(a));
            if let Some(ref mut level) = tab.level {
                for idx in to_delete {
                    level.delete_object(idx);
                }
                tab.selected.clear();
                let cam = tab.renderer.camera.clone();
                tab.renderer.set_level(level);
                tab.renderer.camera = cam;
            }
        }
    }

    /// Paste from the clipboard, offset slightly from the original position.
    pub(super) fn paste(&mut self) {
        let clip = match self.clipboard.clone() {
            Some(c) => c,
            None => return,
        };
        let tab = &mut self.tabs[self.active_tab];
        if tab.level.is_none() {
            return;
        }
        let target = Tab::paste_target_parent(tab.level.as_ref().unwrap(), &tab.selected);
        // push_undo inline
        {
            let level = tab.level.as_ref().unwrap();
            tab.history.undo.push(Snapshot {
                level: level.clone(),
                selected: tab.selected.clone(),
            });
            if tab.history.undo.len() > UNDO_MAX {
                tab.history.undo.remove(0);
            }
            tab.history.redo.clear();
        }
        let level = tab.level.as_mut().unwrap();
        tab.selected.clear();
        for subtree in &clip.subtrees {
            let new_root = level.paste_subtree(subtree, target);
            match &mut level.objects[new_root] {
                LevelObject::Prefab(p) => {
                    p.position.x += 1.0;
                    p.position.y -= 1.0;
                }
                LevelObject::Parent(p) => {
                    p.position.x += 1.0;
                    p.position.y -= 1.0;
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
        let tab = &mut self.tabs[self.active_tab];
        if tab.selected.is_empty() || tab.level.is_none() {
            return;
        }
        let level_ref = tab.level.as_ref().unwrap();
        let items: Vec<(Vec<LevelObject>, Option<ObjectIndex>)> = tab
            .selected
            .iter()
            .filter(|&&s| s < level_ref.objects.len())
            .map(|&sel| {
                let subtree = level_ref.clone_subtree(sel);
                let target = match &level_ref.objects[sel] {
                    LevelObject::Prefab(p) => p.parent,
                    LevelObject::Parent(p) => p.parent,
                };
                (subtree, target)
            })
            .collect();
        if items.is_empty() {
            return;
        }
        // push_undo inline
        tab.history.undo.push(Snapshot {
            level: level_ref.clone(),
            selected: tab.selected.clone(),
        });
        if tab.history.undo.len() > UNDO_MAX {
            tab.history.undo.remove(0);
        }
        tab.history.redo.clear();
        let level = tab.level.as_mut().unwrap();
        tab.selected.clear();
        for (subtree, target) in &items {
            let new_root = level.paste_subtree(subtree, *target);
            match &mut level.objects[new_root] {
                LevelObject::Prefab(p) => {
                    p.position.x += 1.0;
                    p.position.y -= 1.0;
                }
                LevelObject::Parent(p) => {
                    p.position.x += 1.0;
                    p.position.y -= 1.0;
                }
            }
            tab.selected.insert(new_root);
        }
        let cam = tab.renderer.camera.clone();
        tab.renderer.set_level(level);
        tab.renderer.camera = cam;
    }

    /// Load a level into the active tab (or a new tab if active tab already has a level).
    pub(super) fn load_level_into_tab(&mut self, name: String, data: Vec<u8>) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() || tab.save_view.is_some() {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level(name, data, i18n);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs[self.active_tab].load_level(name, data, i18n);
        }
    }

    /// Load a text-format level into the active tab (or new tab).
    pub(super) fn load_level_text_into_tab(&mut self, name: String, text: &str) {
        let i18n = self.lang.i18n();
        let tab = &self.tabs[self.active_tab];
        if tab.level.is_some() || tab.save_view.is_some() {
            let new_renderer = tab.renderer.clone_for_new_tab();
            let mut new_tab = Tab::new(new_renderer, String::new());
            new_tab.load_level_text(name, text, i18n);
            self.tabs.push(new_tab);
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.tabs[self.active_tab].load_level_text(name, text, i18n);
        }
    }

    /// Load a save file into the active tab (or a new tab if active tab already has content).
    pub(super) fn load_save_into_tab(&mut self, name: String, data: Vec<u8>) {
        let (sv, status) = super::save_viewer::SaveViewerData::load(&name, &data);
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
        let (sv, status) = super::save_viewer::SaveViewerData::load_xml(&name, &data);
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
