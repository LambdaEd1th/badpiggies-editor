//! Construction grid — parse and render the placement grid overlay.
//!
//! Parses `m_constructionGridRows` from LevelManager override data and renders
//! each available cell using the same prefab semantics as Unity:
//! default `ConstructionUI` uses `GridCell`, while an explicit
//! `m_gridCellPrefab` ObjectReference override switches to `GridCellLight`.
//!
//! In the extracted atlas assets used by the editor, the grid cell pixels need
//! to bypass the sprite shader's final `rgb *= alpha` step to recover the same
//! bright rounded-cell appearance that Unity shows. The editor therefore keeps
//! a dedicated GPU sprite mode for grid cells that preserves sampled RGB.

use eframe::egui;

use crate::data::sprite_db;
use crate::domain::types::*;
use crate::unity_runtime::components::LevelManager;
use crate::unity_runtime::scene::Scene;

use super::{LevelRenderer, sprite_shader};

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

fn parse_grid_cell_style(lm: &LevelManager) -> ConstructionGridCellStyle {
    if lm.grid_cell_prefab.is_some() {
        ConstructionGridCellStyle::Light
    } else {
        ConstructionGridCellStyle::Default
    }
}

impl LevelRenderer {
    pub(super) fn draw_construction_grid_overlay(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        canvas_rect: egui::Rect,
    ) {
        let Some(grid) = self.construction_grid.as_ref() else {
            return;
        };

        let style = grid.cell_style;
        let sprite = sprite_db::get_sprite_info(style.sprite_name());
        let Some(sprite) = sprite else {
            return;
        };

        if let (Some(resources), Some(device), Some(queue)) =
            (&self.sprite_resources, &self.wgpu_device, &self.wgpu_queue)
            && let Some(atlas) =
                self.sprite_atlas_cache
                    .get_or_load(device, queue, resources, &sprite.atlas)
        {
            let (uv_min, uv_max) = sprite_shader::compute_uvs(
                &sprite.uv,
                atlas.width as f32,
                atlas.height as f32,
                false,
                false,
            );

            let (half_w, half_h) = style.half_extents();
            // GridCell and GridCellLight need to preserve sampled RGB in the
            // editor's premultiplied compositor; the generic sprite path darkens
            // them too much, while this dedicated mode keeps the rounded shape
            // and restores the expected brighter fill.
            let mut gpu_draws = Vec::new();

            for row in 0..grid.grid_height {
                let bits = grid.rows[row as usize];
                if bits == 0 {
                    continue;
                }
                for col in 0..grid.grid_width {
                    if bits & (1 << col) == 0 {
                        continue;
                    }
                    if self.sprite_slot_counter >= sprite_shader::max_draw_slots() {
                        break;
                    }

                    let wx = grid.base_x + (grid.x_min + col) as f32;
                    let wy = grid.base_y + row as f32;

                    let center = self.camera.world_to_screen(
                        crate::domain::types::Vec2 { x: wx, y: wy },
                        canvas_center,
                    );
                    let cell_rect = egui::Rect::from_center_size(
                        center,
                        egui::vec2(
                            half_w * self.camera.zoom * 2.0,
                            half_h * self.camera.zoom * 2.0,
                        ),
                    );
                    if !cell_rect.intersects(canvas_rect) {
                        continue;
                    }

                    let slot = self.sprite_slot_counter;
                    self.sprite_slot_counter += 1;
                    gpu_draws.push(sprite_shader::SpriteBatchDraw {
                        atlas: atlas.clone(),
                        slot,
                        uniforms: sprite_shader::SpriteUniforms {
                            screen_size: [canvas_rect.width(), canvas_rect.height()],
                            camera_center: [self.camera.center.x, self.camera.center.y],
                            zoom: self.camera.zoom,
                            rotation: 0.0,
                            world_center: [wx, wy],
                            half_size: [half_w, half_h],
                            uv_min,
                            uv_max,
                            mode: sprite_shader::MODE_PREALPHA_NORMAL,
                            shine_center: 0.0,
                            tint_color: [1.0, 1.0, 1.0, 1.0],
                        },
                    });
                }
            }

            if !gpu_draws.is_empty() {
                painter.add(sprite_shader::make_sprite_batch_callback(
                    canvas_rect,
                    resources.clone(),
                    gpu_draws,
                ));
            }
        }
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
    let (scene, root) = Scene::from_override_text(text)?;
    let (_, lm) = scene.get_component_of::<LevelManager>(root)?;

    let cell_style = parse_grid_cell_style(lm);
    let rows = lm.construction_grid_rows.as_ref()?.clone();

    if rows.is_empty() {
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{ConstructionGridCellStyle, GRID_PREFAB_LOCAL_SCALE, parse_construction_grid};
    use crate::data::sprite_db;
    use crate::domain::parser::parse_level;
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };
    use std::path::Path;

    const LEVEL_MANAGER_OVERRIDE: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tArray m_constructionGridRows\n\t\t\tArraySize size = 4\n\t\t\tElement 0\n\t\t\t\tInteger data = 15\n\t\t\tElement 1\n\t\t\t\tInteger data = 15\n\t\t\tElement 2\n\t\t\t\tInteger data = 0\n\t\t\tElement 3\n\t\t\t\tInteger data = 2\n\t\tObjectReference m_gridCellPrefab = 6\n";

    #[test]
    fn parses_construction_grid_rows_and_light_cell_style_from_ast() {
        let level = LevelData {
            objects: vec![
                LevelObject::Prefab(prefab(
                    "LevelManager",
                    Vec3::default(),
                    LEVEL_MANAGER_OVERRIDE,
                )),
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

    #[test]
    fn grid_cell_styles_match_runtime_sprite_atlas_and_size() {
        for (style, atlas) in [
            (ConstructionGridCellStyle::Default, "IngameAtlas2.png"),
            (ConstructionGridCellStyle::Light, "IngameAtlas.png"),
        ] {
            let sprite = sprite_db::get_sprite_info(style.sprite_name())
                .expect("missing construction grid sprite info");
            let (half_w, half_h) = style.half_extents();
            assert_eq!(sprite.atlas, atlas);
            assert!((sprite.world_w * GRID_PREFAB_LOCAL_SCALE - half_w).abs() < 1e-6);
            assert!((sprite.world_h * GRID_PREFAB_LOCAL_SCALE - half_h).abs() < 1e-6);
        }
    }

    #[test]
    fn dump_dark_sandbox_prefab_index_6_names() {
        let level_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels/assetbundles/episode_sandbox_levels_2.unity3d/Episode_6_Dark Sandbox_data.bytes");
        let level = parse_level(std::fs::read(&level_path).expect("read dark sandbox"))
            .expect("parse dark sandbox");

        let matches: Vec<_> = level
            .objects
            .iter()
            .filter_map(|object| match object {
                LevelObject::Prefab(prefab) if prefab.prefab_index == 6 => {
                    Some((prefab.name.clone(), prefab.prefab_index))
                }
                _ => None,
            })
            .collect();

        println!("prefab_index=6 objects: {matches:?}");
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
