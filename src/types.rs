//! Bad Piggies level data types.
//! Mirrors the TypeScript types from the web-based level editor.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Unpack from a Uint32 RGBA value (big-endian byte order: R G B A).
    pub fn from_packed(val: u32) -> Self {
        Self {
            r: ((val >> 24) & 0xff) as f32 / 255.0,
            g: ((val >> 16) & 0xff) as f32 / 255.0,
            b: ((val >> 8) & 0xff) as f32 / 255.0,
            a: (val & 0xff) as f32 / 255.0,
        }
    }

    /// Pack into a Uint32 RGBA value (big-endian byte order: R G B A).
    pub fn to_packed(self) -> u32 {
        let r = (self.r * 255.0).round() as u32 & 0xff;
        let g = (self.g * 255.0).round() as u32 & 0xff;
        let b = (self.b * 255.0).round() as u32 & 0xff;
        let a = (self.a * 255.0).round() as u32 & 0xff;
        (r << 24) | (g << 16) | (b << 8) | a
    }

    pub fn to_rgba8(self) -> [u8; 4] {
        [
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DataType {
    None = 0,
    Terrain = 1,
    PrefabOverrides = 2,
}

impl DataType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => DataType::Terrain,
            2 => DataType::PrefabOverrides,
            _ => DataType::None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerrainMesh {
    pub vertices: Vec<Vec2>,
    pub indices: Vec<i16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveTexture {
    pub texture_index: i32,
    pub size: Vec2,
    pub fixed_angle: bool,
    pub fade_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainData {
    pub fill_texture_tile_offset_x: f32,
    pub fill_texture_tile_offset_y: f32,
    pub fill_mesh: TerrainMesh,
    pub fill_color: Color,
    pub fill_texture_index: i32,
    pub curve_mesh: TerrainMesh,
    pub curve_textures: Vec<CurveTexture>,
    pub control_texture_count: i32,
    pub control_texture_data: Option<Vec<u8>>,
    pub has_collider: bool,
    /// Cached fill boundary rect `[min_x, min_y, max_x, max_y]`.
    /// Computed once from the original fill mesh on first load, then only expanded
    /// (never shrunk) when nodes are dragged outside the current boundary.
    #[serde(default)]
    pub fill_boundary: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabOverrideData {
    pub raw_text: String,
    pub raw_bytes: Vec<u8>,
}

/// Index into the flat arena of level objects.
pub type ObjectIndex = usize;

/// A prefab instance (leaf node, childCount == 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabInstance {
    pub name: String,
    pub position: Vec3,
    pub prefab_index: i16,
    pub rotation: Vec3,
    pub scale: Vec3,
    pub data_type: DataType,
    pub terrain_data: Option<Box<TerrainData>>,
    pub override_data: Option<PrefabOverrideData>,
    pub parent: Option<ObjectIndex>,
}

/// A parent object (container, childCount > 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentObject {
    pub name: String,
    pub position: Vec3,
    pub children: Vec<ObjectIndex>,
    pub parent: Option<ObjectIndex>,
}

/// A level object — either a prefab instance or a parent container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LevelObject {
    Prefab(PrefabInstance),
    Parent(ParentObject),
}

impl LevelObject {
    pub fn name(&self) -> &str {
        match self {
            LevelObject::Prefab(p) => &p.name,
            LevelObject::Parent(p) => &p.name,
        }
    }

    pub fn position(&self) -> Vec3 {
        match self {
            LevelObject::Prefab(p) => p.position,
            LevelObject::Parent(p) => p.position,
        }
    }

    pub fn as_prefab(&self) -> Option<&PrefabInstance> {
        match self {
            LevelObject::Prefab(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_parent(&self) -> Option<&ParentObject> {
        match self {
            LevelObject::Parent(p) => Some(p),
            _ => None,
        }
    }
}

/// Complete level data — flat arena of all objects + top-level root indices.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelData {
    /// Flat arena of all objects (parents + prefabs).
    pub objects: Vec<LevelObject>,
    /// Indices of top-level (root) objects in the arena.
    pub roots: Vec<ObjectIndex>,
}

impl LevelData {
    /// Delete an object (and all its descendants if it's a parent) from the level.
    /// Remaps all indices in roots, children, and parent fields.
    pub fn delete_object(&mut self, target: ObjectIndex) {
        if target >= self.objects.len() {
            return;
        }

        // Collect all indices to delete (target + descendants)
        let mut to_delete = std::collections::HashSet::new();
        Self::collect_descendants(&self.objects, target, &mut to_delete);

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
            *r = remap[*r].unwrap();
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

    fn collect_descendants(
        objects: &[LevelObject],
        idx: ObjectIndex,
        set: &mut std::collections::HashSet<ObjectIndex>,
    ) {
        set.insert(idx);
        if let LevelObject::Parent(p) = &objects[idx] {
            for &child in &p.children {
                Self::collect_descendants(objects, child, set);
            }
        }
    }

    /// Move `source` to a new position in the tree, described by `drop_pos`.
    /// Returns the new index of the moved object (after any reindexing), or
    /// `None` if the move was invalid.
    pub fn move_object(
        &mut self,
        source: ObjectIndex,
        drop_pos: crate::app::DropPosition,
    ) -> Option<ObjectIndex> {
        use crate::app::DropPosition;
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
            Self::collect_descendants(&self.objects, source, &mut ancestors);
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
