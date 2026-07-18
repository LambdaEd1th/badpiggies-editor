//! Grid-based `.contraption` preview rendered from the original Unity sprites.

use std::collections::HashSet;

use badpiggies_editor_core::io::save::parser::ContraptionPart;

use crate::data::assets::TextureCache;
use crate::data::{icon_db, sprite_db};
use crate::gpu2d::{self, Color32, Mesh, Pos2, Rect, Shape};
use crate::renderer::grid::ConstructionGridCellStyle;
use crate::renderer::sprite_shader;

const BASE_CELL_SIZE: f32 = 48.0;
const MAX_SCALE: f32 = 1.5;
const PREVIEW_PADDING: f32 = 24.0;
const GRID_PREVIEW_SPRITE_PAD_PX: u32 = 4;

#[derive(serde::Deserialize)]
pub struct ContraptionPreviewPayload {
    pub parts: Vec<ContraptionPart>,
    #[serde(default)]
    pub dark_mode: bool,
}

pub struct ContraptionPreview {
    parts: Vec<ContraptionPart>,
    dark_mode: bool,
    texture_cache: TextureCache,
}

impl ContraptionPreview {
    pub fn new(payload: ContraptionPreviewPayload) -> Self {
        Self {
            parts: payload.parts,
            dark_mode: payload.dark_mode,
            texture_cache: TextureCache::new(),
        }
    }

    pub fn update(&mut self, payload: ContraptionPreviewPayload) {
        self.parts = payload.parts;
        self.dark_mode = payload.dark_mode;
    }

    pub fn show(&mut self, ui: &mut gpu2d::Ui) {
        let canvas = ui.max_rect();
        let background = if self.dark_mode {
            Color32::from_rgb(21, 25, 29)
        } else {
            Color32::from_rgb(248, 248, 248)
        };
        ui.painter().rect_filled(canvas, 0.0, background);

        if self.parts.is_empty() {
            return;
        }

        let min_x = self.parts.iter().map(|part| part.x).min().unwrap_or(0);
        let max_x = self.parts.iter().map(|part| part.x).max().unwrap_or(0);
        let min_y = self.parts.iter().map(|part| part.y).min().unwrap_or(0);
        let max_y = self.parts.iter().map(|part| part.y).max().unwrap_or(0);
        let grid_width = (max_x - min_x + 1).max(1) as f32;
        let grid_height = (max_y - min_y + 1).max(1) as f32;

        let available_width = (canvas.width() - PREVIEW_PADDING * 2.0).max(1.0);
        let available_height = (canvas.height() - PREVIEW_PADDING * 2.0).max(1.0);
        let scale = (available_width / (grid_width * BASE_CELL_SIZE))
            .min(available_height / (grid_height * BASE_CELL_SIZE))
            .clamp(0.05, MAX_SCALE);
        let cell_size = BASE_CELL_SIZE * scale;
        let total_width = grid_width * cell_size;
        let total_height = grid_height * cell_size;
        let origin = gpu2d::pos2(
            canvas.center().x - total_width * 0.5,
            canvas.center().y - total_height * 0.5,
        );

        self.draw_grid(
            ui.painter(),
            origin,
            grid_width as i32,
            grid_height as i32,
            cell_size,
        );
        self.draw_parts(ui.painter(), origin, min_x, max_y, cell_size);
    }

    fn draw_grid(
        &mut self,
        painter: &gpu2d::Painter,
        origin: Pos2,
        width: i32,
        height: i32,
        cell_size: f32,
    ) {
        let style = if self.dark_mode {
            ConstructionGridCellStyle::Light
        } else {
            ConstructionGridCellStyle::Default
        };
        let Some(sprite) = sprite_db::get_sprite_info(style.sprite_name()) else {
            self.draw_line_grid(painter, origin, width, height, cell_size);
            return;
        };

        let atlas_path = format!("Assets/Texture2D/{}", sprite.atlas);
        let cache_key = format!(
            "{}_contraption_grid_premult_pad_{}",
            sprite.atlas, GRID_PREVIEW_SPRITE_PAD_PX
        );
        let Some((texture_id, uv_min, uv_max)) =
            self.texture_cache.load_sprite_crop_padded_premultiplied(
                painter.ctx(),
                &cache_key,
                &atlas_path,
                [sprite.uv.x, sprite.uv.y, sprite.uv.w, sprite.uv.h],
                GRID_PREVIEW_SPRITE_PAD_PX,
            )
        else {
            self.draw_line_grid(painter, origin, width, height, cell_size);
            return;
        };

        let (half_width, half_height) = style.half_extents();
        let sprite_size = gpu2d::vec2(cell_size * half_width * 2.0, cell_size * half_height * 2.0);
        let uv = Rect::from_min_max(uv_min, uv_max);
        let mut mesh = Mesh::with_texture(texture_id);
        for x in 0..width {
            for y in 0..height {
                let center = gpu2d::pos2(
                    origin.x + (x as f32 + 0.5) * cell_size,
                    origin.y + (y as f32 + 0.5) * cell_size,
                );
                mesh.add_rect_with_uv(
                    Rect::from_center_size(center, sprite_size),
                    uv,
                    Color32::WHITE,
                );
            }
        }
        painter.add(Shape::mesh(mesh));
    }

    fn draw_line_grid(
        &self,
        painter: &gpu2d::Painter,
        origin: Pos2,
        width: i32,
        height: i32,
        cell_size: f32,
    ) {
        let color = if self.dark_mode {
            Color32::from_rgba_premultiplied(148, 158, 168, 45)
        } else {
            Color32::from_rgba_premultiplied(88, 94, 101, 36)
        };
        let stroke = gpu2d::Stroke::new(1.0, color);
        let total_width = width as f32 * cell_size;
        let total_height = height as f32 * cell_size;
        for x in 0..=width {
            let screen_x = origin.x + x as f32 * cell_size;
            painter.line_segment(
                [
                    gpu2d::pos2(screen_x, origin.y),
                    gpu2d::pos2(screen_x, origin.y + total_height),
                ],
                stroke,
            );
        }
        for y in 0..=height {
            let screen_y = origin.y + y as f32 * cell_size;
            painter.line_segment(
                [
                    gpu2d::pos2(origin.x, screen_y),
                    gpu2d::pos2(origin.x + total_width, screen_y),
                ],
                stroke,
            );
        }
    }

    fn draw_parts(
        &mut self,
        painter: &gpu2d::Painter,
        origin: Pos2,
        min_x: i32,
        max_y: i32,
        cell_size: f32,
    ) {
        let occupied = self
            .parts
            .iter()
            .map(|part| (part.x, part.y))
            .collect::<HashSet<_>>();
        let rotated_has = |x: i32, y: i32, direction: i32, rotation: i32| {
            const OFFSETS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
            let (dx, dy) = OFFSETS[(direction + rotation).rem_euclid(4) as usize];
            occupied.contains(&(x + dx, y + dy))
        };
        let has_neighbor = |x: i32, y: i32, dx: i32, dy: i32| occupied.contains(&(x + dx, y + dy));

        struct DrawLayer<'a> {
            part: &'a ContraptionPart,
            layer: &'a icon_db::IconLayer,
            world_z: f32,
        }

        let mut layers = Vec::new();
        for part in &self.parts {
            let Some(info) = icon_db::get_part_info(part.part_type, part.custom_part_index) else {
                continue;
            };
            let part_z = -0.1 + info.z_offset - (part.x as f32 + 2.0 * part.y as f32) / 100_000.0;
            for layer in &info.layers {
                if layer_visible(part, layer, &rotated_has, &has_neighbor) {
                    layers.push(DrawLayer {
                        part,
                        layer,
                        world_z: part_z + layer.z_local,
                    });
                }
            }
        }
        layers.sort_by(|left, right| {
            right
                .world_z
                .partial_cmp(&left.world_z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for draw in layers {
            let atlas_path = format!("Assets/Texture2D/{}", draw.layer.atlas);
            let Some(texture_id) =
                self.texture_cache
                    .load_texture(painter.ctx(), &atlas_path, &draw.layer.atlas)
            else {
                continue;
            };
            let Some([atlas_width, atlas_height]) =
                self.texture_cache.texture_size(&draw.layer.atlas)
            else {
                continue;
            };

            let center = gpu2d::pos2(
                origin.x + (draw.part.x - min_x) as f32 * cell_size + cell_size * 0.5,
                origin.y + (max_y - draw.part.y) as f32 * cell_size + cell_size * 0.5,
            );
            let positions = transformed_positions(draw.part, draw.layer, center, cell_size);
            let uv = badpiggies_editor_core::data::sprite_db::UvRect {
                x: draw.layer.uv_x,
                y: draw.layer.uv_y,
                w: draw.layer.uv_w,
                h: draw.layer.uv_h,
            };
            let (uv_min, uv_max) = sprite_shader::compute_uvs(
                &uv,
                atlas_width as f32,
                atlas_height as f32,
                false,
                false,
            );
            let uvs = [
                gpu2d::pos2(uv_min[0], uv_max[1]),
                gpu2d::pos2(uv_min[0], uv_min[1]),
                gpu2d::pos2(uv_max[0], uv_min[1]),
                gpu2d::pos2(uv_max[0], uv_max[1]),
            ];
            painter.add(Shape::mesh(textured_quad(texture_id, positions, uvs)));
        }
    }
}

fn transformed_positions(
    part: &ContraptionPart,
    layer: &icon_db::IconLayer,
    center: Pos2,
    cell_size: f32,
) -> [Pos2; 4] {
    let mut vertices = [
        (layer.v0_x, layer.v0_y),
        (layer.v1_x, layer.v1_y),
        (layer.v2_x, layer.v2_y),
        (layer.v3_x, layer.v3_y),
    ];
    for (x, y) in &mut vertices {
        if part.flipped {
            *x = -*x;
        } else {
            (*x, *y) = match part.rot.rem_euclid(4) {
                1 => (-*y, *x),
                2 => (-*x, -*y),
                3 => (*y, -*x),
                _ => (*x, *y),
            };
        }
    }
    vertices.map(|(x, y)| gpu2d::pos2(center.x + x * cell_size, center.y - y * cell_size))
}

fn textured_quad(texture_id: gpu2d::TextureId, positions: [Pos2; 4], uvs: [Pos2; 4]) -> Mesh {
    let mut mesh = Mesh::with_texture(texture_id);
    for (position, uv) in positions.into_iter().zip(uvs) {
        mesh.vertices.push(gpu2d::epaint::Vertex {
            pos: position,
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    mesh
}

fn layer_visible(
    part: &ContraptionPart,
    layer: &icon_db::IconLayer,
    rotated_has: &dyn Fn(i32, i32, i32, i32) -> bool,
    has_neighbor: &dyn Fn(i32, i32, i32, i32) -> bool,
) -> bool {
    let name = layer.go_name.as_str();
    let (x, y, rotation) = (part.x, part.y, part.rot);
    match part.part_type {
        14 => {
            let up = has_neighbor(x, y, 0, 1);
            let down = has_neighbor(x, y, 0, -1);
            let left = has_neighbor(x, y, -1, 0);
            let right = has_neighbor(x, y, 1, 0);
            match name {
                "TopFrameSprite" => up || left || right,
                "BottomFrameSprite" => {
                    let top_connected = up || left || right;
                    let bottom_connected = down || left || right;
                    bottom_connected || !top_connected
                }
                _ => true,
            }
        }
        17 => {
            let up = rotated_has(x, y, 1, rotation);
            let down = rotated_has(x, y, 3, rotation);
            let left = rotated_has(x, y, 2, rotation);
            let right = rotated_has(x, y, 0, rotation);
            match name {
                "LeftAttachment" => left,
                "RightAttachment" => right,
                "TopAttachment" => up,
                "BottomAttachment" => down || (!up && !left && !right),
                _ => true,
            }
        }
        24 => {
            let up = rotated_has(x, y, 1, rotation);
            let down = rotated_has(x, y, 3, rotation);
            let left = rotated_has(x, y, 2, rotation);
            let right = rotated_has(x, y, 0, rotation);
            match name {
                "LeftAttachment" => left,
                "RightAttachment" => right,
                "TopAttachment" => up,
                "BottomAttachment" => down,
                _ => true,
            }
        }
        39 => name != "SpringVisualization",
        41 | 47 => {
            if part.part_type == 41 && name == "SpringVisualization" {
                return false;
            }
            let up = rotated_has(x, y, 1, rotation);
            let down = rotated_has(x, y, 3, rotation);
            let left = rotated_has(x, y, 2, rotation);
            let right = rotated_has(x, y, 0, rotation);
            let up_left = rotated_has(x, y, 5, rotation);
            let down_left = rotated_has(x, y, 6, rotation);
            let up_right = rotated_has(x, y, 4, rotation);
            let down_right = rotated_has(x, y, 7, rotation);
            let diagonal = (4..=7).contains(&rotation);
            let visible = match name {
                "LeftAttachment" => left && !diagonal,
                "RightAttachment" => right && !diagonal,
                "TopAttachment" => up && !diagonal,
                "BottomAttachment" => (down && !diagonal) || (!up && !left && !right && !diagonal),
                "BottomLeftAttachment" => down_left && diagonal,
                "BottomRightAttachment" => down_right && diagonal,
                "TopLeftAttachment" => up_left && diagonal,
                "TopRightAttachment" => up_right && diagonal,
                _ => return true,
            };
            visible
                || (name == "BottomAttachment"
                    && !(up || down_left || left || right || up_left || up_right || down_right))
        }
        _ => true,
    }
}
