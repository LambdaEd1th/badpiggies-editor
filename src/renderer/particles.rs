//! Particle systems: fan wind, leaf wind, bird Zzz particles.

use eframe::egui;

use crate::data::unity_particles;
use crate::data::unity_particles::UnityParticleSystemDef;
use crate::domain::types::Vec2;

use super::Camera;

/// Simple pseudo-random [0, 1) from u32 seed.
pub(super) fn pseudo_random(seed: u32) -> f32 {
    let n = seed.wrapping_mul(1103515245).wrapping_add(12345);
    ((n >> 16) & 0x7fff) as f32 / 32768.0
}

/// A single Zzz particle.
pub(super) struct ZzzParticle {
    pub x: f32,
    pub y: f32,
    pub vy: f32,
    pub velocity_x_random: f32,
    pub velocity_y_random: f32,
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub wobble_phase: f32,
    pub wobble_freq: f32,
    pub rot: f32,
    pub rot_speed: f32,
    pub uv_col: u8,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttachedEffectKind {
    RocketFire,
    TurboCharger,
    Magnet,
    FlySwarm,
}

pub(crate) struct AttachedEffectEmitter {
    pub world_x: f32,
    pub world_y: f32,
    pub rot: f32,
    pub kind: AttachedEffectKind,
    pub system_time: Vec<f32>,
    pub spawn_accum: Vec<f32>,
}

pub(crate) struct AttachedEffectParticle {
    pub emitter_index: usize,
    pub kind: AttachedEffectKind,
    pub system_index: usize,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub fx: f32,
    pub fy: f32,
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub size_random: f32,
    pub rot: f32,
    pub rot_random: f32,
    pub uv_col: u8,
    pub color_random: f32,
}

pub(super) const WIND_AREA_HALF_W: f32 = 20.0;
pub(super) const WIND_AREA_HALF_H: f32 = 7.5;
pub(super) const WIND_AREA_POWER_FACTOR: f32 = 1.5;
pub(super) const FAN_FIELD_HALF_W: f32 = 1.905;
pub(super) const FAN_FIELD_HALF_H: f32 = 4.8251266;
pub(super) const FAN_FIELD_CENTER_Y: f32 = 4.825125;

pub(crate) fn attached_effect_kind_for_sprite_name(name: &str) -> Option<AttachedEffectKind> {
    if name.starts_with("Part_Rocket_") || name.starts_with("Part_RedRocket_") {
        Some(AttachedEffectKind::RocketFire)
    } else if name.starts_with("Part_Engine") || name == "TurboChargerEffect" {
        Some(AttachedEffectKind::TurboCharger)
    } else if matches!(name, "MagnetEffect" | "SuperMagnet") {
        Some(AttachedEffectKind::Magnet)
    } else if name.starts_with("FlySwarm") {
        Some(AttachedEffectKind::FlySwarm)
    } else {
        None
    }
}

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

fn bird_sleep_system() -> &'static unity_particles::UnityParticleSystemDef {
    &unity_particles::bird_sleep_prefab()
        .expect("Bird sleep particle prefab should be available")
        .system
}

fn fan_puff_system() -> &'static unity_particles::UnityParticleSystemDef {
    &unity_particles::fan_puff_prefab()
        .expect("Fan puff particle prefab should be available")
        .system
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

pub(crate) fn attached_effect_systems(kind: AttachedEffectKind) -> &'static [UnityParticleSystemDef] {
    match kind {
        AttachedEffectKind::RocketFire => &unity_particles::rocket_fire_prefab()
            .expect("Rocket fire particle prefab should be available")
            .systems,
        AttachedEffectKind::TurboCharger => &unity_particles::turbo_charger_prefab()
            .expect("Turbo charger particle prefab should be available")
            .systems,
        AttachedEffectKind::Magnet => &unity_particles::magnet_effect_prefab()
            .expect("Magnet effect particle prefab should be available")
            .systems,
        AttachedEffectKind::FlySwarm => &unity_particles::fly_swarm_prefab()
            .expect("Fly swarm particle prefab should be available")
            .systems,
    }
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

fn zzz_emit_rate() -> f32 {
    bird_sleep_system().emission_rate.sample(0.0, 0.0)
}

fn zzz_max_per_bird() -> usize {
    bird_sleep_system().max_particles
}

fn zzz_start_size(random: f32) -> f32 {
    bird_sleep_system().start_size.sample(0.0, random)
}

fn zzz_spawn_offset() -> Vec2 {
    let system = bird_sleep_system();
    Vec2 {
        x: system.local_position.x,
        y: system.local_position.y,
    }
}

fn zzz_spawn_spread() -> Vec2 {
    bird_sleep_system().projected_ellipsoid_half_extents_xy()
}

fn zzz_lifetime(random: f32) -> f32 {
    bird_sleep_system().start_lifetime.sample(0.0, random)
}

fn zzz_start_rotation(random: f32) -> f32 {
    bird_sleep_system().start_rotation.sample(0.0, random)
}

fn zzz_rotation_speed(random: f32) -> f32 {
    bird_sleep_system()
        .rotation_over_lifetime
        .sample(0.0, random)
}

fn zzz_velocity_x(life_t: f32, random: f32, wobble_phase: f32, wobble_freq: f32, age: f32) -> f32 {
    let _ = (wobble_phase, wobble_freq, age);
    sample_particle_world_velocity_xy(bird_sleep_system(), life_t, random, random, random).x
}

fn zzz_velocity_y(life_t: f32, random: f32, fallback_vy: f32) -> f32 {
    let _ = fallback_vy;
    sample_particle_world_velocity_xy(bird_sleep_system(), life_t, random, random, random).y
}

fn zzz_force_xy(life_t: f32, x_random: f32, y_random: f32) -> Vec2 {
    sample_particle_world_force_xy(bird_sleep_system(), life_t, x_random, y_random, x_random)
}

fn zzz_size_scale(life_t: f32) -> f32 {
    bird_sleep_system().size_over_lifetime.sample(life_t, 0.0)
}

fn zzz_uv_layout() -> (f32, f32, u32) {
    let system = bird_sleep_system();
    (
        system.uv_module.tiles_x as f32,
        system.uv_module.tiles_y as f32,
        system.uv_module.row_index,
    )
}

fn zzz_uv_column(random: f32) -> u8 {
    bird_sleep_system().uv_module.sample_frame_index(random) as u8
}

fn fan_puff_duration() -> f32 {
    fan_puff_system().duration.max(f32::EPSILON)
}

fn fan_puff_max_count() -> usize {
    fan_puff_system().max_particles
}

fn fan_puff_offset() -> Vec2 {
    let system = fan_puff_system();
    Vec2 {
        x: system.local_position.x,
        y: system.local_position.y,
    }
}

fn fan_puff_spawn_half_width() -> f32 {
    (fan_puff_system().shape_scale.x * fan_puff_system().shape_radius * 0.5).abs()
}

fn fan_puff_lifetime(random: f32) -> f32 {
    fan_puff_system().start_lifetime.sample(0.0, random)
}

fn fan_puff_start_size(random: f32) -> f32 {
    fan_puff_system().start_size.sample(0.0, random)
}

fn fan_puff_start_rotation(random: f32) -> f32 {
    fan_puff_system().start_rotation.sample(0.0, random)
}

fn fan_puff_local_velocity(x_random: f32, y_random: f32) -> Vec2 {
    let system = fan_puff_system();
    Vec2 {
        x: system.velocity_x.sample(0.0, x_random),
        y: system.velocity_y.sample(0.0, y_random),
    }
}

fn fan_puff_local_force(x_random: f32, y_random: f32) -> Vec2 {
    let system = fan_puff_system();
    Vec2 {
        x: system.force_x.sample(0.0, x_random),
        y: system.force_y.sample(0.0, y_random),
    }
}

fn fan_puff_rotation_speed(life_t: f32, random: f32) -> f32 {
    fan_puff_system()
        .rotation_over_lifetime
        .sample(life_t, random)
}

fn fan_puff_size_scale(life_t: f32, random: f32) -> f32 {
    fan_puff_system().size_over_lifetime.sample(life_t, random)
}

fn fan_puff_uv_layout() -> (f32, f32, u32) {
    let system = fan_puff_system();
    (
        system.uv_module.tiles_x as f32,
        system.uv_module.tiles_y as f32,
        system.uv_module.row_index,
    )
}

fn fan_puff_uv_column(random: f32) -> u8 {
    fan_puff_system().uv_module.sample_frame_index(random) as u8
}

fn fan_puff_alpha(life_t: f32) -> u8 {
    let _ = life_t;
    255
}

fn particle_sheet_uv_rect(
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

fn particle_color_to_egui(color: unity_particles::ParticleColor) -> egui::Color32 {
    let to_u8 = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    egui::Color32::from_rgba_unmultiplied(
        to_u8(color.r),
        to_u8(color.g),
        to_u8(color.b),
        to_u8(color.a),
    )
}

fn attached_effect_particle_color(
    system: &UnityParticleSystemDef,
    life_t: f32,
    color_random: f32,
) -> egui::Color32 {
    let mut color = system.start_color.sample(0.0, color_random);
    if system.color_over_lifetime_enabled {
        let over_life = system.color_over_lifetime.sample(life_t, color_random);
        let start_is_effectively_black = color.r.max(color.g).max(color.b) < 0.01;
        if start_is_effectively_black {
            color = over_life;
        } else {
            color.r *= over_life.r;
            color.g *= over_life.g;
            color.b *= over_life.b;
            color.a *= over_life.a;
        }
    }
    particle_color_to_egui(color)
}

fn rotate_vec2(vec: Vec2, angle: f32) -> Vec2 {
    let cos_r = angle.cos();
    let sin_r = angle.sin();
    Vec2 {
        x: vec.x * cos_r - vec.y * sin_r,
        y: vec.x * sin_r + vec.y * cos_r,
    }
}

fn spawn_fan_particle(emitter: &FanEmitter, particles: &mut Vec<FanParticle>, seed: u32) {
    let spawn_random = pseudo_random(seed);
    let velocity_x_random = pseudo_random(seed.wrapping_add(1));
    let velocity_y_random = pseudo_random(seed.wrapping_add(2));
    let lifetime_random = pseudo_random(seed.wrapping_add(3));
    let rotation_random = pseudo_random(seed.wrapping_add(4));
    let size_random = pseudo_random(seed.wrapping_add(5));
    let force_x_random = pseudo_random(seed.wrapping_add(6));
    let force_y_random = pseudo_random(seed.wrapping_add(7));
    let frame_random = pseudo_random(seed.wrapping_add(8));

    let offset = fan_puff_offset();
    let local_position = Vec2 {
        x: offset.x + (spawn_random - 0.5) * 2.0 * fan_puff_spawn_half_width(),
        y: offset.y,
    };
    let world_offset = rotate_vec2(local_position, emitter.rot);

    let local_velocity = fan_puff_local_velocity(velocity_x_random, velocity_y_random);
    let local_force = fan_puff_local_force(force_x_random, force_y_random);

    let world_velocity = if fan_puff_system().velocity_world_space {
        local_velocity
    } else {
        rotate_vec2(local_velocity, emitter.rot)
    };
    let world_force = if fan_puff_system().force_world_space {
        local_force
    } else {
        rotate_vec2(local_force, emitter.rot)
    };

    particles.push(FanParticle {
        x: emitter.world_x + world_offset.x,
        y: emitter.world_y + world_offset.y,
        vx: world_velocity.x,
        vy: world_velocity.y,
        fx: world_force.x,
        fy: world_force.y,
        age: 0.0,
        lifetime: fan_puff_lifetime(lifetime_random),
        start_size: fan_puff_start_size(size_random),
        size_random,
        rot: fan_puff_start_rotation(rotation_random),
        rot_random: rotation_random,
        uv_col: fan_puff_uv_column(frame_random),
    });
}

fn dot2(lhs: Vec2, rhs: Vec2) -> f32 {
    lhs.x * rhs.x + lhs.y * rhs.y
}

fn sample_particle_world_velocity_xy(
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

fn sample_particle_world_force_xy(
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

fn attached_effect_particle_count(
    particles: &[AttachedEffectParticle],
    emitter_index: usize,
    system_index: usize,
) -> usize {
    particles
        .iter()
        .filter(|particle| {
            particle.emitter_index == emitter_index && particle.system_index == system_index
        })
        .count()
}

fn attached_effect_lifetime(system: &UnityParticleSystemDef, random: f32) -> f32 {
    system.start_lifetime.sample(0.0, random).max(f32::EPSILON)
}

fn attached_effect_size(system: &UnityParticleSystemDef, random: f32) -> f32 {
    system.start_size.sample(0.0, random).max(0.0)
}

fn attached_effect_duration(system: &UnityParticleSystemDef) -> f32 {
    system.duration.max(f32::EPSILON)
}

fn attached_effect_spawn_particle(
    emitter: &AttachedEffectEmitter,
    emitter_index: usize,
    system_index: usize,
    particles: &mut Vec<AttachedEffectParticle>,
    seed: u32,
) {
    let system = &attached_effect_systems(emitter.kind)[system_index];
    let offset_random_x = pseudo_random(seed.wrapping_add(1));
    let offset_random_y = pseudo_random(seed.wrapping_add(2));
    let velocity_random_x = pseudo_random(seed.wrapping_add(3));
    let velocity_random_y = pseudo_random(seed.wrapping_add(4));
    let velocity_random_z = pseudo_random(seed.wrapping_add(5));
    let force_random_x = pseudo_random(seed.wrapping_add(6));
    let force_random_y = pseudo_random(seed.wrapping_add(7));
    let force_random_z = pseudo_random(seed.wrapping_add(8));
    let lifetime_random = pseudo_random(seed.wrapping_add(9));
    let size_random = pseudo_random(seed.wrapping_add(10));
    let rotation_random = pseudo_random(seed.wrapping_add(11));
    let frame_random = pseudo_random(seed.wrapping_add(12));
    let color_random = pseudo_random(seed.wrapping_add(13));

    let spread = system.projected_ellipsoid_half_extents_xy();
    let spawn_offset = rotate_vec2(
        Vec2 {
            x: system.local_position.x + (offset_random_x - 0.5) * 2.0 * spread.x,
            y: system.local_position.y + (offset_random_y - 0.5) * 2.0 * spread.y,
        },
        emitter.rot,
    );
    let local_velocity = sample_particle_world_velocity_xy(
        system,
        0.0,
        velocity_random_x,
        velocity_random_y,
        velocity_random_z,
    );
    let local_force = sample_particle_world_force_xy(
        system,
        0.0,
        force_random_x,
        force_random_y,
        force_random_z,
    );
    let world_velocity = if system.velocity_world_space {
        local_velocity
    } else {
        rotate_vec2(local_velocity, emitter.rot)
    };
    let world_force = if system.force_world_space {
        local_force
    } else {
        rotate_vec2(local_force, emitter.rot)
    };

    particles.push(AttachedEffectParticle {
        emitter_index,
        kind: emitter.kind,
        system_index,
        x: emitter.world_x + spawn_offset.x,
        y: emitter.world_y + spawn_offset.y,
        vx: world_velocity.x,
        vy: world_velocity.y,
        fx: world_force.x,
        fy: world_force.y,
        age: 0.0,
        lifetime: attached_effect_lifetime(system, lifetime_random),
        start_size: attached_effect_size(system, size_random),
        size_random,
        rot: system.start_rotation.sample(0.0, rotation_random) + emitter.rot,
        rot_random: rotation_random,
        uv_col: system.uv_module.sample_frame_index(frame_random) as u8,
        color_random,
    });
}

fn prewarm_attached_effect_count(system: &UnityParticleSystemDef) -> usize {
    if !system.prewarm {
        return 0;
    }
    let average_lifetime =
        (attached_effect_lifetime(system, 0.0) + attached_effect_lifetime(system, 1.0)) * 0.5;
    (system.emission_rate.sample(0.0, 0.0) * average_lifetime)
        .round()
        .max(1.0) as usize
}

/// Fan state machine (mirrors Fan.cs Update).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum FanState {
    Inactive,
    DelayedStart,
    SpinUp,
    Spinning,
    SpinDown,
}

/// Persistent fan animation state.
pub(super) struct FanEmitter {
    /// Index into sprite_data for propeller scaling.
    pub sprite_index: usize,
    /// Current state.
    pub state: FanState,
    /// Time counter in current state.
    pub counter: f32,
    /// Current visual force after Unity's 0..10 clamp.
    pub force: f32,
    /// Override-controlled physical target force from Fan.prefab / object overrides.
    pub target_force: f32,
    /// Whether particle emission is on.
    pub emitting: bool,
    /// Propeller rotation angle (rad).
    pub angle: f32,
    /// Visual force captured at SpinDown start for the 2-second coast-out.
    pub spin_down_start_force: f32,
    /// Remaining forward rotation until the next 0° or 180° stop orientation.
    pub spin_down_angle_left: f32,
    /// Fan world position.
    pub world_x: f32,
    pub world_y: f32,
    /// Fan rotation in radians.
    pub rot: f32,
    /// Burst emission timer (0..1) cycling at 1 Hz.
    pub burst_time: f32,
    // Timing config from override or defaults
    pub start_time: f32,
    pub on_time: f32,
    pub off_time: f32,
    pub delayed_start: f32,
    pub always_on: bool,
}

/// Fan wind particle.
pub(super) struct FanParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub fx: f32,
    pub fy: f32,
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub size_random: f32,
    pub rot: f32,
    pub rot_random: f32,
    pub uv_col: u8,
}

use super::LevelRenderer;

impl LevelRenderer {
    pub(super) fn seed_attached_effect_particles(&mut self) {
        self.attached_effect_particles.clear();
        for emitter in &mut self.attached_effect_emitters {
            emitter.system_time.fill(0.0);
            emitter.spawn_accum.fill(0.0);
        }

        for emitter_index in 0..self.attached_effect_emitters.len() {
            let emitter = &self.attached_effect_emitters[emitter_index];
            for (system_index, system) in attached_effect_systems(emitter.kind).iter().enumerate() {
                let prewarm_count = prewarm_attached_effect_count(system).min(system.max_particles);
                for prewarm_index in 0..prewarm_count {
                    let seed = emitter_index as u32 * 1009
                        + system_index as u32 * 313
                        + prewarm_index as u32 * 17;
                    attached_effect_spawn_particle(
                        emitter,
                        emitter_index,
                        system_index,
                        &mut self.attached_effect_particles,
                        seed,
                    );
                    if let Some(particle) = self.attached_effect_particles.last_mut() {
                        let frac = pseudo_random(seed.wrapping_add(29));
                        particle.age = particle.lifetime * frac;
                    }
                }
            }
        }
    }

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
        // ── Fan state machine update ──
        for emitter in &mut self.fan_emitters {
            update_fan_emitter(emitter, dt);
        }

        // ── Fan particle burst emission ──
        for ei in 0..self.fan_emitters.len() {
            let prev_t = self.fan_emitters[ei].burst_time;
            self.fan_emitters[ei].burst_time += dt;
            if self.fan_emitters[ei].burst_time >= fan_puff_duration() {
                self.fan_emitters[ei].burst_time -= fan_puff_duration();
            }
            let new_t = self.fan_emitters[ei].burst_time;
            if !self.fan_emitters[ei].emitting {
                continue;
            }
            for (burst_index, burst) in fan_puff_system().bursts.iter().enumerate() {
                let cycle_count = burst.cycle_count.max(1);
                for cycle_index in 0..cycle_count {
                    let bt = burst.time + cycle_index as f32 * burst.repeat_interval;
                    let fired = (prev_t <= bt && new_t > bt)
                        || (prev_t > new_t && (prev_t <= bt || new_t > bt));
                    if !fired {
                        continue;
                    }
                    let seed = (self.time * 1000.0) as u32
                        + ei as u32 * 773
                        + burst_index as u32 * 419
                        + cycle_index * 97
                        + self.fan_particles.len() as u32 * 211;
                    let burst_count = burst.sample_count(pseudo_random(seed.wrapping_add(17)));
                    let e = &self.fan_emitters[ei];
                    for particle_index in 0..burst_count {
                        if self.fan_particles.len() >= fan_puff_max_count() {
                            break;
                        }
                        spawn_fan_particle(
                            e,
                            &mut self.fan_particles,
                            seed.wrapping_add(particle_index as u32 * 23),
                        );
                    }
                }
            }
        }
        // Update fan particles
        let mut fi = 0;
        while fi < self.fan_particles.len() {
            let p = &mut self.fan_particles[fi];
            p.age += dt;
            let t = p.age / p.lifetime;
            if t >= 1.0 {
                self.fan_particles.swap_remove(fi);
                continue;
            }
            p.vx += p.fx * dt;
            p.vy += p.fy * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.rot += fan_puff_rotation_speed(t, p.rot_random) * dt;
            fi += 1;
        }

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
        for emitter_index in 0..self.attached_effect_emitters.len() {
            let emitter = &mut self.attached_effect_emitters[emitter_index];
            for (system_index, system) in attached_effect_systems(emitter.kind).iter().enumerate() {
                let duration = attached_effect_duration(system);
                let prev_time = emitter.system_time[system_index];
                let mut new_time = prev_time + dt;
                if system.looping {
                    while new_time >= duration {
                        new_time -= duration;
                    }
                } else {
                    new_time = new_time.min(duration);
                }
                emitter.system_time[system_index] = new_time;

                let t_frac = (new_time / duration).clamp(0.0, 1.0);
                emitter.spawn_accum[system_index] +=
                    dt * system.emission_rate.sample(t_frac, 0.0).max(0.0);
                let mut system_count = attached_effect_particle_count(
                    &self.attached_effect_particles,
                    emitter_index,
                    system_index,
                );
                while emitter.spawn_accum[system_index] >= 1.0 && system_count < system.max_particles {
                    emitter.spawn_accum[system_index] -= 1.0;
                    let seed = (self.time * 1000.0) as u32
                        + emitter_index as u32 * 661
                        + system_index as u32 * 173
                        + self.attached_effect_particles.len() as u32 * 19;
                    attached_effect_spawn_particle(
                        emitter,
                        emitter_index,
                        system_index,
                        &mut self.attached_effect_particles,
                        seed,
                    );
                    system_count += 1;
                }

                for (burst_index, burst) in system.bursts.iter().enumerate() {
                    let cycle_count = burst.cycle_count.max(1);
                    for cycle_index in 0..cycle_count {
                        let burst_time = burst.time + cycle_index as f32 * burst.repeat_interval;
                        let fired = if system.looping {
                            (prev_time <= burst_time && new_time > burst_time)
                                || (prev_time > new_time
                                    && (prev_time <= burst_time || new_time > burst_time))
                        } else {
                            prev_time <= burst_time && new_time > burst_time
                        };
                        if !fired {
                            continue;
                        }
                        let burst_count = burst
                            .sample_count(pseudo_random(
                                emitter_index as u32 * 941
                                    + system_index as u32 * 281
                                    + burst_index as u32 * 43
                                    + cycle_index * 7,
                            ))
                            .min(system.max_particles.saturating_sub(system_count));
                        for particle_index in 0..burst_count {
                            let seed = (self.time * 1000.0) as u32
                                + emitter_index as u32 * 857
                                + system_index as u32 * 131
                                + burst_index as u32 * 59
                                + particle_index as u32 * 23;
                            attached_effect_spawn_particle(
                                emitter,
                                emitter_index,
                                system_index,
                                &mut self.attached_effect_particles,
                                seed,
                            );
                        }
                        system_count += burst_count;
                    }
                }
            }
        }

        let mut attached_index = 0;
        while attached_index < self.attached_effect_particles.len() {
            let particle = &mut self.attached_effect_particles[attached_index];
            let system = &attached_effect_systems(particle.kind)[particle.system_index];
            particle.age += dt;
            if particle.age >= particle.lifetime {
                self.attached_effect_particles.swap_remove(attached_index);
                continue;
            }
            let life_t = particle.age / particle.lifetime;
            particle.vx += particle.fx * dt;
            particle.vy += particle.fy * dt;
            particle.x += particle.vx * dt;
            particle.y += particle.vy * dt;
            particle.rot += system.rotation_over_lifetime.sample(life_t, particle.rot_random) * dt;
            attached_index += 1;
        }

        // ── Zzz particle update (bird sleeping) ──
        // Spawn new Zzz particles
        for bi in 0..self.bird_positions.len() {
            if bi < self.zzz_emit_accum.len() {
                self.zzz_emit_accum[bi] += dt;
                while self.zzz_emit_accum[bi] >= 1.0 / zzz_emit_rate()
                    && self.zzz_particles.len() < zzz_max_per_bird() * self.bird_positions.len()
                {
                    self.zzz_emit_accum[bi] -= 1.0 / zzz_emit_rate();
                    let bx = self.bird_positions[bi].x;
                    let by = self.bird_positions[bi].y;
                    let seed = (self.time * 1000.0) as u32
                        + bi as u32 * 997
                        + self.zzz_particles.len() as u32 * 337;
                    let r1 = pseudo_random(seed);
                    let r2 = pseudo_random(seed.wrapping_add(1));
                    let r3 = pseudo_random(seed.wrapping_add(2));
                    let r4 = pseudo_random(seed.wrapping_add(3));
                    let r5 = pseudo_random(seed.wrapping_add(4));
                    let spawn_offset = zzz_spawn_offset();
                    let spawn_spread = zzz_spawn_spread();
                    self.zzz_particles.push(ZzzParticle {
                        x: bx + spawn_offset.x + (r1 - 0.5) * 2.0 * spawn_spread.x,
                        y: by + spawn_offset.y + (r2 - 0.5) * 2.0 * spawn_spread.y,
                        vy: 0.31 + r3 * 0.18,
                        velocity_x_random: r3,
                        velocity_y_random: r4,
                        age: 0.0,
                        lifetime: zzz_lifetime(r4),
                        start_size: zzz_start_size(r3),
                        wobble_phase: r5 * std::f32::consts::TAU,
                        wobble_freq: 0.8 + pseudo_random(seed.wrapping_add(5)) * 0.4,
                        rot: zzz_start_rotation(r5),
                        rot_speed: zzz_rotation_speed(pseudo_random(seed.wrapping_add(6))),
                        uv_col: zzz_uv_column(r1),
                    });
                }
            }
        }
        // Update Zzz particles
        let mut zi = 0;
        while zi < self.zzz_particles.len() {
            let p = &mut self.zzz_particles[zi];
            p.age += dt;
            if p.age >= p.lifetime {
                self.zzz_particles.swap_remove(zi);
                continue;
            }
            let life_t = p.age / p.lifetime;
            p.x += zzz_velocity_x(life_t, p.velocity_x_random, p.wobble_phase, p.wobble_freq, p.age)
                * dt;
            p.y += zzz_velocity_y(life_t, p.velocity_y_random, p.vy) * dt;
            let force = zzz_force_xy(life_t, p.velocity_x_random, p.velocity_y_random);
            p.x += 0.5 * force.x * dt * dt;
            p.y += 0.5 * force.y * dt * dt;
            p.rot += p.rot_speed * dt;
            zi += 1;
        }
    }
}

const FAN_VISUAL_FORCE_MAX: f32 = 10.0;
const FAN_RUNNING_ROTATION_SPEED: f32 = 600.0 * std::f32::consts::PI / 180.0;
const FAN_SPINDOWN_ROTATION_SPEED: f32 = 60.0 * std::f32::consts::PI / 180.0;
const FAN_SPINDOWN_TIME: f32 = 2.0;
const FAN_SPINDOWN_SNAP_EPSILON: f32 = 3.0 * std::f32::consts::PI / 180.0;

fn fan_visual_target_force(target_force: f32) -> f32 {
    target_force.clamp(0.0, FAN_VISUAL_FORCE_MAX)
}

fn snap_fan_angle(angle: f32) -> f32 {
    let wrapped = angle.rem_euclid(std::f32::consts::TAU);
    if (wrapped - std::f32::consts::PI).abs() < std::f32::consts::FRAC_PI_2 {
        std::f32::consts::PI
    } else {
        0.0
    }
}

fn spin_down_angle_left(angle: f32) -> f32 {
    let wrapped = angle.rem_euclid(std::f32::consts::TAU);
    let angle_left = std::f32::consts::TAU - wrapped;
    if angle_left >= std::f32::consts::PI {
        angle_left
    } else {
        angle_left + std::f32::consts::PI
    }
}

pub(super) fn reset_fan_emitter_for_build(emitter: &mut FanEmitter) {
    emitter.state = FanState::Inactive;
    emitter.counter = 0.0;
    emitter.force = 0.0;
    emitter.emitting = false;
    emitter.angle = 0.0;
    emitter.spin_down_start_force = 0.0;
    emitter.spin_down_angle_left = 0.0;
    emitter.burst_time = pseudo_random(emitter.sprite_index as u32 * 997);
}

pub(super) fn start_fan_emitter_for_play(emitter: &mut FanEmitter) {
    emitter.counter = 0.0;
    emitter.force = 0.0;
    emitter.spin_down_start_force = 0.0;
    emitter.spin_down_angle_left = 0.0;
    if !emitter.always_on && emitter.delayed_start > 0.0 {
        emitter.state = FanState::DelayedStart;
        emitter.emitting = false;
    } else {
        emitter.state = FanState::SpinUp;
        emitter.emitting = true;
    }
}

fn update_fan_emitter(emitter: &mut FanEmitter, dt: f32) {
    let target_force = fan_visual_target_force(emitter.target_force);

    match emitter.state {
        FanState::DelayedStart => {
            emitter.force = 0.0;
            emitter.counter += dt;
            if emitter.counter >= emitter.delayed_start {
                emitter.state = FanState::SpinUp;
                emitter.counter = 0.0;
                emitter.emitting = true;
            }
        }
        FanState::SpinUp => {
            emitter.counter += dt;
            if emitter.start_time <= f32::EPSILON || emitter.counter >= emitter.start_time {
                emitter.state = FanState::Spinning;
                emitter.counter = 0.0;
                emitter.force = target_force;
            } else {
                let t = emitter.counter / emitter.start_time;
                emitter.force = target_force * t * t; // spinupRamp: t² in Fan.prefab
            }
            if emitter.force > 0.0 {
                emitter.angle = (emitter.angle + FAN_RUNNING_ROTATION_SPEED * emitter.force * dt)
                    .rem_euclid(std::f32::consts::TAU);
            }
        }
        FanState::Spinning => {
            emitter.force = target_force;
            emitter.angle = (emitter.angle + FAN_RUNNING_ROTATION_SPEED * emitter.force * dt)
                .rem_euclid(std::f32::consts::TAU);
            if !emitter.always_on {
                emitter.counter += dt;
                if emitter.counter >= emitter.on_time {
                    emitter.emitting = false;
                    emitter.state = FanState::SpinDown;
                    emitter.counter = 0.0;
                    emitter.spin_down_start_force = emitter.force;
                    emitter.spin_down_angle_left = spin_down_angle_left(emitter.angle);
                }
            }
        }
        FanState::SpinDown => {
            emitter.counter += dt;
            let t = (emitter.counter / FAN_SPINDOWN_TIME).min(1.0);
            emitter.force = emitter.spin_down_start_force * (1.0 - t);
            if emitter.spin_down_angle_left > 0.0 && emitter.force > 0.0 {
                let delta = (FAN_SPINDOWN_ROTATION_SPEED * emitter.force * dt)
                    .min(emitter.spin_down_angle_left);
                emitter.angle = (emitter.angle + delta).rem_euclid(std::f32::consts::TAU);
                emitter.spin_down_angle_left -= delta;
            }
            if emitter.spin_down_angle_left <= FAN_SPINDOWN_SNAP_EPSILON || t >= 1.0 {
                emitter.state = FanState::Inactive;
                emitter.counter = 0.0;
                emitter.force = 0.0;
                emitter.angle = snap_fan_angle(emitter.angle);
                emitter.spin_down_angle_left = 0.0;
            }
        }
        FanState::Inactive => {
            emitter.force = 0.0;
            emitter.counter += dt;
            if emitter.counter >= emitter.off_time {
                emitter.state = FanState::SpinUp;
                emitter.counter = 0.0;
                emitter.emitting = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fan_puff_duration, fan_puff_size_scale, fan_puff_uv_column,
        fan_visual_target_force, reset_fan_emitter_for_build, snap_fan_angle,
        particle_sheet_uv_rect,
        spawn_fan_particle, spawn_wind_particle, spin_down_angle_left,
        start_fan_emitter_for_play, update_fan_emitter, update_wind_particle,
        wind_particle_side_velocity, wind_particle_size_scale, FanEmitter, FanState,
        WindAreaDef, WindParticle,
    };

    fn test_emitter() -> FanEmitter {
        FanEmitter {
            sprite_index: 0,
            state: FanState::SpinUp,
            counter: 0.0,
            force: 0.0,
            target_force: 115.0,
            emitting: true,
            angle: 0.0,
            spin_down_start_force: 0.0,
            spin_down_angle_left: 0.0,
            world_x: 0.0,
            world_y: 0.0,
            rot: 0.0,
            burst_time: 0.0,
            start_time: 2.0,
            on_time: 4.0,
            off_time: 2.0,
            delayed_start: 5.0,
            always_on: true,
        }
    }

    #[test]
    fn fan_spinup_uses_target_force_clamped_for_visuals() {
        let mut emitter = test_emitter();
        update_fan_emitter(&mut emitter, 1.0);

        assert_eq!(fan_visual_target_force(emitter.target_force), 10.0);
        assert!((emitter.force - 2.5).abs() < 0.001);
        assert!(emitter.angle > 0.0);
    }

    #[test]
    fn fan_spindown_snaps_back_to_cardinal_orientation() {
        let mut emitter = test_emitter();
        emitter.state = FanState::SpinDown;
        emitter.force = 10.0;
        emitter.spin_down_start_force = 10.0;
        emitter.spin_down_angle_left = spin_down_angle_left(std::f32::consts::PI * 0.75);
        emitter.angle = std::f32::consts::PI * 0.75;
        update_fan_emitter(&mut emitter, 2.1);

        assert_eq!(emitter.state, FanState::Inactive);
        assert_eq!(emitter.force, 0.0);
        assert_eq!(emitter.angle, snap_fan_angle(std::f32::consts::PI * 0.75));
    }

    #[test]
    fn fan_spindown_rotates_slower_than_active_spin() {
        let dt = 1.0 / 60.0;

        let mut spinning = test_emitter();
        spinning.state = FanState::Spinning;
        update_fan_emitter(&mut spinning, dt);

        let mut spin_down = test_emitter();
        spin_down.state = FanState::SpinDown;
        spin_down.force = 10.0;
        spin_down.spin_down_start_force = 10.0;
        spin_down.spin_down_angle_left = spin_down_angle_left(0.0);
        update_fan_emitter(&mut spin_down, dt);

        assert!(spinning.angle > 1.7);
        assert!(spin_down.angle < 0.2);
        assert!(spinning.angle > spin_down.angle * 5.0);
    }

    #[test]
    fn build_reset_returns_fan_to_unpowered_idle_pose() {
        let mut emitter = test_emitter();
        emitter.state = FanState::Spinning;
        emitter.force = 8.0;
        emitter.emitting = true;
        emitter.angle = 1.23;
        reset_fan_emitter_for_build(&mut emitter);

        assert_eq!(emitter.state, FanState::Inactive);
        assert_eq!(emitter.force, 0.0);
        assert!(!emitter.emitting);
        assert_eq!(emitter.angle, 0.0);
    }

    #[test]
    fn play_from_build_respects_delayed_start() {
        let mut emitter = test_emitter();
        emitter.always_on = false;
        emitter.delayed_start = 5.0;
        start_fan_emitter_for_play(&mut emitter);

        assert_eq!(emitter.state, FanState::DelayedStart);
        assert!(!emitter.emitting);
        assert_eq!(emitter.force, 0.0);
    }

    #[test]
    fn fan_puff_spawn_uses_unity_prefab_particle_config() {
        let emitter = test_emitter();
        let mut particles = Vec::new();
        spawn_fan_particle(&emitter, &mut particles, 0);

        let particle = &particles[0];
        assert!((particle.y - 0.6365275).abs() < 0.001);
        assert_eq!(particle.uv_col, 3);
        assert!((particle.fy + 0.5).abs() < 0.001);
        assert!((0.7..=1.5).contains(&particle.lifetime));
        assert!((particle.start_size - 1.2).abs() < 0.001);
    }

    #[test]
    fn fan_puff_helpers_follow_unity_prefab_curve_and_sheet() {
        assert_eq!(fan_puff_duration(), 1.0);
        assert_eq!(fan_puff_uv_column(0.0), 3);
        assert_eq!(fan_puff_size_scale(0.0, 0.5), 0.0);
        assert!(fan_puff_size_scale(0.5, 0.5) > 0.09);
        assert_eq!(fan_puff_size_scale(1.0, 0.5), 0.0);
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

    #[test]
    fn attached_effect_sprite_names_map_to_runtime_kinds() {
        assert_eq!(
            super::attached_effect_kind_for_sprite_name("Part_Rocket_01_SET"),
            Some(super::AttachedEffectKind::RocketFire)
        );
        assert_eq!(
            super::attached_effect_kind_for_sprite_name("Part_EngineBig_03_SET"),
            Some(super::AttachedEffectKind::TurboCharger)
        );
        assert_eq!(
            super::attached_effect_kind_for_sprite_name("SuperMagnetIcon"),
            None
        );
        assert_eq!(
            super::attached_effect_kind_for_sprite_name("SuperMagnet"),
            Some(super::AttachedEffectKind::Magnet)
        );
        assert_eq!(
            super::attached_effect_kind_for_sprite_name("FlySwarm"),
            Some(super::AttachedEffectKind::FlySwarm)
        );
        assert_eq!(super::attached_effect_kind_for_sprite_name("Fan"), None);
    }

    #[test]
    fn turbo_effect_particle_color_uses_color_over_lifetime_gradient() {
        let system = &super::attached_effect_systems(super::AttachedEffectKind::TurboCharger)[0];
        let start = super::attached_effect_particle_color(system, 0.0, 0.0);
        let end = super::attached_effect_particle_color(system, 1.0, 0.0);

        assert!(start.r() > end.r());
        assert!(start.g() > end.g());
        assert!(start.b() > end.b());
        assert_eq!(start.a(), end.a());
    }
}

pub(super) fn draw_attached_effect_particles(
    particles: &[AttachedEffectParticle],
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    for particle in particles {
        let system = &attached_effect_systems(particle.kind)[particle.system_index];
        let life_t = particle.age / particle.lifetime;
        let size_scale = system.size_over_lifetime.sample(life_t, particle.size_random);
        let sz = particle.start_size * size_scale * camera.zoom;
        if sz < 0.5 {
            continue;
        }

        let center = camera.world_to_screen(
            Vec2 {
                x: particle.x,
                y: particle.y,
            },
            canvas_center,
        );
        if !rect.expand(30.0).contains(center) {
            continue;
        }

        let color = attached_effect_particle_color(system, life_t, particle.color_random);
        let hw = sz * 0.5;
        let hh = sz * 0.5;
        let cos_r = particle.rot.cos();
        let sin_r = particle.rot.sin();
        let rot_pt = |dx: f32, dy: f32| -> egui::Pos2 {
            egui::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };

        if let Some(tex_id) = tex_id {
            let (u0, u1, v0, v1) = particle_sheet_uv_rect(
                system.uv_module.tiles_x as f32,
                system.uv_module.tiles_y as f32,
                system.uv_module.row_index,
                particle.uv_col,
            );
            let mut mesh = egui::Mesh::with_texture(tex_id);
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
        } else {
            painter.circle_filled(center, hw, color);
        }
    }
}

/// Draw Zzz particles (textured rotated quads from Particles_Sheet_01.png).
pub(super) fn draw_zzz_particles(
    particles: &[ZzzParticle],
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    let (tiles_x, tiles_y, row_index) = zzz_uv_layout();
    for p in particles {
        let life_t = p.age / p.lifetime;
        let size_scale = zzz_size_scale(life_t);
        let sz = p.start_size * size_scale * camera.zoom;
        if sz < 0.5 {
            continue;
        }
        let center = camera.world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
        if !rect.expand(20.0).contains(center) {
            continue;
        }
        let alpha = 255u8;
        if let Some(tex_id) = tex_id {
            let (u0, u1, v0, v1) = particle_sheet_uv_rect(tiles_x, tiles_y, row_index, p.uv_col);
            let hw = sz * 0.5;
            let hh = sz * 0.5;
            let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            let mut mesh = egui::Mesh::with_texture(tex_id);
            let cos_r = p.rot.cos();
            let sin_r = p.rot.sin();
            let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                egui::pos2(
                    center.x + dx * cos_r + dy * sin_r,
                    center.y - dx * sin_r + dy * cos_r,
                )
            };
            let tl = rot(-hw, -hh);
            let tr = rot(hw, -hh);
            let br = rot(hw, hh);
            let bl = rot(-hw, hh);
            let i = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tl,
                uv: egui::pos2(u0, v0),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tr,
                uv: egui::pos2(u1, v0),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: br,
                uv: egui::pos2(u1, v1),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: bl,
                uv: egui::pos2(u0, v1),
                color: tint,
            });
            mesh.indices
                .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

/// Draw fan wind particles (cloud puffs from Particles_Sheet_01.png).
pub(super) fn draw_fan_particles(
    particles: &[FanParticle],
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    let (tiles_x, tiles_y, row_index) = fan_puff_uv_layout();
    for p in particles {
        let t_frac = p.age / p.lifetime;
        let size_scale = fan_puff_size_scale(t_frac, p.size_random);
        let sz = p.start_size * size_scale;
        let center = camera.world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
        if !rect.expand(30.0).contains(center) {
            continue;
        }
        let alpha = fan_puff_alpha(t_frac);
        let hw = sz * camera.zoom * 0.5;
        let hh = hw;
        if let Some(tex_id) = tex_id {
            let (u0, u1, v0, v1) = particle_sheet_uv_rect(tiles_x, tiles_y, row_index, p.uv_col);
            let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            let cos_r = p.rot.cos();
            let sin_r = p.rot.sin();
            let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                egui::pos2(
                    center.x + dx * cos_r + dy * sin_r,
                    center.y - dx * sin_r + dy * cos_r,
                )
            };
            let tl = rot(-hw, -hh);
            let tr = rot(hw, -hh);
            let br = rot(hw, hh);
            let bl = rot(-hw, hh);
            let mut mesh = egui::Mesh::with_texture(tex_id);
            let i = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tl,
                uv: egui::pos2(u0, v0),
                color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tr,
                uv: egui::pos2(u1, v0),
                color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: br,
                uv: egui::pos2(u1, v1),
                color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: bl,
                uv: egui::pos2(u0, v1),
                color,
            });
            mesh.indices
                .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
            painter.add(egui::Shape::mesh(mesh));
        } else {
            let puff_color = egui::Color32::from_rgba_unmultiplied(220, 230, 245, alpha);
            let puff_rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));
            painter.rect_filled(puff_rect, hw, puff_color);
        }
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
