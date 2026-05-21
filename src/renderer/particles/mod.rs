//! Particle systems: fan wind, leaf wind, bird Zzz particles.

use std::sync::OnceLock;

use crate::data::assets;
use crate::data::unity_particles;
use crate::data::unity_anim::HermiteKey;
use crate::domain::prefab_asset::PrefabAssetDocument;
use crate::domain::types::Vec2;

use super::Camera;

mod attached;
mod fan;
mod wind;
mod zzz;

pub(crate) use attached::{
    AttachedEffectEmitter, AttachedEffectParticle, attached_effect_kind_for_sprite_name,
    attached_effect_systems, draw_attached_effect_particles,
};
pub(crate) use fan::{
    FanEmitter, FanParticle, FanState, draw_fan_particles, fan_particle_texture_name,
    reset_fan_emitter_for_build,
    start_fan_emitter_for_play,
};
pub(crate) use wind::{
    WindAreaDef, WindParticle, build_wind_area_def, draw_wind_particles,
    wind_area_particle_system_count, wind_particle_texture_name,
};
pub(super) use zzz::{ZzzParticle, draw_zzz_particles, zzz_particle_texture_name};

/// Simple pseudo-random [0, 1) from u32 seed.
pub(crate) fn pseudo_random(seed: u32) -> f32 {
    let n = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((n >> 16) & 0x7fff) as f32 / 32768.0
}

const FAN_PREFAB_ASSET: &str = "Assets/Prefab/Fan.prefab";

#[derive(Clone, Copy)]
pub(super) struct FanFieldDefaults {
    pub half_w: f32,
    pub half_h: f32,
    pub center_x: f32,
    pub center_y: f32,
}

#[derive(Clone)]
struct FanFieldProfile {
    defaults: FanFieldDefaults,
    vertical_ramp: Vec<HermiteKey>,
    horizontal_ramp: Vec<HermiteKey>,
    spinup_ramp: Vec<HermiteKey>,
}

fn fan_field_profile() -> &'static FanFieldProfile {
    static PROFILE: OnceLock<FanFieldProfile> = OnceLock::new();

    PROFILE.get_or_init(load_fan_field_profile)
}

pub(super) fn fan_field_defaults() -> FanFieldDefaults {
    fan_field_profile().defaults
}

pub(super) fn fan_field_profile_weight(local_x: f32, local_y: f32) -> f32 {
    let profile = fan_field_profile();
    let defaults = profile.defaults;
    let normalized_x = (1.0
        - ((local_x - defaults.center_x).abs() / defaults.half_w.max(f32::EPSILON)))
        .clamp(0.0, 1.0);
    let normalized_y = ((defaults.center_y + defaults.half_h - local_y)
        / (defaults.half_h * 2.0).max(f32::EPSILON))
        .clamp(0.0, 1.0);

    sample_hermite(&profile.vertical_ramp, normalized_y)
        * sample_hermite(&profile.horizontal_ramp, normalized_x)
}

pub(crate) fn fan_propeller_visual_angle(fan_angle: Option<f32>) -> f32 {
    fan_angle.unwrap_or(0.0)
}

pub(crate) fn fan_propeller_foreshorten(fan_angle: Option<f32>) -> f32 {
    fan_propeller_visual_angle(fan_angle).cos().abs()
}

pub(super) fn fan_spinup_profile_weight(time: f32) -> f32 {
    sample_hermite(&fan_field_profile().spinup_ramp, time.clamp(0.0, 1.0))
}

fn load_fan_field_profile() -> FanFieldProfile {
    let text = assets::read_pathname_text(FAN_PREFAB_ASSET)
        .expect("Fan.prefab should load from embedded assets");
    let prefab =
        PrefabAssetDocument::parse(&text).expect("Fan.prefab should parse from embedded assets");
    let collider = prefab
        .root_component("BoxCollider")
        .expect("Fan.prefab must include a root BoxCollider");
    let size = collider
        .field_vec3("m_Size")
        .expect("Fan.prefab BoxCollider must include m_Size");
    let center = collider
        .field_vec3("m_Center")
        .expect("Fan.prefab BoxCollider must include m_Center");
    let fan = prefab
        .root_component("Fan")
        .expect("Fan.prefab must include a root Fan component");
    let vertical_ramp = fan
        .field_curve("verticalRamp")
        .expect("Fan.prefab Fan component must include verticalRamp");
    let horizontal_ramp = fan
        .field_curve("horizontalRamp")
        .expect("Fan.prefab Fan component must include horizontalRamp");
    let spinup_ramp = fan
        .field_curve("spinupRamp")
        .expect("Fan.prefab Fan component must include spinupRamp");

    FanFieldProfile {
        defaults: FanFieldDefaults {
            half_w: size[0].abs() * 0.5,
            half_h: size[1].abs() * 0.5,
            center_x: center[0],
            center_y: center[1],
        },
        vertical_ramp,
        horizontal_ramp,
        spinup_ramp,
    }
}

fn sample_hermite(keys: &[HermiteKey], time: f32) -> f32 {
    let count = keys.len();
    if count == 0 {
        return 1.0;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    if time >= keys[count - 1].0 {
        return keys[count - 1].1;
    }

    let mut index = 0;
    while index < count - 2 && keys[index + 1].0 < time {
        index += 1;
    }

    let (t0, v0, _, out_slope) = keys[index];
    let (t1, v1, in_slope, _) = keys[index + 1];
    let dt = (t1 - t0).max(f32::EPSILON);
    let s = (time - t0) / dt;
    let s2 = s * s;
    let s3 = s2 * s;

    (2.0 * s3 - 3.0 * s2 + 1.0) * v0
        + (s3 - 2.0 * s2 + s) * (out_slope * dt)
        + (-2.0 * s3 + 3.0 * s2) * v1
        + (s3 - s2) * (in_slope * dt)
}

pub(super) fn particle_sheet_uv_rect(
    tiles_x: f32,
    tiles_y: f32,
    row_index: u32,
    frame_index: u8,
) -> (f32, f32, f32, f32) {
    let col = frame_index as f32;
    let row = row_index as f32;
    let u0 = col / tiles_x;
    let u1 = (col + 1.0) / tiles_x;
    let v0 = row / tiles_y;
    let v1 = (row + 1.0) / tiles_y;
    (u0, u1, v0, v1)
}

pub(super) fn rotate_vec2(vec: Vec2, angle: f32) -> Vec2 {
    let cos_r = angle.cos();
    let sin_r = angle.sin();
    Vec2 {
        x: vec.x * cos_r - vec.y * sin_r,
        y: vec.x * sin_r + vec.y * cos_r,
    }
}

pub(super) fn sample_particle_world_velocity_xy(
    system: &unity_particles::UnityParticleSystemDef,
    life_t: f32,
    x_random: f32,
    y_random: f32,
    z_random: f32,
) -> Vec2 {
    let vx = system.velocity_x.sample(life_t, x_random);
    let vy = system.velocity_y.sample(life_t, y_random);
    let vz = system.velocity_z.sample(life_t, z_random);
    if system.velocity_world_space {
        return Vec2 { x: vx, y: vy };
    }
    let right = system.projected_right_xy();
    let up = system.projected_up_xy();
    let forward = system.projected_forward_xy();
    Vec2 {
        x: right.x * vx + up.x * vy + forward.x * vz,
        y: right.y * vx + up.y * vy + forward.y * vz,
    }
}

pub(super) fn sample_particle_world_force_xy(
    system: &unity_particles::UnityParticleSystemDef,
    life_t: f32,
    x_random: f32,
    y_random: f32,
    z_random: f32,
) -> Vec2 {
    let fx = system.force_x.sample(life_t, x_random);
    let fy = system.force_y.sample(life_t, y_random);
    let fz = system.force_z.sample(life_t, z_random);
    if system.force_world_space {
        return Vec2 { x: fx, y: fy };
    }
    let right = system.projected_right_xy();
    let up = system.projected_up_xy();
    let forward = system.projected_forward_xy();
    Vec2 {
        x: right.x * fx + up.x * fy + forward.x * fz,
        y: right.y * fx + up.y * fy + forward.y * fz,
    }
}

use super::LevelRenderer;

impl LevelRenderer {
    /// Advance all particle systems by `dt` seconds (fan, wind, zzz).
    pub(super) fn update_particles(&mut self, dt: f32) {
        // ── Fan: state machine + particle spawn/update ──
        self.update_fan_particles(dt);

        // ── Wind leaf particle update ──
        self.update_wind_particles(dt);

        // ── Attached effect particle update (rocket, turbo, magnet) ──
        self.update_attached_effect_particles(dt);

        // ── Zzz particle update (bird sleeping) ──
        self.update_zzz_particles(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fan_field_defaults, fan_field_profile_weight, fan_propeller_foreshorten,
        fan_propeller_visual_angle, fan_spinup_profile_weight, particle_sheet_uv_rect,
    };

    #[test]
    fn fan_field_defaults_match_embedded_prefab() {
        let defaults = fan_field_defaults();

        assert!((defaults.half_w - 1.905).abs() < 0.0001);
        assert!((defaults.half_h - 4.8251266).abs() < 0.0001);
        assert!(defaults.center_x.abs() < 0.0001);
        assert!((defaults.center_y - 4.825125).abs() < 0.0001);
    }

    #[test]
    fn fan_field_profile_matches_unity_ramp_orientation() {
        let defaults = fan_field_defaults();

        assert_eq!(fan_field_profile_weight(defaults.center_x, defaults.center_y + defaults.half_h), 0.0);
        assert!((fan_field_profile_weight(defaults.center_x, defaults.center_y) - 0.25).abs() < 0.0001);
        assert!((fan_field_profile_weight(defaults.center_x, defaults.center_y - defaults.half_h) - 1.0).abs() < 0.0001);
        assert_eq!(fan_field_profile_weight(defaults.center_x + defaults.half_w, defaults.center_y), 0.0);
    }

    #[test]
    fn fan_spinup_profile_matches_embedded_prefab_curve() {
        assert_eq!(fan_spinup_profile_weight(0.0), 0.0);
        assert!((fan_spinup_profile_weight(0.5) - 0.25).abs() < 0.0001);
        assert_eq!(fan_spinup_profile_weight(1.0), 1.0);
    }

    #[test]
    fn fan_propeller_visual_angle_defaults_to_idle_pose_without_runtime_state() {
        assert_eq!(fan_propeller_visual_angle(None), 0.0);
        assert_eq!(fan_propeller_visual_angle(Some(1.25)), 1.25);
    }

    #[test]
    fn fan_propeller_foreshorten_matches_exact_y_axis_projection() {
        assert_eq!(fan_propeller_foreshorten(None), 1.0);
        assert!(fan_propeller_foreshorten(Some(std::f32::consts::FRAC_PI_2)) < 1e-6);
        assert!((fan_propeller_foreshorten(Some(std::f32::consts::PI)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn particle_sheet_uv_rect_matches_previous_correct_hardcoded_rows() {
        let fan = particle_sheet_uv_rect(8.0, 8.0, 0, 3);
        assert_eq!(fan, (3.0 / 8.0, 4.0 / 8.0, 0.0 / 8.0, 1.0 / 8.0));

        let zzz = particle_sheet_uv_rect(8.0, 8.0, 2, 6);
        assert_eq!(zzz, (6.0 / 8.0, 7.0 / 8.0, 2.0 / 8.0, 3.0 / 8.0));

        let wind = particle_sheet_uv_rect(16.0, 16.0, 2, 4);
        assert_eq!(wind, (4.0 / 16.0, 5.0 / 16.0, 2.0 / 16.0, 3.0 / 16.0));
    }
}
