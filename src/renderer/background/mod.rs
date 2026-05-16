//! Background rendering — sky color, ground fill, parallax sprite layers.
//!
//! Draws the scene backdrop with up to 7 parallax layers per theme. Each layer
//! has a speed factor controlling how much it shifts relative to the camera.
//! Fill sprites are solid-color rectangles, atlas sprites use UV-mapped textures.

mod cache;
mod draw;

#[cfg(test)]
mod tests;

pub use cache::{BgGpuState, BgLayerCache, build_bg_layer_cache};
pub(in crate::renderer) use draw::hermite;
pub use draw::{draw_background, draw_bg_layers};

use eframe::egui;

use crate::data::bg_data;

use super::{DrawCtx, LevelRenderer, clouds};

impl LevelRenderer {
    pub(in crate::renderer) fn draw_bg_sprite_index(
        &mut self,
        sprite_index: usize,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let Some(theme_name) = self.bg_theme else {
            return;
        };
        let Some(ref cache) = self.bg_layer_cache else {
            return;
        };
        let Some(theme) = bg_data::get_theme(theme_name) else {
            return;
        };

        let sprites = cache.sprites(theme);
        if sprite_index >= sprites.len() {
            return;
        }

        let mut gpu_state = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
            (Some(r), Some(d), Some(q)) => Some(BgGpuState {
                resources: r.clone(),
                atlas_cache: &mut self.bg_atlas_cache,
                device: d,
                queue: q,
                slot_counter: &mut self.bg_slot_counter,
            }),
            _ => None,
        };
        let mut gpu = gpu_state.as_mut();

        draw::draw_bg_sprite(
            &DrawCtx {
                painter,
                camera: &self.camera,
                canvas_center,
                canvas_rect: rect,
                tex_cache: &self.tex_cache,
            },
            self.time,
            sprites,
            sprite_index,
            cache,
            &mut gpu,
        );
    }

    pub(super) fn draw_ground_bg_and_decorative_terrain(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let decorative_queue: Vec<(f32, usize)> = self
            .terrain_data
            .iter()
            .enumerate()
            .filter_map(|(terrain_index, terrain)| {
                terrain.decorative.then_some((terrain.world_z, terrain_index))
            })
            .collect();

        let ground_bg_queue: Vec<(f32, usize)> = if self.show_bg {
            if let (Some(theme_name), Some(cache)) = (self.bg_theme, self.bg_layer_cache.as_ref()) {
                if let Some(theme) = bg_data::get_theme(theme_name) {
                    let sprites = cache.sprites(theme);
                    cache
                        .sorted_indices
                        .iter()
                        .filter_map(|&sprite_index| {
                            let world_z = sprites[sprite_index].world_z;
                            (world_z >= 0.0 && world_z < 5.0).then_some((world_z, sprite_index))
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let mut terrain_cursor = 0usize;
        let mut bg_cursor = 0usize;

        while terrain_cursor < decorative_queue.len() || bg_cursor < ground_bg_queue.len() {
            match (
                decorative_queue.get(terrain_cursor).copied(),
                ground_bg_queue.get(bg_cursor).copied(),
            ) {
                // Equal-Z tie goes to terrain first, so ground background draws later
                // and stays on top of decorative terrain like the original pass order.
                (Some((terrain_z, terrain_index)), Some((bg_z, _))) if terrain_z >= bg_z => {
                    self.draw_terrain_index(terrain_index, painter, canvas_center, rect);
                    terrain_cursor += 1;
                }
                (Some(_), Some((_, bg_sprite_index))) => {
                    self.draw_bg_sprite_index(bg_sprite_index, painter, canvas_center, rect);
                    bg_cursor += 1;
                }
                (Some((_, terrain_index)), None) => {
                    self.draw_terrain_index(terrain_index, painter, canvas_center, rect);
                    terrain_cursor += 1;
                }
                (None, Some((_, bg_sprite_index))) => {
                    self.draw_bg_sprite_index(bg_sprite_index, painter, canvas_center, rect);
                    bg_cursor += 1;
                }
                (None, None) => break,
            }
        }
    }

    /// Draw background layers for a Z range, constructing BgGpuState if available.
    pub(super) fn draw_bg_z_range(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        z_range: (f32, f32),
    ) {
        let Some(theme_name) = self.bg_theme else {
            return;
        };
        let Some(ref cache) = self.bg_layer_cache else {
            return;
        };
        let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
            (Some(r), Some(d), Some(q)) => Some(BgGpuState {
                resources: r.clone(),
                atlas_cache: &mut self.bg_atlas_cache,
                device: d,
                queue: q,
                slot_counter: &mut self.bg_slot_counter,
            }),
            _ => None,
        };
        draw_bg_layers(
            &DrawCtx {
                painter,
                camera: &self.camera,
                canvas_center,
                canvas_rect: rect,
                tex_cache: &self.tex_cache,
            },
            theme_name,
            self.time,
            z_range,
            cache,
            gpu.as_mut(),
        );
    }

    /// Draw sky background, parallax layers interleaved with clouds, and ground bg.
    pub(super) fn draw_background_all(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        dt: f32,
    ) {
        draw_background(painter, rect, &self.camera, canvas_center, self.bg_theme);

        // Reset per-frame shader slot counters
        self.bg_slot_counter = 0;
        self.sprite_slot_counter = 0;
        self.fill_slot_counter = 0;

        // Parallax background layers: sky (worldZ >= 17.5, before cloud instances)
        self.draw_bg_z_range(painter, canvas_center, rect, (17.5, f32::INFINITY));

        // Parallax background layers + clouds: (5 <= worldZ < 17.5)
        {
            let cloud_z_min = self
                .cloud_instances
                .iter()
                .map(|c| c.z)
                .fold(f32::INFINITY, f32::min);
            let cloud_z_max = self
                .cloud_instances
                .iter()
                .map(|c| c.z)
                .fold(f32::NEG_INFINITY, f32::max);

            if !self.cloud_instances.is_empty() {
                self.draw_bg_z_range(painter, canvas_center, rect, (cloud_z_max, 17.5));
                clouds::update_and_draw_clouds(
                    &mut self.cloud_instances,
                    dt,
                    &self.camera,
                    painter,
                    canvas_center,
                    rect,
                    &self.tex_cache,
                );
                self.draw_bg_z_range(painter, canvas_center, rect, (5.0, cloud_z_min));
            } else {
                self.draw_bg_z_range(painter, canvas_center, rect, (5.0, 17.5));
                clouds::update_cloud_positions(&mut self.cloud_instances, dt);
            }
        }
    }
}
