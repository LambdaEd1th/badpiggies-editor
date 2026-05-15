//! Particle systems: fan wind, leaf wind, bird Zzz particles.

use eframe::egui;

use crate::data::unity_particles;
use crate::data::unity_particles::UnityParticleSystemDef;
use crate::domain::types::Vec2;

use super::Camera;

mod attached;
mod fan;
mod zzz;

pub(crate) use attached::{
    AttachedEffectEmitter, AttachedEffectParticle, attached_effect_kind_for_sprite_name,
    attached_effect_systems, draw_attached_effect_particles,
};
pub(crate) use fan::{
    FanEmitter, FanParticle, FanState, draw_fan_particles, reset_fan_emitter_for_build,
    start_fan_emitter_for_play,
};
pub(super) use zzz::{ZzzParticle, draw_zzz_particles};

/// Simple pseudo-random [0, 1) from u32 seed.
pub(super) fn pseudo_random(seed: u32) -> f32 {
    let n = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((n >> 16) & 0x7fff) as f32 / 32768.0
}

/// Wind area zone definition.
#[derive(Clone, Copy, Debug)]
pub(super) struct WindAreaDef {
    pub sprite_index: usize,
    pub center_x: f32,
    pub center_y: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub power_factor: f32,
}

pub(super) const WIND_AREA_HALF_W: f32 = 20.0;
pub(super) const WIND_AREA_HALF_H: f32 = 7.5;
pub(super) const WIND_AREA_POWER_FACTOR: f32 = 1.5;
pub(super) const FAN_FIELD_HALF_W: f32 = 1.905;
pub(super) const FAN_FIELD_HALF_H: f32 = 4.8251266;
pub(super) const FAN_FIELD_CENTER_Y: f32 = 4.825125;

/// Leaf UV frame within 16×16 Particles_Sheet_01 atlas.
/// Row 2 from top → UV Y = 13/16, columns 4/5/6.

/// A single wind leaf particle.
pub(super) struct WindParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub side_x: f32,
    pub side_y: f32,
    pub age: f32,
    pub lifetime: f32,
    pub rot: f32,
    pub rot_speed: f32,
    pub size: f32,
    /// Sheet column chosen by the prefab UV module.
    pub leaf_col: u8,
    pub source_sprite_index: usize,
    pub source_system_index: usize,
}

fn wind_area_prefab() -> &'static unity_particles::WindAreaParticlePrefab {
    unity_particles::wind_area_prefab().expect("WindArea particle prefab should be available")
}

fn wind_area_particle_systems() -> &'static [UnityParticleSystemDef] {
    &wind_area_prefab().systems
}

pub(super) fn wind_area_particle_system_count() -> usize {
    wind_area_particle_systems()
        .iter()
        .filter(|system| system.name.starts_with("WindEffect"))
        .count()
}

fn wind_area_particle_system() -> &'static unity_particles::UnityParticleSystemDef {
    wind_area_particle_systems()
        .iter()
        .find(|system| system.name.starts_with("WindEffect"))
        .expect("WindArea should contain a WindEffect particle system")
}

fn wind_area_particle_spawn_rate() -> f32 {
    let system = wind_area_particle_system();
    if !system.play_on_awake && !system.looping {
        0.0
    } else {
        system.emission_rate.sample(0.0, 0.0)
    }
}

fn wind_area_particle_max_count() -> usize {
    wind_area_particle_system().max_particles
}

fn wind_area_particle_prewarm_count() -> usize {
    let system = wind_area_particle_system();
    if !system.prewarm {
        0
    } else {
        let average_lifetime =
            (system.start_lifetime.sample(0.0, 0.0) + system.start_lifetime.sample(0.0, 1.0))
                * 0.5;
        let seeded = (wind_area_particle_spawn_rate() * average_lifetime).round() as usize;
        seeded.max(1).min(wind_area_particle_max_count())
    }
}

fn wind_particle_default_power() -> f32 {
    wind_area_prefab().power_factor
}

pub(super) fn wind_area_local_direction() -> Vec2 {
    normalize_vec2(wind_area_prefab().wind_direction)
}

fn wind_particle_power_scale(power_factor: f32) -> f32 {
    (power_factor / wind_particle_default_power()).max(0.25)
}

fn wind_particle_emitter_offsets() -> (f32, f32) {
    let prefab = wind_area_prefab();
    let system = wind_area_particle_system();
    let dir = normalize_vec2(prefab.wind_direction);
    let side = Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let local = Vec2 {
        x: system.local_position.x,
        y: system.local_position.z,
    };
    (dot2(local, dir), dot2(local, side))
}

fn wind_particle_shape_half_extents() -> (f32, f32) {
    let system = wind_area_particle_system();
    let extents = system.projected_ellipsoid_half_extents_xz();
    (extents.x.max(f32::EPSILON), extents.y.max(f32::EPSILON))
}

fn wind_particle_lifetime(random: f32) -> f32 {
    wind_area_particle_system().start_lifetime.sample(0.0, random)
}

fn wind_particle_start_speed(random: f32) -> f32 {
    wind_area_particle_system().start_speed.sample(0.0, random)
}

fn wind_particle_start_size(random: f32) -> f32 {
    wind_area_particle_system().start_size.sample(0.0, random)
}

fn wind_particle_start_rotation(random: f32) -> f32 {
    wind_area_particle_system().start_rotation.sample(0.0, random)
}

fn wind_particle_rotation_speed(random: f32) -> f32 {
    wind_area_particle_system()
        .rotation_over_lifetime
        .sample(0.0, random)
}

fn wind_particle_uv_column(random: f32) -> u8 {
    wind_area_particle_system().uv_module.sample_frame_index(random) as u8
}

fn wind_particle_uv_layout() -> (f32, f32, u32) {
    let system = wind_area_particle_system();
    (
        system.uv_module.tiles_x as f32,
        system.uv_module.tiles_y as f32,
        system.uv_module.row_index,
    )
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

fn dot2(lhs: Vec2, rhs: Vec2) -> f32 {
    lhs.x * rhs.x + lhs.y * rhs.y
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

fn normalize_vec2(vec: Vec2) -> Vec2 {
    let len = (vec.x * vec.x + vec.y * vec.y).sqrt();
    if len <= f32::EPSILON {
        return Vec2 { x: 0.0, y: 0.0 };
    }
    Vec2 {
        x: vec.x / len,
        y: vec.y / len,
    }
}

fn wind_particle_side_velocity(t: f32) -> f32 {
    wind_area_particle_system().velocity_y.sample(t, 0.0)
}

fn wind_particle_size_scale(t: f32) -> f32 {
    wind_area_particle_system().size_over_lifetime.sample(t, 0.0)
}

fn update_wind_particle(particle: &mut WindParticle, dt: f32) -> bool {
    particle.age += dt;
    if particle.age >= particle.lifetime {
        return false;
    }

    let t_frac = particle.age / particle.lifetime;
    let side_velocity = wind_particle_side_velocity(t_frac);
    particle.x += (particle.vx + particle.side_x * side_velocity) * dt;
    particle.y += (particle.vy + particle.side_y * side_velocity) * dt;
    particle.rot += particle.rot_speed * dt;
    true
}

/// Spawn a wind leaf particle in the given area/system.
pub(super) fn spawn_wind_particle(
    area: &WindAreaDef,
    system_index: usize,
    particles: &mut Vec<WindParticle>,
    seed: u32,
) {
    let dir_len = (area.dir_x * area.dir_x + area.dir_y * area.dir_y)
        .sqrt()
        .max(f32::EPSILON);
    let dir_x = area.dir_x / dir_len;
    let dir_y = area.dir_y / dir_len;
    let side_x = -dir_y;
    let side_y = dir_x;
    let power_scale = wind_particle_power_scale(area.power_factor);
    let (emitter_offset_dir, emitter_offset_side) = wind_particle_emitter_offsets();
    let (shape_half_dir, shape_half_side) = wind_particle_shape_half_extents();
    let emitter_center_x = area.center_x
        + dir_x * emitter_offset_dir
        + side_x * emitter_offset_side;
    let emitter_center_y = area.center_y
        + dir_y * emitter_offset_dir
        + side_y * emitter_offset_side;
    let flow_offset = (pseudo_random(seed.wrapping_mul(3)) - 0.5) * shape_half_dir * 2.0;
    let side_offset = (pseudo_random(seed.wrapping_mul(7).wrapping_add(1)) - 0.5)
        * shape_half_side
        * 2.0;
    let x = emitter_center_x + dir_x * flow_offset + side_x * side_offset;
    let y = emitter_center_y + dir_y * flow_offset + side_y * side_offset;
    let size_random = pseudo_random(seed.wrapping_mul(11).wrapping_add(5));
    let speed_random = pseudo_random(seed.wrapping_mul(13).wrapping_add(9));
    let rot_random = pseudo_random(seed.wrapping_mul(23));
    let frame_random = pseudo_random(seed.wrapping_mul(31).wrapping_add(13));
    let rot_sign = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    let size = wind_particle_start_size(size_random);
    let speed = wind_particle_start_speed(speed_random) * power_scale;
    particles.push(WindParticle {
        x,
        y,
        vx: dir_x * speed,
        vy: dir_y * speed,
        side_x,
        side_y,
        age: 0.0,
        lifetime: wind_particle_lifetime(speed_random),
        rot: wind_particle_start_rotation(rot_random),
        rot_speed: wind_particle_rotation_speed(rot_random) * rot_sign,
        size,
        leaf_col: wind_particle_uv_column(frame_random),
        source_sprite_index: area.sprite_index,
        source_system_index: system_index,
    });
}

use super::LevelRenderer;

impl LevelRenderer {
    pub(super) fn seed_wind_particles(&mut self) {
        self.wind_particles.clear();
        let system_count = wind_area_particle_system_count();
        self.wind_spawn_accum = vec![0.0; self.wind_areas.len() * system_count];
        for (area_index, area) in self.wind_areas.iter().enumerate() {
            for system_index in 0..system_count {
                for prewarm_index in 0..wind_area_particle_prewarm_count() {
                    let seed = area_index as u32 * 1009
                        + system_index as u32 * 313
                        + prewarm_index as u32 * 17;
                    spawn_wind_particle(area, system_index, &mut self.wind_particles, seed);
                    if let Some(p) = self.wind_particles.last_mut() {
                        let frac = pseudo_random(
                            seed.wrapping_add(29) ^ p.x.to_bits() ^ p.y.to_bits(),
                        );
                        p.age = p.lifetime * frac;
                    }
                }
            }
        }
    }

    /// Advance all particle systems by `dt` seconds (fan, wind, zzz).
    pub(super) fn update_particles(&mut self, dt: f32) {
        // ── Fan: state machine + particle spawn/update ──
        self.update_fan_particles(dt);

        // ── Wind leaf particle update ──
        // Spawn new particles
        let system_count = wind_area_particle_system_count();
        for a in 0..self.wind_areas.len() {
            for system_index in 0..system_count {
                let accum_index = a * system_count + system_index;
                self.wind_spawn_accum[accum_index] += dt * wind_area_particle_spawn_rate();
                let mut area_count = self
                    .wind_particles
                    .iter()
                    .filter(|p| {
                        p.source_sprite_index == self.wind_areas[a].sprite_index
                            && p.source_system_index == system_index
                    })
                    .count();
                while self.wind_spawn_accum[accum_index] >= 1.0
                    && area_count < wind_area_particle_max_count()
                {
                    self.wind_spawn_accum[accum_index] -= 1.0;
                    let seed = (self.time * 1000.0) as u32
                        + a as u32 * 977
                        + system_index as u32 * 313
                        + self.wind_particles.len() as u32 * 131;
                    spawn_wind_particle(
                        &self.wind_areas[a],
                        system_index,
                        &mut self.wind_particles,
                        seed,
                    );
                    area_count += 1;
                }
            }
        }
        // Update particles
        let mut i = 0;
        while i < self.wind_particles.len() {
            let p = &mut self.wind_particles[i];
            if !update_wind_particle(p, dt) {
                self.wind_particles.swap_remove(i);
                continue;
            }
            i += 1;
        }

        // ── Attached effect particle update (rocket, turbo, magnet) ──
        self.update_attached_effect_particles(dt);

        // ── Zzz particle update (bird sleeping) ──
        self.update_zzz_particles(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        particle_sheet_uv_rect, spawn_wind_particle, update_wind_particle,
        wind_particle_side_velocity, wind_particle_size_scale, WindAreaDef, WindParticle,
    };

    #[test]
    fn particle_sheet_uv_rect_matches_previous_correct_hardcoded_rows() {
        let fan = particle_sheet_uv_rect(8.0, 8.0, 0, 3);
        assert_eq!(fan, (3.0 / 8.0, 4.0 / 8.0, 0.0 / 8.0, 1.0 / 8.0));

        let zzz = particle_sheet_uv_rect(8.0, 8.0, 2, 6);
        assert_eq!(zzz, (6.0 / 8.0, 7.0 / 8.0, 2.0 / 8.0, 3.0 / 8.0));

        let wind = particle_sheet_uv_rect(16.0, 16.0, 2, 4);
        assert_eq!(wind, (4.0 / 16.0, 5.0 / 16.0, 2.0 / 16.0, 3.0 / 16.0));
    }

    #[test]
    fn wind_area_particles_follow_area_direction() {
        let mut particles = Vec::new();
        spawn_wind_particle(
            &WindAreaDef {
                sprite_index: 0,
                center_x: 0.0,
                center_y: 0.0,
                half_w: 20.0,
                half_h: 7.5,
                dir_x: 0.0,
                dir_y: 8.0,
                power_factor: 1.5,
            },
            0,
            &mut particles,
            0,
        );

        let particle = &particles[0];
        assert!(particle.vy > 0.0);
        assert!(particle.vy.abs() > particle.vx.abs());
    }

    #[test]
    fn wind_particle_side_velocity_curve_matches_unity_prefab_sign_changes() {
        assert!(wind_particle_side_velocity(0.0) < 0.0);
        assert!(wind_particle_side_velocity(0.06) > 0.0);
        assert!(wind_particle_side_velocity(0.30) < 0.0);
        assert!(wind_particle_side_velocity(0.75) > 0.0);
    }

    #[test]
    fn wind_particle_size_curve_matches_unity_prefab_envelope() {
        assert_eq!(wind_particle_size_scale(0.0), 0.0);
        assert!(wind_particle_size_scale(0.03) > 0.5);
        assert!((wind_particle_size_scale(0.5) - 1.0889437).abs() < 0.001);
        assert!(wind_particle_size_scale(0.95) < 0.5);
    }

    #[test]
    fn wind_particle_update_uses_perpendicular_sway_axis() {
        let lifetime = super::wind_particle_lifetime(0.0);
        let mut particle = WindParticle {
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 8.0,
            side_x: 1.0,
            side_y: 0.0,
            age: lifetime * 0.04,
            lifetime,
            rot: 0.0,
            rot_speed: 0.0,
            size: 0.5,
            leaf_col: 0,
            source_sprite_index: 0,
            source_system_index: 0,
        };

        assert!(update_wind_particle(&mut particle, lifetime * 0.02));
        assert!(particle.x.abs() > 0.01);
        assert!(particle.y > 0.5);
    }
}

/// Draw wind leaf particles (from Particles_Sheet_01.png 16×16 grid).
pub(super) fn draw_wind_particles(
    particles: &[WindParticle],
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    let Some(leaf_tex) = tex_id else { return };
    let (tiles_x, tiles_y, row_index) = wind_particle_uv_layout();
    for p in particles {
        let t_frac = p.age / p.lifetime;
        let size_scale = wind_particle_size_scale(t_frac);
        let alpha = size_scale.clamp(0.0, 1.0);
        let sz = p.size * size_scale * camera.zoom;
        if sz < 0.5 {
            continue;
        }
        let center = camera.world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
        if !rect.expand(20.0).contains(center) {
            continue;
        }

        let (u0, u1, v0, v1) =
            particle_sheet_uv_rect(tiles_x, tiles_y, row_index, p.leaf_col);

        let hw = sz * 0.5;
        let hh = sz * 0.5;
        let cos_r = p.rot.cos();
        let sin_r = p.rot.sin();
        let rot_pt = |dx: f32, dy: f32| -> egui::Pos2 {
            egui::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };

        let a = (alpha * 255.0) as u8;
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, a);

        let mut mesh = egui::Mesh::with_texture(leaf_tex);
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rot_pt(-hw, -hh),
            uv: egui::pos2(u0, v0),
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rot_pt(hw, -hh),
            uv: egui::pos2(u1, v0),
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rot_pt(hw, hh),
            uv: egui::pos2(u1, v1),
            color,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: rot_pt(-hw, hh),
            uv: egui::pos2(u0, v1),
            color,
        });
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }
}
