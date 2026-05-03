//! Glow rendering for select sprites.

use eframe::egui;

use super::super::{DrawCtx, opaque_shader};
use super::SpriteDrawData;

pub fn has_glow(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.starts_with("goalarea") && n != "goalarea_night")
        || n.starts_with("boxchallenge")
        || n.starts_with("dynamicboxchallenge")
        || n.contains("starbox")
}

/// Draw glow starburst effect. Called in a separate pass before terrain to match
/// Unity/TS render order (glow renderOrder = terrainFill - 1 = behind terrain).
/// GoalArea glows bob vertically; BoxChallenge/StarBox glows are stationary.
pub fn draw_glow(
    painter: &egui::Painter,
    sprite: &SpriteDrawData,
    camera: &Camera,
    canvas_center: egui::Vec2,
    canvas_rect: egui::Rect,
    time: f64,
    glow_tex: egui::TextureId,
) {
    let name_lower = &sprite.name_lower;
    let bobs = name_lower.starts_with("goalarea");
    let y_offset = if bobs {
        (time * 3.0).sin() as f32 * 0.25
    } else {
        0.0
    };

    let center = camera.world_to_screen(
        Vec2 {
            x: sprite.world_pos.x,
            y: sprite.world_pos.y + y_offset,
        },
        canvas_center,
    );

    // Glow sprite from Particles_Sheet_01.png: 3×3 cells at grid (0,5) in 16×16 atlas
    // TS: glowFullSize = 114 * 20 / 768 ≈ 2.96875 world units
    let glow_world_size = 114.0 * 20.0 / 768.0;
    let glow_hw = glow_world_size * 0.5 * camera.zoom;
    let glow_hh = glow_world_size * 0.5 * camera.zoom;

    // Quick frustum cull
    let margin = glow_hw.max(glow_hh) + 20.0;
    if center.x + margin < canvas_rect.left()
        || center.x - margin > canvas_rect.right()
        || center.y + margin < canvas_rect.top()
        || center.y - margin > canvas_rect.bottom()
    {
        return;
    }

    let angle = (time * std::f64::consts::PI / 10.0) as f32;

    // UV rect: (0/16, 5/16) .. (3/16, 8/16) — flip Y for egui (V=0=top)
    let u0 = 0.0_f32;
    let u1 = 3.0 / 16.0;
    let v0 = 1.0 - 8.0 / 16.0; // = 0.5
    let v1 = 1.0 - 5.0 / 16.0; // = 0.6875

    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let rot = |dx: f32, dy: f32| -> egui::Pos2 {
        egui::pos2(
            center.x + dx * cos_a + dy * sin_a,
            center.y - dx * sin_a + dy * cos_a,
        )
    };

    let mut mesh = egui::Mesh::with_texture(glow_tex);
    let tl = rot(-glow_hw, -glow_hh);
    let tr = rot(glow_hw, -glow_hh);
    let br = rot(glow_hw, glow_hh);
    let bl = rot(-glow_hw, glow_hh);
    let white = egui::Color32::WHITE;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: tl,
        uv: egui::pos2(u0, v0),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: tr,
        uv: egui::pos2(u1, v0),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: br,
        uv: egui::pos2(u1, v1),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: bl,
        uv: egui::pos2(u0, v1),
        color: white,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}
