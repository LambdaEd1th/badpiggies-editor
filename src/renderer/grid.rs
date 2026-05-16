//! Construction grid — parse and render the placement grid overlay.
//!
//! Parses `m_constructionGridRows` from LevelManager override data and renders
//! each available cell using the same prefab semantics as Unity:
//! default `ConstructionUI` uses `GridCell`, while an explicit
//! `m_gridCellPrefab` ObjectReference override switches to `GridCellLight`.
//!
//! Unity rendering: shader `_Custom/Unlit_ColorTransparent_Geometry` applies
//! `tex2D(_MainTex, uv) * _Color` with `_Color = (1,1,1,1)` and standard
//! alpha blending (`Blend SrcAlpha OneMinusSrcAlpha`). The sprite's own RGBA
//! pixels determine the final visual.

use eframe::egui;

use crate::data::assets::TextureCache;
use crate::data::sprite_db;
use crate::domain::prefab_override_runtime::{RuntimeOverrideDocument, RuntimeOverrideNode};
use crate::domain::types::*;

use super::Camera;

const GRID_PREFAB_LOCAL_SCALE: f32 = 0.3;
const WORLD_SCALE: f32 = 10.0 / 768.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructionGridCellStyle {
    Default,
    Light,
}

impl ConstructionGridCellStyle {
    pub(crate) fn sprite_name(self) -> &'static str {
        match self {
            Self::Default => "GridCell",
            Self::Light => "GridCellLight",
        }
    }

    pub(crate) fn texture_cache_key(self) -> &'static str {
        match self {
            Self::Default => "GridCell_raw",
            Self::Light => "GridCellLight_raw",
        }
    }

    pub(crate) fn half_extents(self) -> (f32, f32) {
        match self {
            Self::Default => (
                103.0 * GRID_PREFAB_LOCAL_SCALE * WORLD_SCALE,
                104.0 * GRID_PREFAB_LOCAL_SCALE * WORLD_SCALE,
            ),
            Self::Light => (
                104.0 * GRID_PREFAB_LOCAL_SCALE * WORLD_SCALE,
                105.0 * GRID_PREFAB_LOCAL_SCALE * WORLD_SCALE,
            ),
        }
    }

    pub(crate) fn fallback_color(self) -> egui::Color32 {
        match self {
            Self::Default => egui::Color32::from_rgba_unmultiplied(126, 133, 148, 112),
            Self::Light => egui::Color32::from_rgba_unmultiplied(48, 48, 48, 64),
        }
    }
}

/// Parsed construction grid.
pub struct ConstructionGrid {
    /// Bitmask per row (bit = cell available).
    pub rows: Vec<i32>,
    /// LevelStart world position (grid origin).
    pub base_x: f32,
    pub base_y: f32,
    /// Max columns (highest bit + 1).
    pub grid_width: i32,
    /// Number of active rows.
    pub grid_height: i32,
    /// Left-most column offset relative to base.
    pub x_min: i32,
    /// Which cell prefab Unity would use for this level.
    pub cell_style: ConstructionGridCellStyle,
}

fn parse_grid_cell_style(document: &RuntimeOverrideDocument) -> ConstructionGridCellStyle {
    if document.roots.iter().any(|root| {
        root.find_descendant(&|node| {
            node.node_type == "ObjectReference" && node.name == "m_gridCellPrefab"
        })
        .is_some()
    })
    {
        ConstructionGridCellStyle::Light
    } else {
        ConstructionGridCellStyle::Default
    }
}

/// Try to parse the construction grid from level data.
/// Looks for LevelManager override data and LevelStart position.
pub fn parse_construction_grid(level: &LevelData) -> Option<ConstructionGrid> {
    let mut lm_override: Option<&str> = None;
    let mut level_start_pos = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    for obj in &level.objects {
        match obj {
            LevelObject::Prefab(p) => {
                if p.name == "LevelManager"
                    && let Some(ref od) = p.override_data
                {
                    lm_override = Some(&od.raw_text);
                }
                if p.name == "LevelStart" {
                    level_start_pos = p.position;
                }
            }
            LevelObject::Parent(p) => {
                if p.name == "LevelStart" {
                    level_start_pos = p.position;
                }
            }
        }
    }

    let text = lm_override?;

    let document = RuntimeOverrideDocument::parse(text);
    let cell_style = parse_grid_cell_style(&document);
    let rows_node = document.roots.iter().find_map(|root| {
        root.find_descendant(&|node| {
            node.node_type == "Array" && node.name == "m_constructionGridRows"
        })
    })?;
    let rows_array = rows_node.as_array()?;
    let row_count = rows_array.size?;

    if row_count == 0 {
        return None;
    }

    let mut rows = Vec::with_capacity(row_count);
    for element in rows_array.iter() {
        let value = read_grid_row_value(&element.value)?;
        rows.push(value);
    }

    if rows.len() != row_count {
        return None;
    }

    // Compute grid dimensions
    let mut grid_width: i32 = 0;
    let mut grid_height: i32 = 0;
    for (r, &bits) in rows.iter().enumerate() {
        if bits != 0 {
            grid_height = r as i32 + 1;
            let highest_bit = 32 - (bits as u32).leading_zeros() as i32;
            if highest_bit > grid_width {
                grid_width = highest_bit;
            }
        }
    }

    if grid_width == 0 {
        return None;
    }

    // Center: -(gridWidth - 1) / 2, integer division truncating toward zero
    let x_min = -((grid_width - 1) / 2);

    Some(ConstructionGrid {
        rows,
        base_x: level_start_pos.x,
        base_y: level_start_pos.y,
        grid_width,
        grid_height,
        x_min,
        cell_style,
    })
}

fn read_grid_row_value(node: &RuntimeOverrideNode) -> Option<i32> {
    if node.node_type == "Integer" && node.name == "data" {
        node.value_as_i32()
    } else {
        node.find_descendant(&|child| child.node_type == "Integer" && child.name == "data")
            .and_then(RuntimeOverrideNode::value_as_i32)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_construction_grid, ConstructionGridCellStyle};
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };

    const LEVEL_MANAGER_OVERRIDE: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tArray m_constructionGridRows\n\t\t\tArraySize size = 4\n\t\t\tElement 0\n\t\t\t\tInteger data = 15\n\t\t\tElement 1\n\t\t\t\tInteger data = 15\n\t\t\tElement 2\n\t\t\t\tInteger data = 0\n\t\t\tElement 3\n\t\t\t\tInteger data = 2\n\t\tObjectReference m_gridCellPrefab = 6\n";

    #[test]
    fn parses_construction_grid_rows_and_light_cell_style_from_ast() {
        let level = LevelData {
            objects: vec![
                LevelObject::Prefab(prefab("LevelManager", Vec3::default(), LEVEL_MANAGER_OVERRIDE)),
                LevelObject::Prefab(prefab(
                    "LevelStart",
                    Vec3 {
                        x: 3.0,
                        y: 4.0,
                        z: 0.0,
                    },
                    "",
                )),
            ],
            roots: vec![0, 1],
        };

        let grid = parse_construction_grid(&level).expect("expected construction grid");
        assert_eq!(grid.rows, vec![15, 15, 0, 2]);
        assert_eq!(grid.base_x, 3.0);
        assert_eq!(grid.base_y, 4.0);
        assert_eq!(grid.grid_width, 4);
        assert_eq!(grid.grid_height, 4);
        assert_eq!(grid.x_min, -1);
        assert_eq!(grid.cell_style, ConstructionGridCellStyle::Light);
    }

    fn prefab(name: &str, position: Vec3, raw_text: &str) -> PrefabInstance {
        PrefabInstance {
            name: name.to_string(),
            position,
            prefab_index: 0,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            data_type: DataType::None,
            terrain_data: None,
            override_data: if raw_text.is_empty() {
                None
            } else {
                Some(PrefabOverrideData {
                    raw_text: raw_text.to_string(),
                    raw_bytes: raw_text.as_bytes().to_vec(),
                })
            },
            parent: None,
        }
    }
}

/// Draw the construction grid matching Unity's rendering.
///
/// Unity shader: `output = tex2D(_MainTex, uv) * _Color` with `_Color = (1,1,1,1)`,
/// blend mode `Blend SrcAlpha OneMinusSrcAlpha`.  The sprite texture's own RGBA
/// values determine the final appearance.
pub fn draw_construction_grid(
    painter: &egui::Painter,
    grid: &ConstructionGrid,
    camera: &Camera,
    canvas_center: egui::Vec2,
    canvas_rect: egui::Rect,
    tex_cache: &mut TextureCache,
    ctx: &egui::Context,
) {
    let style = grid.cell_style;
    let sprite = sprite_db::get_sprite_info(style.sprite_name());

    let tex_id = if let Some(s) = sprite {
        let uv = &s.uv;
        let atlas_path = format!("sprites/{}", s.atlas);
        tex_cache.load_sprite_crop(
            ctx,
            style.texture_cache_key(),
            &atlas_path,
            [uv.x, uv.y, uv.w, uv.h],
        )
    } else {
        None
    };
    let uv_min = egui::pos2(0.0, 0.0);
    let uv_max = egui::pos2(1.0, 1.0);

    let (half_w, half_h) = style.half_extents();

    // Vertex tint = white → passes texture through unmodified,
    // matching Unity's `_Color = (1,1,1,1)`.
    let tint = egui::Color32::from_rgba_premultiplied(255, 255, 255, 255);

    for row in 0..grid.grid_height {
        let bits = grid.rows[row as usize];
        if bits == 0 {
            continue;
        }
        for col in 0..grid.grid_width {
            if bits & (1 << col) == 0 {
                continue;
            }
            let wx = grid.base_x + (grid.x_min + col) as f32;
            let wy = grid.base_y + row as f32;

            let center =
                camera.world_to_screen(crate::domain::types::Vec2 { x: wx, y: wy }, canvas_center);

            let hw = half_w * camera.zoom;
            let hh = half_h * camera.zoom;

            let cell_rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));

            if !cell_rect.intersects(canvas_rect) {
                continue;
            }

            if let Some(tid) = tex_id {
                let mut mesh = egui::Mesh::with_texture(tid);
                let uv_rect = egui::Rect::from_min_max(uv_min, uv_max);
                mesh.add_rect_with_uv(cell_rect, uv_rect, tint);
                painter.add(egui::Shape::mesh(mesh));
            } else {
                painter.rect_filled(cell_rect, 0.0, style.fallback_color());
            }
        }
    }
}
