//! Control-texture PNG (en/de)coding for terrain curve nodes.

use crate::diagnostics::error::{AppError, AppResult};

use super::curve::CurveNode;

/// Decode raw PNG bytes to RGBA pixel data.
pub fn decode_control_png_pixels(data: &[u8]) -> Option<Vec<u8>> {
    let decoder = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let img = decoder.decode().ok()?;
    Some(img.to_rgba8().into_raw())
}

/// Encode node texture indices into a 1×N PNG (control texture).
/// Returns raw PNG bytes.
pub fn encode_control_png(nodes: &[CurveNode]) -> AppResult<Vec<u8>> {
    let n = nodes.len().max(1);
    // Control texture width = next power of two of node count
    let tex_width = n.next_power_of_two();

    let mut pixels = vec![0u8; tex_width * 4]; // RGBA
    for (i, node) in nodes.iter().enumerate() {
        let base = i * 4;
        match node.texture % 4 {
            0 => pixels[base] = 255,     // R
            1 => pixels[base + 1] = 255, // G
            2 => pixels[base + 2] = 255, // B
            3 => pixels[base + 3] = 255, // A
            _ => unreachable!(),
        }
    }

    // Encode as PNG
    let mut buf = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            encoder,
            &pixels,
            tex_width as u32,
            1,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| {
            AppError::invalid_data_key1("error_terrain_control_png_encode", error.to_string())
        })?;
    }
    Ok(buf)
}
