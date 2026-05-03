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

/// Where to drop an item in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    /// Insert before `target` in its parent's children list (or in roots).
    Before(ObjectIndex),
    /// Insert after `target` in its parent's children list (or in roots).
    After(ObjectIndex),
    /// Insert as the last child of a Parent object.
    IntoParent(ObjectIndex),
}

/// Where to paste a subtree in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PastePosition {
    /// Append as the last child of `parent`, or as a new root when `None`.
    AppendTo(Option<ObjectIndex>),
    /// Insert at an exact tree position.
    Exact(DropPosition),
}

/// Complete level data — flat arena of all objects + top-level root indices.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LevelData {
    /// Flat arena of all objects (parents + prefabs).
    pub objects: Vec<LevelObject>,
    /// Indices of top-level (root) objects in the arena.
    pub roots: Vec<ObjectIndex>,
}
