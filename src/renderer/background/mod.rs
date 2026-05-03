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

use super::{DrawCtx, LevelRenderer, clouds};

impl LevelRenderer {
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
