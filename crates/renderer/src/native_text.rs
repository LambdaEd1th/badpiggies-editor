use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};

pub(crate) struct RasterizedText {
    pub width: usize,
    pub height: usize,
    pub alpha: Vec<u8>,
}

pub(crate) struct NativeTextRasterizer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl NativeTextRasterizer {
    pub(crate) fn new() -> Option<Self> {
        let font_system = FontSystem::new();
        font_system.db().faces().next()?;
        Some(Self {
            font_system,
            swash_cache: SwashCache::new(),
        })
    }

    pub(crate) fn rasterize(&mut self, text: &str, size: f32) -> Option<RasterizedText> {
        const PADDING: i32 = 2;

        let font_size = size.max(1.0);
        let line_height = (font_size * 1.4).max(font_size + 1.0);
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
        {
            let mut borrowed = buffer.borrow_with(&mut self.font_system);
            borrowed.set_size(None, None);
            borrowed.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        }
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut content_width = 1.0_f32;
        let mut content_height = line_height;
        for run in buffer.layout_runs() {
            content_width = content_width.max(run.line_w);
            content_height = content_height.max(run.line_top + run.line_height);
        }
        let width = (content_width.ceil() as i32 + PADDING * 2).max(1) as usize;
        let height = (content_height.ceil() as i32 + PADDING * 2).max(1) as usize;
        let mut alpha = vec![0_u8; width * height];

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgb(255, 255, 255),
            |x, y, glyph_width, glyph_height, color| {
                let value = color.a();
                for local_y in 0..glyph_height as i32 {
                    let target_y = y + local_y + PADDING;
                    if target_y < 0 || target_y >= height as i32 {
                        continue;
                    }
                    for local_x in 0..glyph_width as i32 {
                        let target_x = x + local_x + PADDING;
                        if target_x < 0 || target_x >= width as i32 {
                            continue;
                        }
                        let index = target_y as usize * width + target_x as usize;
                        alpha[index] = alpha[index].max(value);
                    }
                }
            },
        );

        Some(RasterizedText {
            width,
            height,
            alpha,
        })
    }
}
