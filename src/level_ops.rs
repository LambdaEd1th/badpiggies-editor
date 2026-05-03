//! Level data tree operations (delete, move, reorder).

use crate::types::*;

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
