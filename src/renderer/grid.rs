//! Construction grid — parse and render the placement grid overlay.
//!
//! Parses `m_constructionGridRows` from LevelManager override data and renders
//! each available cell using the GridCellLight sprite from IngameAtlas.png.
//!
//! Unity rendering: shader `_Custom/Unlit_ColorTransparent_Geometry` applies
//! `tex2D(_MainTex, uv) * _Color` with `_Color = (1,1,1,1)` and standard
//! alpha blending (`Blend SrcAlpha OneMinusSrcAlpha`).  The sprite's own RGBA
//! pixels determine the final visual — GridCellLight is RGBA≈(48,48,48,64).

use eframe::egui;

use crate::assets::TextureCache;
use crate::sprite_db;
use crate::types::*;

use super::Camera;

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

    // Find m_constructionGridRows array and its size
    let size_pattern = "m_constructionGridRows";
    let grid_start = text.find(size_pattern)?;
    let after = &text[grid_start..];

    // Parse "ArraySize size = N"
    let size_match = after.find("size = ")?;
    let after_size = &after[size_match + 7..];
    let end = after_size.find(|c: char| !c.is_ascii_digit())?;
    let row_count: usize = after_size[..end].parse().ok()?;

    if row_count == 0 {
        return None;
    }

    // Extract "Integer data = N" values
    let mut rows = Vec::with_capacity(row_count);
    let mut search = after_size;
    for _ in 0..row_count {
        let data_pos = search.find("data = ")?;
        let after_data = &search[data_pos + 7..];
        let end = after_data
            .find(|c: char| !c.is_ascii_digit() && c != '-')
            .unwrap_or(after_data.len());
        let val: i32 = after_data[..end].parse().ok()?;
        rows.push(val);
        search = &after_data[end..];
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
    })
}

/// Draw the construction grid matching Unity's rendering.
///
/// Unity shader: `output = tex2D(_MainTex, uv) * _Color` with `_Color = (1,1,1,1)`,
/// blend mode `Blend SrcAlpha OneMinusSrcAlpha`.  The sprite texture's own RGBA
/// values determine the final appearance — GridCellLight is a rounded-corner square
/// filled with RGBA≈(48,48,48,64), producing a subtle dark overlay.
///
/// Grid cell world size: sprite_px × prefabScale(0.3) × 10/768
pub fn draw_construction_grid(
    painter: &egui::Painter,
    grid: &ConstructionGrid,
    camera: &Camera,
    canvas_center: egui::Vec2,
    canvas_rect: egui::Rect,
    tex_cache: &mut TextureCache,
    ctx: &egui::Context,
) {
    // Load the GridCellLight sprite directly from IngameAtlas.png, preserving
    // original RGBA pixels.  This matches Unity's `tex2D * _Color(1,1,1,1)`.
    //
    // NOTE: GridCell (on Ingame_Characters_Sheet_01) has mesh-based UVs that
    // can't be represented as a simple rect crop, so we use GridCellLight for
    // both grid variants.  GridCellLight RGBA≈(48,48,48,64) naturally pulls
    // any background toward gray — darkening light backgrounds and slightly
    // lightening dark ones.
    let sprite = sprite_db::get_sprite_info("GridCellLight");

    let tex_id = if let Some(s) = sprite {
        let uv = &s.uv;
        let atlas_path = format!("sprites/{}", s.atlas);
        tex_cache.load_sprite_crop(
            ctx,
            "GridCellLight_raw",
            &atlas_path,
            uv.x,
            uv.y,
            uv.w,
            uv.h,
        )
    } else {
        None
    };
    let uv_min = egui::pos2(0.0, 0.0);
    let uv_max = egui::pos2(1.0, 1.0);

    // Cell half-extents: GridCellLight is 104×105 px, prefab scale = 0.3
    let half_w = 104.0 * 0.3 * 10.0 / 768.0;
    let half_h = 105.0 * 0.3 * 10.0 / 768.0;

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

            let center = camera.world_to_screen(crate::types::Vec2 { x: wx, y: wy }, canvas_center);

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
                // Fallback: approximate GridCellLight's visual — RGBA(48,48,48,64)
                let fallback = egui::Color32::from_rgba_unmultiplied(48, 48, 48, 64);
                painter.rect_filled(cell_rect, 0.0, fallback);
            }
        }
    }
}
