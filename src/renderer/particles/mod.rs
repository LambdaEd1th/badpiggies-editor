//! Particle systems: fan wind, leaf wind, bird Zzz particles.

use crate::data::unity_particles;
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
    FanEmitter, FanParticle, FanState, draw_fan_particles, reset_fan_emitter_for_build,
    start_fan_emitter_for_play,
};
pub(crate) use wind::{
    WindAreaDef, WindParticle, build_wind_area_def, draw_wind_particles,
    wind_area_particle_system_count,
};
pub(super) use zzz::{ZzzParticle, draw_zzz_particles};

/// Simple pseudo-random [0, 1) from u32 seed.
pub(crate) fn pseudo_random(seed: u32) -> f32 {
    let n = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((n >> 16) & 0x7fff) as f32 / 32768.0
}

pub(super) const FAN_FIELD_HALF_W: f32 = 1.905;
pub(super) const FAN_FIELD_HALF_H: f32 = 4.8251266;
pub(super) const FAN_FIELD_CENTER_Y: f32 = 4.825125;

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
    use super::particle_sheet_uv_rect;

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
