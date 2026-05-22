//! Glow rendering for select sprites.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use eframe::egui;

use crate::data::assets;
use crate::data::goal_animation::goal_visual_state;
use crate::data::sprite_db::UvRect;
use crate::data::unity_anim;
use crate::domain::prefab_asset::{PrefabAssetComponent, PrefabAssetDocument};
use crate::domain::types::Vec2;

use super::super::{Camera, background};
use super::SpriteDrawData;

const UNMANAGED_SPRITE_ATLAS_SIZE: f32 = 1024.0;
const WORLD_SCALE: f32 = 10.0 / 768.0;

#[derive(Clone, Copy, Debug)]
struct GlowSpriteConfig {
    half_w: f32,
    half_h: f32,
    uv: UvRect,
    texture_name: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct GlowRenderState {
    y_offset: f32,
    alpha: f32,
    half_w: f32,
    half_h: f32,
    uv: UvRect,
}

#[cfg_attr(not(test), allow(dead_code))]
fn has_glow(name: &str) -> bool {
    glow_sprite_config(name).is_some()
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
    let Some(glow) = glow_render_state(sprite, time) else {
        return;
    };

    let center = camera.world_to_screen(
        Vec2 {
            x: sprite.world_pos.x,
            y: sprite.world_pos.y + glow.y_offset,
        },
        canvas_center,
    );

    let glow_hw = glow.half_w * camera.zoom;
    let glow_hh = glow.half_h * camera.zoom;

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

    let u0 = glow.uv.x;
    let u1 = glow.uv.x + glow.uv.w;
    let v0 = 1.0 - glow.uv.y - glow.uv.h;
    let v1 = 1.0 - glow.uv.y;

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
    let white = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * glow.alpha) as u8);
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

pub(super) fn glow_world_radius(sprite: &SpriteDrawData, time: f64) -> Option<f32> {
    let glow = glow_render_state(sprite, time)?;
    Some(glow.half_w.hypot(glow.half_h) + glow.y_offset.abs())
}

fn glow_render_state(sprite: &SpriteDrawData, time: f64) -> Option<GlowRenderState> {
    let goal_visual = sprite
        .name_lower
        .starts_with("goalarea")
        .then(|| goal_visual_state(sprite.goal_animation_state, time, sprite.index));
    let alpha = goal_visual.map(|visual| visual.alpha).unwrap_or(1.0);
    if alpha <= 0.0 {
        return None;
    }

    let glow = glow_sprite_config(&sprite.name)?;
    let instance_scale_x = sprite.scale.0.abs().max(0.01);
    let instance_scale_y = sprite.scale.1.abs().max(0.01);
    let scale_x = goal_visual
        .map(|visual| visual.scale_x)
        .unwrap_or(1.0)
        .abs();
    let scale_y = goal_visual
        .map(|visual| visual.scale_y)
        .unwrap_or(1.0)
        .abs();

    Some(GlowRenderState {
        y_offset: goal_visual
            .map(|visual| visual.y_offset * instance_scale_y)
            .unwrap_or(0.0),
        alpha,
        half_w: glow.half_w * instance_scale_x * scale_x,
        half_h: glow.half_h * instance_scale_y * scale_y,
        uv: glow.uv,
    })
}

fn glow_sprite_config(prefab_name: &str) -> Option<GlowSpriteConfig> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<GlowSpriteConfig>>>> = OnceLock::new();

    let key = prefab_name
        .split(" (")
        .next()
        .unwrap_or(prefab_name)
        .to_string();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(cached) = cache
        .lock()
        .expect("glow sprite cache poisoned")
        .get(&key)
        .copied()
    {
        return cached;
    }

    let loaded = load_glow_sprite_config(&key);
    cache
        .lock()
        .expect("glow sprite cache poisoned")
        .insert(key, loaded);
    loaded
}

fn load_glow_sprite_config(prefab_name: &str) -> Option<GlowSpriteConfig> {
    let asset_path = format!("Assets/Prefab/{prefab_name}.prefab");
    let text = assets::read_pathname_text(&asset_path)?;
    let prefab = PrefabAssetDocument::parse(&text)?;
    let world_scale = prefab.cumulative_scale_by_game_object_name("Glow")?;
    let sprite = prefab.component_by_game_object_name("Glow", "UnmanagedSprite")?;
    let renderer = prefab.component_by_game_object_name("Glow", "MeshRenderer")?;
    let uv = unmanaged_sprite_uv_rect(sprite)?;
    let [sprite_width, sprite_height] = unmanaged_sprite_pixel_size(sprite)?;
    let texture_name =
        assets::effect_texture_name_for_material_guid(renderer.field_guid("m_Materials")?)?;

    Some(GlowSpriteConfig {
        half_w: world_scale[0].abs() * sprite_width * WORLD_SCALE,
        half_h: world_scale[1].abs() * sprite_height * WORLD_SCALE,
        uv,
        texture_name,
    })
}

pub(super) fn glow_texture_name(sprite: &SpriteDrawData) -> Option<&'static str> {
    glow_sprite_config(&sprite.name).map(|glow| glow.texture_name)
}

fn unmanaged_sprite_uv_rect(sprite: &PrefabAssetComponent) -> Option<UvRect> {
    let uv_x = sprite.field_i32("m_UVx")? as f32;
    let uv_y = sprite.field_i32("m_UVy")? as f32;
    let width = sprite.field_i32("m_width")? as f32;
    let height = sprite.field_i32("m_height")? as f32;
    let subdivisions_x = sprite
        .field_i32("m_subdivisionsX")
        .or_else(|| sprite.field_i32("m_atlasGridSubdivisions"))? as f32;
    let subdivisions_y = sprite
        .field_i32("m_subdivisionsY")
        .or_else(|| sprite.field_i32("m_atlasGridSubdivisions"))? as f32;
    let border = sprite.field_i32("m_border").unwrap_or(0) as f32;
    if subdivisions_x <= 0.0 || subdivisions_y <= 0.0 {
        return None;
    }

    let cell_x = 1.0 / subdivisions_x;
    let cell_y = 1.0 / subdivisions_y;
    let half_texel = 0.5 / UNMANAGED_SPRITE_ATLAS_SIZE;
    let border_uv = border / UNMANAGED_SPRITE_ATLAS_SIZE;

    Some(UvRect {
        x: uv_x * cell_x + half_texel + border_uv,
        y: uv_y * cell_y + half_texel + border_uv,
        w: width * cell_x - 2.0 * half_texel - 2.0 * border_uv,
        h: height * cell_y - 2.0 * half_texel - 2.0 * border_uv,
    })
}

fn unmanaged_sprite_pixel_size(sprite: &PrefabAssetComponent) -> Option<[f32; 2]> {
    let border = sprite.field_i32("m_border").unwrap_or(0) as f32;

    let sprite_width = sprite
        .field_i32("m_spriteWidth")
        .map(|value| value as f32)
        .filter(|value| *value > 0.0)
        .or_else(|| {
            let scale = sprite.field_f32("m_scale")?;
            let texture_width = sprite.field_i32("m_textureWidth")? as f32;
            Some(scale.abs() * (texture_width - 2.0 * border))
        })?;

    let sprite_height = sprite
        .field_i32("m_spriteHeight")
        .map(|value| value as f32)
        .filter(|value| *value > 0.0)
        .or_else(|| {
            let scale = sprite.field_f32("m_scale")?;
            let texture_height = sprite.field_i32("m_textureHeight")? as f32;
            Some(scale.abs() * (texture_height - 2.0 * border))
        })?;

    Some([sprite_width, sprite_height])
}

fn glow_rotation_angle(time: f64) -> f32 {
    glow_rotation_angle_from_clip(
        unity_anim::rotating_glow_clip()
            .expect("RotatingGlow.anim should load from embedded assets"),
        time,
    )
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

    panic!("RotatingGlow.anim must include quaternion or Euler Z rotation curves");
}

fn quaternion_z_angle(x: f32, y: f32, z: f32, w: f32) -> f32 {
    (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::goal_animation::GoalAnimationState;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    fn test_sprite(name: &str) -> SpriteDrawData {
        SpriteDrawData {
            world_pos: crate::domain::types::Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            color: egui::Color32::WHITE,
            half_size: (1.0, 1.0),
            scale: (1.0, 1.0),
            name: name.to_string(),
            index: 0,
            is_terrain: false,
            atlas: None,
            uv: None,
            opaque_tint_color: [1.0, 1.0, 1.0, 1.0],
            is_alpha_blend: false,
            is_hidden: false,
            parent: None,
            override_text: None,
            rotation: 0.0,
            bird_phase: 0.0,
            name_lower: name.to_ascii_lowercase(),
            goal_animation_state: GoalAnimationState::Idle,
        }
    }

    fn test_sprite_with_scale(name: &str, scale_x: f32, scale_y: f32) -> SpriteDrawData {
        let mut sprite = test_sprite(name);
        sprite.scale = (scale_x, scale_y);
        sprite
    }

    #[test]
    fn glow_rotation_uses_smooth_quaternion_curve() {
        let clip = unity_anim::rotating_glow_clip().expect("RotatingGlow clip should load");
        let angle_deg = glow_rotation_angle_from_clip(clip, 0.5).to_degrees();
        assert!(
            (-20.0..0.0).contains(&angle_deg),
            "expected smooth quaternion rotation near the legacy fallback pace, got {angle_deg}"
        );
    }

    #[test]
    fn asset_driven_glow_detection_includes_cake_prefab() {
        assert!(has_glow("Cake"));
        assert!(has_glow("CakeFloating"));
    }

    #[test]
    fn asset_driven_glow_detection_keeps_goal_area_night_disabled() {
        assert!(!has_glow("GoalArea_Night"));
    }

    #[test]
    fn glow_world_radius_is_none_for_non_glow_sprite() {
        let sprite = test_sprite("TNT_Box");
        assert_eq!(glow_world_radius(&sprite, 0.0), None);
    }

    #[test]
    fn goal_area_glow_uses_embedded_unmanaged_sprite_defaults() {
        let glow = glow_sprite_config("GoalArea_MM").expect("expected GoalArea_MM glow config");

        assert_close(glow.half_w, 114.0 * WORLD_SCALE);
        assert_close(glow.half_h, 114.0 * WORLD_SCALE);
        assert_eq!(glow.texture_name, "Particles_Sheet_01.png");
        assert_close(glow.uv.x, 0.00048828125);
        assert_close(glow.uv.y, 0.31298828);
        assert_close(glow.uv.w, 0.18652344);
        assert_close(glow.uv.h, 0.18652344);
    }

    #[test]
    fn box_challenge_glow_uses_prefab_specific_unmanaged_sprite_defaults() {
        let glow = glow_sprite_config("BoxChallenge").expect("expected BoxChallenge glow config");

        assert_close(glow.half_w, 114.0 * WORLD_SCALE);
        assert_close(glow.half_h, 114.0 * WORLD_SCALE);
        assert_eq!(glow.texture_name, "Particles_Sheet_01.png");
        assert_close(glow.uv.x, 0.00048828125);
        assert_close(glow.uv.y, 0.31298828);
        assert_close(glow.uv.w, 0.18652344);
        assert_close(glow.uv.h, 0.18652344);
    }

    #[test]
    fn dynamic_star_box_glow_includes_prefab_parent_scale() {
        let glow =
            glow_sprite_config("DynamicStarBox").expect("expected DynamicStarBox glow config");

        assert_close(glow.half_w, 114.0 * WORLD_SCALE);
        assert_close(glow.half_h, 114.0 * WORLD_SCALE);
    }

    #[test]
    fn glow_render_state_respects_instance_scale() {
        let sprite = test_sprite_with_scale("BoxChallenge", 0.5, 0.25);
        let glow = glow_render_state(&sprite, 0.0).expect("expected BoxChallenge glow");

        assert_close(glow.half_w, 57.0 * WORLD_SCALE);
        assert_close(glow.half_h, 28.5 * WORLD_SCALE);
    }
}
