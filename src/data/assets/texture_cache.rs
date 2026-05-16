//! egui texture cache with several decode/wrap modes.

use std::collections::HashMap;

use eframe::egui;

use super::embedded::read_asset;

/// Build an `egui::ColorImage` with gamma-space premultiplied alpha.
fn color_image_premultiplied(size: [usize; 2], rgba: &[u8]) -> egui::ColorImage {
    let pixels = rgba
        .chunks_exact(4)
        .map(|p| {
            let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
            if a == 255 {
                egui::Color32::from_rgba_premultiplied(r, g, b, 255)
            } else if a == 0 {
                egui::Color32::TRANSPARENT
            } else {
                let af = a as f32 * (1.0 / 255.0);
                let rp = (r as f32 * af + 0.5) as u8;
                let gp = (g as f32 * af + 0.5) as u8;
                let bp = (b as f32 * af + 0.5) as u8;
                egui::Color32::from_rgba_premultiplied(rp, gp, bp, a)
            }
        })
        .collect();
    egui::ColorImage::new(size, pixels)
}

/// Texture cache for egui texture handles.
pub struct TextureCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Load a PNG and register it as an egui texture.
    pub fn load_texture(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let handle = ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat (tiling) wrap mode.
    pub fn load_texture_repeat(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat wrap and vertical flip.
    pub fn load_texture_repeat_flipv(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let img = image::imageops::flip_vertical(&img);
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a sprite region cropped from its atlas, keeping original RGBA pixels.
    pub fn load_sprite_crop(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        atlas_path: &str,
        uv_rect: [f32; 4],
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(key) {
            return Some(handle.id());
        }
        let [uv_x, uv_y, uv_w, uv_h] = uv_rect;
        let data = read_asset(atlas_path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let (aw, ah) = (img.width(), img.height());
        // UV → pixel coords (Unity V=0 at bottom)
        let px0 = (uv_x * aw as f32) as u32;
        let py0 = ((1.0 - uv_y - uv_h) * ah as f32) as u32;
        let pw = (uv_w * aw as f32) as u32;
        let ph = (uv_h * ah as f32) as u32;
        let crop = image::imageops::crop_imm(&img, px0, py0, pw, ph).to_image();
        let size = [crop.width() as usize, crop.height() as usize];
        let pixels: Vec<egui::Color32> = crop
            .pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        let color_image = egui::ColorImage::new(size, pixels);
        let handle = ctx.load_texture(key, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);
        Some(id)
    }

    pub fn get(&self, path: &str) -> Option<egui::TextureId> {
        self.textures.get(path).map(|h| h.id())
    }

    /// Get the pixel dimensions of a loaded texture.
    pub fn texture_size(&self, name: &str) -> Option<[usize; 2]> {
        self.textures.get(name).map(|h| h.size())
    }
}
