//! Wind area particle systems: spawn, update, draw.

use std::collections::HashMap;

use eframe::egui;

use crate::data::unity_particles;
use crate::data::unity_particles::{ParticleCurve, UnityParticleSystemDef};
use crate::domain::types::{Vec2, Vec3};

use super::{
    Camera, LevelRenderer, particle_sheet_uv_rect, pseudo_random, rotate_vec2,
};

/// Wind area zone definition.
#[derive(Clone, Debug)]
pub(crate) struct WindAreaDef {
    pub sprite_index: usize,
    pub center_x: f32,
    pub center_y: f32,
    pub render_z: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub local_dir_x: f32,
    pub local_dir_y: f32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub power_factor: f32,
    pub systems: Vec<UnityParticleSystemDef>,
}

pub(crate) const WIND_AREA_HALF_W: f32 = 20.0;
pub(crate) const WIND_AREA_HALF_H: f32 = 7.5;
pub(crate) const WIND_AREA_POWER_FACTOR: f32 = 1.5;

#[derive(Default)]
struct WindAreaOverrideData {
    box_size: Option<Vec3>,
    handle_world: Option<Vec3>,
    power_factor: Option<f32>,
    systems: HashMap<String, WindAreaSystemOverrides>,
}

#[derive(Default)]
struct WindAreaSystemOverrides {
    local_position: Option<Vec3>,
    local_rotation: Option<[f32; 4]>,
    start_lifetime_scalar: Option<f32>,
    start_speed_scalar: Option<f32>,
    emission_rate_scalar: Option<f32>,
}

/// A single wind leaf particle.
pub(crate) struct WindParticle {
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

fn default_wind_area_systems() -> Vec<UnityParticleSystemDef> {
    wind_area_particle_systems().to_vec()
}

pub(crate) fn wind_area_particle_system_count() -> usize {
    wind_area_particle_systems().len()
}

fn wind_system_spawn_phase(sprite_index: usize, system_index: usize) -> f32 {
    pseudo_random(sprite_index as u32 * 4099 + system_index as u32 * 977 + 17)
}

fn wind_area_particle_system(
    area: &WindAreaDef,
    system_index: usize,
) -> &unity_particles::UnityParticleSystemDef {
    &area.systems[system_index]
}

fn wind_area_particle_spawn_rate(area: &WindAreaDef, system_index: usize) -> f32 {
    let system = wind_area_particle_system(area, system_index);
    if !system.play_on_awake && !system.looping {
        0.0
    } else {
        system.emission_rate.sample(0.0, 0.0)
    }
}

fn wind_area_particle_max_count(area: &WindAreaDef, system_index: usize) -> usize {
    wind_area_particle_system(area, system_index).max_particles
}

fn wind_area_particle_prewarm_count(area: &WindAreaDef, system_index: usize) -> usize {
    let system = wind_area_particle_system(area, system_index);
    if !system.prewarm {
        0
    } else {
        let average_lifetime =
            (system.start_lifetime.sample(0.0, 0.0) + system.start_lifetime.sample(0.0, 1.0))
                * 0.5;
        let seeded = (wind_area_particle_spawn_rate(area, system_index) * average_lifetime)
            .round() as usize;
        seeded.max(1).min(wind_area_particle_max_count(area, system_index))
    }
}

pub(crate) fn wind_area_local_direction() -> Vec2 {
    normalize_vec2(wind_area_prefab().wind_direction)
}

fn wind_particle_emitter_offsets(area: &WindAreaDef, system_index: usize) -> (f32, f32) {
    let system = wind_area_particle_system(area, system_index);
    let dir = Vec2 {
        x: area.local_dir_x,
        y: area.local_dir_y,
    };
    let side = Vec2 {
        x: -dir.y,
        y: dir.x,
    };
    let local = Vec2 {
        x: system.local_position.x,
        y: system.local_position.y,
    };
    (dot2(local, dir), dot2(local, side))
}

fn wind_particle_shape_half_extents(area: &WindAreaDef, system_index: usize) -> (f32, f32) {
    let system = wind_area_particle_system(area, system_index);
    let extents = system.projected_ellipsoid_half_extents_xy();
    (extents.x.max(f32::EPSILON), extents.y.max(f32::EPSILON))
}

fn wind_particle_lifetime(area: &WindAreaDef, system_index: usize, random: f32) -> f32 {
    wind_area_particle_system(area, system_index)
        .start_lifetime
        .sample(0.0, random)
}

fn wind_particle_start_speed(area: &WindAreaDef, system_index: usize, random: f32) -> f32 {
    wind_area_particle_system(area, system_index)
        .start_speed
        .sample(0.0, random)
}

fn wind_particle_start_size(area: &WindAreaDef, system_index: usize, random: f32) -> f32 {
    wind_area_particle_system(area, system_index)
        .start_size
        .sample(0.0, random)
}

fn wind_particle_start_rotation(area: &WindAreaDef, system_index: usize, random: f32) -> f32 {
    wind_area_particle_system(area, system_index)
        .start_rotation
        .sample(0.0, random)
}

fn wind_particle_rotation_speed(area: &WindAreaDef, system_index: usize, random: f32) -> f32 {
    wind_area_particle_system(area, system_index)
        .rotation_over_lifetime
        .sample(0.0, random)
}

fn wind_particle_uv_column(area: &WindAreaDef, system_index: usize, random: f32) -> u8 {
    wind_area_particle_system(area, system_index)
        .uv_module
        .sample_frame_index(random) as u8
}

fn wind_particle_uv_layout() -> (f32, f32, u32) {
    let system = wind_area_particle_systems()
        .first()
        .expect("WindArea should contain a WindEffect particle system");
    (
        system.uv_module.tiles_x as f32,
        system.uv_module.tiles_y as f32,
        system.uv_module.row_index,
    )
}

fn dot2(lhs: Vec2, rhs: Vec2) -> f32 {
    lhs.x * rhs.x + lhs.y * rhs.y
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

fn vec3_set_axis(vec: &mut Vec3, axis: &str, value: f32) {
    match axis {
        "x" => vec.x = value,
        "y" => vec.y = value,
        "z" => vec.z = value,
        _ => {}
    }
}

fn quat_set_axis(quat: &mut [f32; 4], axis: &str, value: f32) {
    match axis {
        "x" => quat[0] = value,
        "y" => quat[1] = value,
        "z" => quat[2] = value,
        "w" => quat[3] = value,
        _ => {}
    }
}

fn parse_float_assignment(trimmed: &str) -> Option<(&str, f32)> {
    let rest = trimmed.strip_prefix("Float ")?;
    let (field, value) = rest.split_once('=')?;
    Some((field.trim(), value.trim().parse::<f32>().ok()?))
}

fn parse_wind_area_overrides(raw_text: Option<&str>) -> WindAreaOverrideData {
    let Some(text) = raw_text else {
        return WindAreaOverrideData::default();
    };

    let mut result = WindAreaOverrideData::default();
    let mut path: Vec<String> = Vec::new();

    for line in text.lines() {
        let stripped = line.trim_start_matches('\t');
        let depth = line.len() - stripped.len();
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if path.len() <= depth {
            path.resize(depth + 1, String::new());
        }
        path[depth] = trimmed.to_string();
        path.truncate(depth + 1);

        let Some((field, value)) = parse_float_assignment(trimmed) else {
            continue;
        };

        if path.len() >= 4
            && path[1] == "Component UnityEngine.BoxCollider"
            && path[2] == "Vector3 m_Size"
        {
            let vec = result.box_size.get_or_insert(Vec3::default());
            vec3_set_axis(vec, field, value);
            continue;
        }

        if path.len() >= 4 && path[1] == "Component WindArea" && path[2] == "Vector3 windDirectionHandle" {
            let vec = result.handle_world.get_or_insert(Vec3::default());
            vec3_set_axis(vec, field, value);
            continue;
        }

        if path.len() >= 3 && path[1] == "Component WindArea" && field == "m_windPowerFactor" {
            result.power_factor = Some(value);
            continue;
        }

        let Some(child_name) = path.get(1).and_then(|entry| entry.strip_prefix("GameObject ")) else {
            continue;
        };
        if !child_name.starts_with("WindEffect") {
            continue;
        }

        let system = result.systems.entry(child_name.to_string()).or_default();

        if path.len() >= 5
            && path[2] == "Component UnityEngine.Transform"
            && path[3] == "Vector3 m_LocalPosition"
        {
            let vec = system.local_position.get_or_insert(Vec3::default());
            vec3_set_axis(vec, field, value);
            continue;
        }

        if path.len() >= 5
            && path[2] == "Component UnityEngine.Transform"
            && path[3] == "Quaternion m_LocalRotation"
        {
            let quat = system.local_rotation.get_or_insert([0.0, 0.0, 0.0, 0.0]);
            quat_set_axis(quat, field, value);
            continue;
        }

        if path.len() >= 6
            && path[2] == "Component UnityEngine.ParticleSystem"
            && path[3] == "Generic InitialModule"
            && path[4] == "Generic startLifetime"
            && field == "scalar"
        {
            system.start_lifetime_scalar = Some(value);
            continue;
        }

        if path.len() >= 6
            && path[2] == "Component UnityEngine.ParticleSystem"
            && path[3] == "Generic InitialModule"
            && path[4] == "Generic startSpeed"
            && field == "scalar"
        {
            system.start_speed_scalar = Some(value);
            continue;
        }

        if path.len() >= 6
            && path[2] == "Component UnityEngine.ParticleSystem"
            && path[3] == "Generic EmissionModule"
            && path[4] == "Generic rate"
            && field == "scalar"
        {
            system.emission_rate_scalar = Some(value);
        }
    }

    result
}

fn apply_wind_area_system_override(
    system: &mut UnityParticleSystemDef,
    overrides: Option<&WindAreaSystemOverrides>,
    scale_x: f32,
    scale_y: f32,
) {
    if let Some(overrides) = overrides {
        if let Some(local_position) = overrides.local_position {
            system.local_position = local_position;
        }
        if let Some(local_rotation) = overrides.local_rotation {
            system.local_rotation = local_rotation;
        }
        if let Some(start_lifetime) = overrides.start_lifetime_scalar {
            system.start_lifetime = ParticleCurve::constant(start_lifetime);
        }
        if let Some(start_speed) = overrides.start_speed_scalar {
            system.start_speed = ParticleCurve::constant(start_speed);
        }
        if let Some(emission_rate) = overrides.emission_rate_scalar {
            system.emission_rate = ParticleCurve::constant(emission_rate);
        }
    }

    system.local_position.x *= scale_x;
    system.local_position.y *= scale_y;
    system.shape_scale.x *= scale_x.abs();
    system.shape_scale.y *= scale_y.abs();
}

fn wind_area_render_z(world_z: f32, systems: &[UnityParticleSystemDef]) -> f32 {
    let child_z = systems
        .iter()
        .map(|system| system.local_position.z)
        .fold(0.0, f32::min);
    world_z + child_z
}

pub(crate) fn build_wind_area_def(
    sprite_index: usize,
    center_x: f32,
    center_y: f32,
    world_z: f32,
    rotation: f32,
    scale_x: f32,
    scale_y: f32,
    override_text: Option<&str>,
) -> WindAreaDef {
    let overrides = parse_wind_area_overrides(override_text);
    let default_local_dir = wind_area_local_direction();
    let local_dir = overrides
        .handle_world
        .map(|handle_world| {
            rotate_vec2(
                Vec2 {
                    x: handle_world.x - center_x,
                    y: handle_world.y - center_y,
                },
                -rotation,
            )
        })
        .map(normalize_vec2)
        .filter(|dir| dir.x != 0.0 || dir.y != 0.0)
        .unwrap_or(default_local_dir);
    let world_dir = rotate_vec2(local_dir, rotation);
    let box_size = overrides.box_size.unwrap_or(Vec3 {
        x: WIND_AREA_HALF_W * 2.0,
        y: WIND_AREA_HALF_H * 2.0,
        z: 10.0,
    });
    let mut systems = default_wind_area_systems();
    for system in &mut systems {
        apply_wind_area_system_override(
            system,
            overrides.systems.get(&system.name),
            scale_x,
            scale_y,
        );
    }

    WindAreaDef {
        sprite_index,
        center_x,
        center_y,
        render_z: wind_area_render_z(world_z, &systems),
        half_w: box_size.x.abs() * 0.5 * scale_x.abs(),
        half_h: box_size.y.abs() * 0.5 * scale_y.abs(),
        local_dir_x: local_dir.x,
        local_dir_y: local_dir.y,
        dir_x: world_dir.x,
        dir_y: world_dir.y,
        power_factor: overrides
            .power_factor
            .unwrap_or(WIND_AREA_POWER_FACTOR),
        systems,
    }
}

fn wind_particle_side_velocity(t: f32) -> f32 {
    wind_area_particle_systems()[0].velocity_y.sample(t, 0.0)
}

fn wind_particle_size_scale(t: f32) -> f32 {
    wind_area_particle_systems()[0].size_over_lifetime.sample(t, 0.0)
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
fn spawn_wind_particle(
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
    let (emitter_offset_dir, emitter_offset_side) = wind_particle_emitter_offsets(area, system_index);
    let (_, shape_half_side) = wind_particle_shape_half_extents(area, system_index);
    let emitter_center_x = area.center_x
        + dir_x * emitter_offset_dir
        + side_x * emitter_offset_side;
    let emitter_center_y = area.center_y
        + dir_y * emitter_offset_dir
        + side_y * emitter_offset_side;
    let side_offset = (pseudo_random(seed.wrapping_mul(7).wrapping_add(1)) - 0.5)
        * shape_half_side
        * 2.0;
    let x = emitter_center_x + side_x * side_offset;
    let y = emitter_center_y + side_y * side_offset;
    let size_random = pseudo_random(seed.wrapping_mul(11).wrapping_add(5));
    let speed_random = pseudo_random(seed.wrapping_mul(13).wrapping_add(9));
    let rot_random = pseudo_random(seed.wrapping_mul(23));
    let frame_random = pseudo_random(seed.wrapping_mul(31).wrapping_add(13));
    let rot_sign = if seed.is_multiple_of(2) { 1.0 } else { -1.0 };
    let size = wind_particle_start_size(area, system_index, size_random);
    let speed = wind_particle_start_speed(area, system_index, speed_random);
    particles.push(WindParticle {
        x,
        y,
        vx: dir_x * speed,
        vy: dir_y * speed,
        side_x,
        side_y,
        age: 0.0,
        lifetime: wind_particle_lifetime(area, system_index, speed_random),
        rot: wind_particle_start_rotation(area, system_index, rot_random),
        rot_speed: wind_particle_rotation_speed(area, system_index, rot_random) * rot_sign,
        size,
        leaf_col: wind_particle_uv_column(area, system_index, frame_random),
        source_sprite_index: area.sprite_index,
        source_system_index: system_index,
    });
}

impl LevelRenderer {
    pub(crate) fn seed_wind_particles(&mut self) {
        self.wind_particles.clear();
        let system_count = wind_area_particle_system_count();
        self.wind_spawn_accum = self
            .wind_areas
            .iter()
            .flat_map(|area| {
                (0..system_count)
                    .map(move |system_index| wind_system_spawn_phase(area.sprite_index, system_index))
            })
            .collect();
        for (area_index, area) in self.wind_areas.iter().enumerate() {
            for system_index in 0..system_count {
                for prewarm_index in 0..wind_area_particle_prewarm_count(area, system_index) {
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

    pub(super) fn update_wind_particles(&mut self, dt: f32) {
        // Spawn new particles
        let system_count = wind_area_particle_system_count();
        for a in 0..self.wind_areas.len() {
            for system_index in 0..system_count {
                let accum_index = a * system_count + system_index;
                self.wind_spawn_accum[accum_index] +=
                    dt * wind_area_particle_spawn_rate(&self.wind_areas[a], system_index);
                let mut area_count = self
                    .wind_particles
                    .iter()
                    .filter(|p| {
                        p.source_sprite_index == self.wind_areas[a].sprite_index
                            && p.source_system_index == system_index
                    })
                    .count();
                while self.wind_spawn_accum[accum_index] >= 1.0
                    && area_count < wind_area_particle_max_count(&self.wind_areas[a], system_index)
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
    }
}

/// Draw wind leaf particles (from Particles_Sheet_01.png 16×16 grid).
pub(crate) fn draw_wind_particles(
    particles: &[WindParticle],
    source_sprite_index: Option<usize>,
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    let Some(leaf_tex) = tex_id else { return };
    let (tiles_x, tiles_y, row_index) = wind_particle_uv_layout();
    for p in particles {
        if source_sprite_index.is_some_and(|sprite_index| p.source_sprite_index != sprite_index) {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL_01_WINDAREA_OVERRIDE: &str = "GameObject WindArea\n\tComponent UnityEngine.BoxCollider\n\t\tVector3 m_Size\n\t\t\tFloat x = 31.4\n\tComponent WindArea\n\t\tVector3 windDirectionHandle\n\t\t\tFloat x = 17.67106\n\t\t\tFloat y = 0.617309\n\t\t\tFloat z = 0\n\t\tFloat m_windPowerFactor = 0.26\n\tGameObject WindEffect1\n\t\tComponent UnityEngine.Transform\n\t\t\tQuaternion m_LocalRotation\n\t\t\t\tFloat x = 0.0005903003\n\t\t\t\tFloat y = 0.7071065\n\t\t\t\tFloat z = -0.0005903003\n\t\t\t\tFloat w = 0.7071065\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = -15.69998\n\t\t\t\tFloat y = 0.01111133\n\t\t\t\tFloat z = -2\n\t\tComponent UnityEngine.ParticleSystem\n\t\t\tGeneric InitialModule\n\t\t\t\tGeneric startLifetime\n\t\t\t\t\tFloat scalar = 5.233333\n\t\t\t\tGeneric startSpeed\n\t\t\t\t\tFloat scalar = 6\n\t\t\tGeneric EmissionModule\n\t\t\t\tGeneric rate\n\t\t\t\t\tFloat scalar = 1\n";

    fn test_wind_area(dir_x: f32, dir_y: f32) -> WindAreaDef {
        let local_dir = normalize_vec2(Vec2 { x: dir_x, y: dir_y });
        WindAreaDef {
            sprite_index: 0,
            center_x: 0.0,
            center_y: 0.0,
            render_z: 0.0,
            half_w: 20.0,
            half_h: 7.5,
            local_dir_x: local_dir.x,
            local_dir_y: local_dir.y,
            dir_x: dir_x,
            dir_y: dir_y,
            power_factor: 1.5,
            systems: default_wind_area_systems(),
        }
    }

    #[test]
    fn wind_area_particles_follow_area_direction() {
        let mut particles = Vec::new();
        spawn_wind_particle(
            &test_wind_area(0.0, 8.0),
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
    fn wind_particle_emitter_offsets_match_prefab_xy_plane() {
        let area = build_wind_area_def(0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, None);
        let (dir_offset, side_offset) = wind_particle_emitter_offsets(&area, 0);
        assert!((dir_offset + 12.161071).abs() < 0.001);
        assert!((side_offset + 6.0450926).abs() < 0.001);
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
        let area = build_wind_area_def(0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, None);
        let lifetime = wind_particle_lifetime(&area, 0, 0.0);
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
    fn wind_area_level01_override_produces_horizontal_runtime_values() {
        let area = build_wind_area_def(
            0,
            16.059305,
            0.62,
            0.0,
            0.0,
            2.0,
            1.0,
            Some(LEVEL_01_WINDAREA_OVERRIDE),
        );

        assert!((area.dir_x - 1.0).abs() < 0.01);
        assert!(area.dir_y.abs() < 0.01);
        assert!((area.half_w - 31.4).abs() < 0.01);
        assert!((area.power_factor - 0.26).abs() < 0.0001);
        assert!((area.systems[0].local_position.x + 31.39996).abs() < 0.01);
        assert!((area.systems[0].local_position.y - 0.01111133).abs() < 0.0001);
        assert_eq!(area.systems[0].local_position.z, -2.0);
        assert_eq!(area.render_z, -2.0);
        let extents = area.systems[0].projected_ellipsoid_half_extents_xy();
        assert!(extents.x > 1.9);
        assert!((area.systems[0].start_lifetime.sample(0.0, 0.0) - 5.233333).abs() < 0.0001);
        assert!((area.systems[0].start_speed.sample(0.0, 0.0) - 6.0).abs() < 0.0001);

        let mut particles = Vec::new();
        spawn_wind_particle(&area, 0, &mut particles, 0);
        let particle = &particles[0];
        assert!(particle.vx > 5.9);
        assert!(particle.vx.abs() > particle.vy.abs() * 100.0);
    }

    #[test]
    fn wind_particle_horizontal_area_spawns_along_side_strip() {
        let area = build_wind_area_def(
            0,
            16.059305,
            0.62,
            0.0,
            0.0,
            2.0,
            1.0,
            Some(LEVEL_01_WINDAREA_OVERRIDE),
        );

        let mut particles = Vec::new();
        for seed in 0..16 {
            spawn_wind_particle(&area, 0, &mut particles, seed);
        }

        let min_x = particles.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = particles.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = particles.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = particles.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);

        let x_span = max_x - min_x;
        let y_span = max_y - min_y;
        assert!(x_span < 0.1);
        assert!(y_span > 1.0);
        assert!(y_span > x_span * 20.0);
    }

    #[test]
    fn wind_system_spawn_phases_are_desynchronized() {
        let phase0 = wind_system_spawn_phase(0, 0);
        let phase1 = wind_system_spawn_phase(0, 1);
        let phase2 = wind_system_spawn_phase(0, 2);
        assert_ne!(phase0.to_bits(), phase1.to_bits());
        assert_ne!(phase1.to_bits(), phase2.to_bits());
    }
}
