//! Bird "Zzz" sleep particles.

use eframe::egui;

use crate::data::unity_particles;
use crate::domain::types::Vec2;

use super::{
    Camera, LevelRenderer, particle_sheet_uv_rect, pseudo_random, sample_particle_world_force_xy,
    sample_particle_world_velocity_xy,
};

/// A single Zzz particle.
pub(crate) struct ZzzParticle {
    pub source_bird_index: usize,
    pub render_z: f32,
    pub x: f32,
    pub y: f32,
    pub velocity_x_random: f32,
    pub velocity_y_random: f32,
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub rot: f32,
    pub rot_speed: f32,
    pub uv_col: u8,
}

fn bird_sleep_system() -> &'static unity_particles::UnityParticleSystemDef {
    &unity_particles::bird_sleep_prefab()
        .expect("Bird sleep particle prefab should be available")
        .system
}

fn zzz_emit_rate() -> f32 {
    bird_sleep_system().emission_rate.sample(0.0, 0.0)
}

pub(crate) fn zzz_particle_texture_name() -> Option<&'static str> {
    bird_sleep_system().texture_name()
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

fn zzz_velocity_xy(life_t: f32, x_random: f32, y_random: f32) -> Vec2 {
    sample_particle_world_velocity_xy(bird_sleep_system(), life_t, x_random, y_random, x_random)
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

fn zzz_render_z(world_z: f32) -> f32 {
    world_z + bird_sleep_system().local_position.z
}

impl LevelRenderer {
    /// Spawn and advance Zzz particles attached to sleeping birds.
    pub(super) fn update_zzz_particles(&mut self, dt: f32) {
        // Spawn new Zzz particles
        for bi in 0..self.bird_positions.len() {
            if bi < self.zzz_emit_accum.len() {
                self.zzz_emit_accum[bi] += dt;
                while self.zzz_emit_accum[bi] >= 1.0 / zzz_emit_rate()
                    && self.zzz_particles.len() < zzz_max_per_bird() * self.bird_positions.len()
                {
                    self.zzz_emit_accum[bi] -= 1.0 / zzz_emit_rate();
                    let bird_pos = self.bird_positions[bi];
                    let bx = bird_pos.x;
                    let by = bird_pos.y;
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
                        source_bird_index: bi,
                        render_z: zzz_render_z(bird_pos.z),
                        x: bx + spawn_offset.x + (r1 - 0.5) * 2.0 * spawn_spread.x,
                        y: by + spawn_offset.y + (r2 - 0.5) * 2.0 * spawn_spread.y,
                        velocity_x_random: r3,
                        velocity_y_random: r4,
                        age: 0.0,
                        lifetime: zzz_lifetime(r4),
                        start_size: zzz_start_size(r3),
                        rot: zzz_start_rotation(r5),
                        rot_speed: zzz_rotation_speed(pseudo_random(seed.wrapping_add(5))),
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
            let velocity = zzz_velocity_xy(life_t, p.velocity_x_random, p.velocity_y_random);
            p.x += velocity.x * dt;
            p.y += velocity.y * dt;
            let force = zzz_force_xy(life_t, p.velocity_x_random, p.velocity_y_random);
            p.x += 0.5 * force.x * dt * dt;
            p.y += 0.5 * force.y * dt * dt;
            p.rot += p.rot_speed * dt;
            zi += 1;
        }
    }
}

/// Draw Zzz particles (textured rotated quads from Particles_Sheet_01.png).
pub(crate) fn draw_zzz_particles(
    particles: &[ZzzParticle],
    source_bird_index: Option<usize>,
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_id: Option<egui::TextureId>,
) {
    let (tiles_x, tiles_y, row_index) = zzz_uv_layout();
    for p in particles {
        if source_bird_index.is_some_and(|source_index| p.source_bird_index != source_index) {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn zzz_velocity_sampling_uses_prefab_y_random_range() {
        let low = zzz_velocity_xy(0.0, 0.0, 0.0);
        let high = zzz_velocity_xy(0.0, 0.0, 1.0);

        assert_close(low.y, 0.49414596);
        assert_close(high.y, 0.3103452);
    }
}
