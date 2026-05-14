//! Glow rendering for select sprites.

use eframe::egui;

use crate::data::unity_anim;
use crate::domain::types::Vec2;
use crate::goal_animation::goal_visual_state;

use super::super::{Camera, background};
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
    let goal_visual = sprite
        .name_lower
        .starts_with("goalarea")
        .then(|| goal_visual_state(sprite.goal_animation_state, time, sprite.index));
    let y_offset = goal_visual.map(|visual| visual.y_offset).unwrap_or(0.0);
    let glow_alpha = goal_visual.map(|visual| visual.alpha).unwrap_or(1.0);
    if glow_alpha <= 0.0 {
        return;
    }

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
    let glow_scale_x = goal_visual.map(|visual| visual.scale_x).unwrap_or(1.0);
    let glow_scale_y = goal_visual.map(|visual| visual.scale_y).unwrap_or(1.0);
    let glow_hw = glow_world_size * 0.5 * glow_scale_x * camera.zoom;
    let glow_hh = glow_world_size * 0.5 * glow_scale_y * camera.zoom;

    // Quick frustum cull
    let margin = glow_hw.max(glow_hh) + 20.0;
    if center.x + margin < canvas_rect.left()
        || center.x - margin > canvas_rect.right()
        || center.y + margin < canvas_rect.top()
        || center.y - margin > canvas_rect.bottom()
    {
        return;
    }

    let angle = glow_rotation_angle(time);

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
    let white = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * glow_alpha) as u8);
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

fn glow_rotation_angle(time: f64) -> f32 {
    if let Some(clip) = unity_anim::rotating_glow_clip() {
        return glow_rotation_angle_from_clip(clip, time);
    }

    (time * std::f64::consts::PI / 10.0) as f32
}

fn glow_rotation_angle_from_clip(clip: &unity_anim::UnityAnimationClip, time: f64) -> f32 {
    let sample_time = clip.sample_time(time, 0.0);

    // Prefer the quaternion curve: Unity's Euler hint contains authoring wrap/jump values,
    // while the quaternion track preserves the original smooth rotation that the old
    // hardcoded fallback approximated.
    if let Some(rotation) = clip.root_rotation() {
        let x = background::hermite(rotation.x.as_slice(), sample_time);
        let y = background::hermite(rotation.y.as_slice(), sample_time);
        let z = background::hermite(rotation.z.as_slice(), sample_time);
        let w = background::hermite(rotation.w.as_slice(), sample_time);
        return quaternion_z_angle(x, y, z, w);
    }

    if let Some(euler_z) = clip.root_float_curve("m_LocalEulerAnglesHint.z") {
        return background::hermite(euler_z, sample_time).to_radians();
    }

    (time * std::f64::consts::PI / 10.0) as f32
}

fn quaternion_z_angle(x: f32, y: f32, z: f32, w: f32) -> f32 {
    (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_rotation_uses_smooth_quaternion_curve() {
        let clip = unity_anim::rotating_glow_clip().expect("RotatingGlow clip should load");
        let angle_deg = glow_rotation_angle_from_clip(clip, 0.5).to_degrees();
        assert!(
            (-20.0..0.0).contains(&angle_deg),
            "expected smooth quaternion rotation near the legacy fallback pace, got {angle_deg}"
        );
    }
}
