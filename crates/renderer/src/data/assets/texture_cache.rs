//! GPU texture cache with several decode/wrap modes.

use std::collections::HashMap;

use super::read_pathname;

/// Build an `crate::gpu2d::ColorImage` with gamma-space premultiplied alpha.
fn color_image_premultiplied(size: [usize; 2], rgba: &[u8]) -> crate::gpu2d::ColorImage {
    let pixels = rgba
        .chunks_exact(4)
        .map(|p| {
            let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
            if a == 255 {
                crate::gpu2d::Color32::from_rgba_premultiplied(r, g, b, 255)
            } else if a == 0 {
                crate::gpu2d::Color32::TRANSPARENT
            } else {
                let af = a as f32 * (1.0 / 255.0);
                let rp = (r as f32 * af + 0.5) as u8;
                let gp = (g as f32 * af + 0.5) as u8;
                let bp = (b as f32 * af + 0.5) as u8;
                crate::gpu2d::Color32::from_rgba_premultiplied(rp, gp, bp, a)
            }
        })
        .collect();
    crate::gpu2d::ColorImage::new(size, pixels)
}

fn color_image_from_premultiplied_rgba(size: [usize; 2], rgba: &[u8]) -> crate::gpu2d::ColorImage {
    let pixels = rgba
        .chunks_exact(4)
        .map(|p| crate::gpu2d::Color32::from_rgba_premultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    crate::gpu2d::ColorImage::new(size, pixels)
}

/// Texture cache for renderer texture handles.
pub struct TextureCache {
    textures: HashMap<String, crate::gpu2d::TextureHandle>,
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Load a PNG and register it as a GPU texture.
    pub fn load_texture(
        &mut self,
        ctx: &crate::gpu2d::Context,
        path: &str,
        name: &str,
    ) -> Option<crate::gpu2d::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_pathname(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let handle = ctx.load_texture(name, color_image, crate::gpu2d::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat (tiling) wrap mode.
    pub fn load_texture_repeat(
        &mut self,
        ctx: &crate::gpu2d::Context,
        path: &str,
        name: &str,
    ) -> Option<crate::gpu2d::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_pathname(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = crate::gpu2d::TextureOptions {
            wrap_mode: crate::gpu2d::TextureWrapMode::Repeat,
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat wrap and vertical flip.
    pub fn load_texture_repeat_flipv(
        &mut self,
        ctx: &crate::gpu2d::Context,
        path: &str,
        name: &str,
    ) -> Option<crate::gpu2d::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_pathname(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let img = image::imageops::flip_vertical(&img);
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = crate::gpu2d::TextureOptions {
            wrap_mode: crate::gpu2d::TextureWrapMode::Repeat,
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a sprite region cropped from its atlas, keeping original RGBA pixels.
    pub fn load_sprite_crop(
        &mut self,
        ctx: &crate::gpu2d::Context,
        key: &str,
        atlas_path: &str,
        uv_rect: [f32; 4],
    ) -> Option<crate::gpu2d::TextureId> {
        if let Some(handle) = self.textures.get(key) {
            return Some(handle.id());
        }
        let [uv_x, uv_y, uv_w, uv_h] = uv_rect;
        let data = read_pathname(atlas_path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let (aw, ah) = (img.width(), img.height());
        // UV → pixel coords (Unity V=0 at bottom)
        let px0 = (uv_x * aw as f32) as u32;
        let py0 = ((1.0 - uv_y - uv_h) * ah as f32) as u32;
        let pw = (uv_w * aw as f32) as u32;
        let ph = (uv_h * ah as f32) as u32;
        let crop = image::imageops::crop_imm(&img, px0, py0, pw, ph).to_image();
        let size = [crop.width() as usize, crop.height() as usize];
        let pixels: Vec<crate::gpu2d::Color32> = crop
            .pixels()
            .map(|p| crate::gpu2d::Color32::from_rgba_unmultiplied(p.0[0], p.0[1], p.0[2], p.0[3]))
            .collect();
        let color_image = crate::gpu2d::ColorImage::new(size, pixels);
        let handle = ctx.load_texture(key, color_image, crate::gpu2d::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);
        Some(id)
    }

    /// Load a sprite region cropped from its atlas with transparent padding on
    /// all sides, interpreting source RGB as already premultiplied by alpha.
    pub fn load_sprite_crop_padded_premultiplied(
        &mut self,
        ctx: &crate::gpu2d::Context,
        key: &str,
        atlas_path: &str,
        uv_rect: [f32; 4],
        pad_px: u32,
    ) -> Option<(
        crate::gpu2d::TextureId,
        crate::gpu2d::Pos2,
        crate::gpu2d::Pos2,
    )> {
        if let Some(handle) = self.textures.get(key) {
            let size = handle.size();
            let tex_w = size[0] as f32;
            let tex_h = size[1] as f32;
            let uv_min = crate::gpu2d::pos2(pad_px as f32 / tex_w, pad_px as f32 / tex_h);
            let uv_max = crate::gpu2d::pos2(
                (tex_w - pad_px as f32) / tex_w,
                (tex_h - pad_px as f32) / tex_h,
            );
            return Some((handle.id(), uv_min, uv_max));
        }

        let [uv_x, uv_y, uv_w, uv_h] = uv_rect;
        let data = read_pathname(atlas_path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let (aw, ah) = (img.width(), img.height());
        let px0 = (uv_x * aw as f32) as u32;
        let py0 = ((1.0 - uv_y - uv_h) * ah as f32) as u32;
        let pw = (uv_w * aw as f32) as u32;
        let ph = (uv_h * ah as f32) as u32;
        let crop = image::imageops::crop_imm(&img, px0, py0, pw, ph).to_image();

        let tex_w = pw + pad_px * 2;
        let tex_h = ph + pad_px * 2;
        let mut padded = image::RgbaImage::from_pixel(tex_w, tex_h, image::Rgba([0, 0, 0, 0]));
        image::imageops::replace(&mut padded, &crop, pad_px.into(), pad_px.into());

        let size = [padded.width() as usize, padded.height() as usize];
        let color_image = color_image_from_premultiplied_rgba(size, &padded.into_raw());
        let handle = ctx.load_texture(key, color_image, crate::gpu2d::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);

        let tex_w = tex_w as f32;
        let tex_h = tex_h as f32;
        let uv_min = crate::gpu2d::pos2(pad_px as f32 / tex_w, pad_px as f32 / tex_h);
        let uv_max = crate::gpu2d::pos2((pad_px + pw) as f32 / tex_w, (pad_px + ph) as f32 / tex_h);
        Some((id, uv_min, uv_max))
    }

    pub fn get(&self, path: &str) -> Option<crate::gpu2d::TextureId> {
        self.textures.get(path).map(|h| h.id())
    }

    /// Get the pixel dimensions of a loaded texture.
    pub fn texture_size(&self, name: &str) -> Option<[usize; 2]> {
        self.textures.get(name).map(|h| h.size())
    }
}
