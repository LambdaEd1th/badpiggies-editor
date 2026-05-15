//! Attached-emitter particle systems (rocket fire, turbo charger, magnet, fly swarm).

use eframe::egui;

use crate::data::unity_particles;
use crate::data::unity_particles::UnityParticleSystemDef;
use crate::domain::types::Vec2;

use super::{
    Camera, LevelRenderer, particle_sheet_uv_rect, pseudo_random, rotate_vec2,
    sample_particle_world_force_xy, sample_particle_world_velocity_xy,
};

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

impl LevelRenderer {
    pub(crate) fn seed_attached_effect_particles(&mut self) {
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

    pub(super) fn update_attached_effect_particles(&mut self, dt: f32) {
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
    }
}

pub(crate) fn draw_attached_effect_particles(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_effect_sprite_names_map_to_runtime_kinds() {
        assert_eq!(
            attached_effect_kind_for_sprite_name("Part_Rocket_01_SET"),
            Some(AttachedEffectKind::RocketFire)
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("Part_EngineBig_03_SET"),
            Some(AttachedEffectKind::TurboCharger)
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("SuperMagnetIcon"),
            None
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("SuperMagnet"),
            Some(AttachedEffectKind::Magnet)
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("FlySwarm"),
            Some(AttachedEffectKind::FlySwarm)
        );
        assert_eq!(attached_effect_kind_for_sprite_name("Fan"), None);
    }

    #[test]
    fn turbo_effect_particle_color_uses_color_over_lifetime_gradient() {
        let system = &attached_effect_systems(AttachedEffectKind::TurboCharger)[0];
        let start = attached_effect_particle_color(system, 0.0, 0.0);
        let end = attached_effect_particle_color(system, 1.0, 0.0);

        assert!(start.r() > end.r());
        assert!(start.g() > end.g());
        assert!(start.b() > end.b());
        assert_eq!(start.a(), end.a());
    }
}
