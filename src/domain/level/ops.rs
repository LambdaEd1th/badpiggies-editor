//! Level data tree operations (delete, move, reorder).

use crate::domain::types::*;

impl LevelData {
    /// Delete an object (and all its descendants if it's a parent) from the level.
    /// Remaps all indices in roots, children, and parent fields.
    pub fn delete_object(&mut self, target: ObjectIndex) {
        if target >= self.objects.len() {
            return;
        }

        // Collect all indices to delete (target + descendants)
        let mut to_delete = std::collections::HashSet::new();
        collect_descendants(&self.objects, target, &mut to_delete);

        // Build index remapping: old index → new index
        let mut remap: Vec<Option<ObjectIndex>> = Vec::with_capacity(self.objects.len());
        let mut new_idx = 0usize;
        for i in 0..self.objects.len() {
            if to_delete.contains(&i) {
                remap.push(None);
            } else {
                remap.push(Some(new_idx));
                new_idx += 1;
            }
        }

        // Rebuild roots
        self.roots.retain(|r| !to_delete.contains(r));
        for r in &mut self.roots {
            if let Some(mapped) = remap[*r] {
                *r = mapped;
            }
        }

        // Rebuild objects with remapped indices
        let old_objects: Vec<LevelObject> = self.objects.drain(..).collect();
        for (i, obj) in old_objects.into_iter().enumerate() {
            if to_delete.contains(&i) {
                continue;
            }
            match obj {
                LevelObject::Prefab(mut p) => {
                    p.parent = p.parent.and_then(|pi| remap[pi]);
                    self.objects.push(LevelObject::Prefab(p));
                }
                LevelObject::Parent(mut p) => {
                    p.children.retain(|c| !to_delete.contains(c));
                    p.children = p.children.iter().filter_map(|&c| remap[c]).collect();
                    p.parent = p.parent.and_then(|pi| remap[pi]);
                    self.objects.push(LevelObject::Parent(p));
                }
            }
        }
    }

    /// Move `source` to a new position in the tree, described by `drop_pos`.
    /// Returns the new index of the moved object (after any reindexing), or
    /// `None` if the move was invalid.
    pub fn move_object(
        &mut self,
        source: ObjectIndex,
        drop_pos: DropPosition,
    ) -> Option<ObjectIndex> {
        if source >= self.objects.len() {
            return None;
        }

        // Prevent dropping a parent into its own subtree.
        let dest_idx = match &drop_pos {
            DropPosition::Before(t) | DropPosition::After(t) => *t,
            DropPosition::IntoParent(t) => *t,
        };
        {
            let mut ancestors = std::collections::HashSet::new();
            collect_descendants(&self.objects, source, &mut ancestors);
            if ancestors.contains(&dest_idx) {
                return None;
            }
        }

        // 1. Detach source from its current parent (or roots).
        let source_parent = match &self.objects[source] {
            LevelObject::Prefab(p) => p.parent,
            LevelObject::Parent(p) => p.parent,
        };
        if let Some(pi) = source_parent {
            if let LevelObject::Parent(p) = &mut self.objects[pi] {
                p.children.retain(|&c| c != source);
            }
        } else {
            self.roots.retain(|&r| r != source);
        }

        // 2. Determine target parent and insertion position.
        let (target, is_before, into_parent) = match &drop_pos {
            DropPosition::Before(t) => (*t, true, false),
            DropPosition::After(t) => (*t, false, false),
            DropPosition::IntoParent(t) => (*t, false, true),
        };

        if into_parent {
            match &mut self.objects[source] {
                LevelObject::Prefab(p) => p.parent = Some(target),
                LevelObject::Parent(p) => p.parent = Some(target),
            }
            if let LevelObject::Parent(p) = &mut self.objects[target] {
                p.children.push(source);
            }
        } else {
            let target_parent = match &self.objects[target] {
                LevelObject::Prefab(p) => p.parent,
                LevelObject::Parent(p) => p.parent,
            };
            match &mut self.objects[source] {
                LevelObject::Prefab(p) => p.parent = target_parent,
                LevelObject::Parent(p) => p.parent = target_parent,
            }
            if let Some(pi) = target_parent {
                if let LevelObject::Parent(p) = &mut self.objects[pi] {
                    let pos = p.children.iter().position(|&c| c == target).unwrap_or(0);
                    let insert_at = if is_before { pos } else { pos + 1 };
                    p.children.insert(insert_at, source);
                }
            } else {
                let pos = self.roots.iter().position(|&r| r == target).unwrap_or(0);
                let insert_at = if is_before { pos } else { pos + 1 };
                self.roots.insert(insert_at, source);
            }
        }
        Some(source)
    }
}

fn collect_descendants(
    objects: &[LevelObject],
    idx: ObjectIndex,
    set: &mut std::collections::HashSet<ObjectIndex>,
) {
    set.insert(idx);
    if let LevelObject::Parent(p) = &objects[idx] {
        for &child in &p.children {
            collect_descendants(objects, child, set);
        }
    }
}

impl LevelData {
    /// Deep-clone the subtree rooted at `root_idx` into a self-contained
    /// `Clipboard`.  All internal parent/children indices are remapped to
    /// be relative to the cloned vec (root is always index 0).
    pub(crate) fn clone_subtree(&self, root_idx: ObjectIndex) -> Vec<LevelObject> {
        // Collect all indices in the subtree (BFS).
        let mut indices = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(root_idx);
        while let Some(idx) = queue.pop_front() {
            indices.push(idx);
            if let LevelObject::Parent(p) = &self.objects[idx] {
                for &child in &p.children {
                    queue.push_back(child);
                }
            }
        }
        // Build old→new index mapping.
        let remap: std::collections::HashMap<ObjectIndex, ObjectIndex> = indices
            .iter()
            .enumerate()
            .map(|(new, &old)| (old, new))
            .collect();
        // Clone and remap.
        indices
            .iter()
            .map(|&old_idx| {
                let mut obj = self.objects[old_idx].clone();
                match &mut obj {
                    LevelObject::Prefab(p) => {
                        p.parent = p.parent.and_then(|pi| remap.get(&pi).copied());
                    }
                    LevelObject::Parent(p) => {
                        p.parent = p.parent.and_then(|pi| remap.get(&pi).copied());
                        p.children = p
                            .children
                            .iter()
                            .filter_map(|c| remap.get(c).copied())
                            .collect();
                    }
                }
                obj
            })
            .collect()
    }

    /// Paste a subtree (Vec<LevelObject>) into the level. All objects are appended to
    /// the arena, then the pasted root is inserted either as the last child/root
    /// or at an exact `DropPosition`.
    /// Returns the index of the pasted root.
    pub(crate) fn paste_subtree(
        &mut self,
        subtree: &[LevelObject],
        paste_position: PastePosition,
    ) -> ObjectIndex {
        let root_parent = match paste_position {
            PastePosition::AppendTo(parent_idx) => parent_idx,
            PastePosition::Exact(DropPosition::IntoParent(target)) => Some(target),
            PastePosition::Exact(DropPosition::Before(target))
            | PastePosition::Exact(DropPosition::After(target)) => {
                match self.objects.get(target) {
                    Some(LevelObject::Prefab(prefab)) => prefab.parent,
                    Some(LevelObject::Parent(parent)) => parent.parent,
                    None => None,
                }
            }
        };
        let base = self.objects.len();
        for (i, obj) in subtree.iter().enumerate() {
            let mut obj = obj.clone();
            match &mut obj {
                LevelObject::Prefab(p) => {
                    p.parent = p.parent.map(|pi| pi + base);
                }
                LevelObject::Parent(p) => {
                    p.parent = p.parent.map(|pi| pi + base);
                    p.children = p.children.iter().map(|&c| c + base).collect();
                }
            }
            // The root of the pasted subtree: set its parent.
            if i == 0 {
                match &mut obj {
                    LevelObject::Prefab(p) => p.parent = root_parent,
                    LevelObject::Parent(p) => p.parent = root_parent,
                }
            }
            self.objects.push(obj);
        }
        match paste_position {
            PastePosition::AppendTo(Some(parent_idx)) => {
                if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(parent_idx) {
                    parent.children.push(base);
                } else {
                    self.roots.push(base);
                }
            }
            PastePosition::AppendTo(None) => {
                self.roots.push(base);
            }
            PastePosition::Exact(DropPosition::IntoParent(target)) => {
                if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(target) {
                    parent.children.push(base);
                } else {
                    self.roots.push(base);
                }
            }
            PastePosition::Exact(DropPosition::Before(target))
            | PastePosition::Exact(DropPosition::After(target)) => {
                let is_before = matches!(paste_position, PastePosition::Exact(DropPosition::Before(_)));
                let target_parent = match self.objects.get(target) {
                    Some(LevelObject::Prefab(prefab)) => prefab.parent,
                    Some(LevelObject::Parent(parent)) => parent.parent,
                    None => None,
                };
                if let Some(parent_idx) = target_parent {
                    if let Some(LevelObject::Parent(parent)) = self.objects.get_mut(parent_idx) {
                        let pos = parent.children.iter().position(|&child| child == target);
                        let insert_at = pos.map_or(parent.children.len(), |index| {
                            if is_before { index } else { index + 1 }
                        });
                        parent.children.insert(insert_at, base);
                    } else {
                        self.roots.push(base);
                    }
                } else {
                    let pos = self.roots.iter().position(|&root| root == target);
                    let insert_at = pos.map_or(self.roots.len(), |index| {
                        if is_before { index } else { index + 1 }
                    });
                    self.roots.insert(insert_at, base);
                }
            }
        }
        base
    }
}
