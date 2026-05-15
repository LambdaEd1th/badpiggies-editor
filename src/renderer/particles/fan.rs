//! Fan particle systems: state machine, puff emission, draw.

use eframe::egui;

use crate::data::unity_particles;
use crate::domain::types::Vec2;

use super::{Camera, LevelRenderer, particle_sheet_uv_rect, pseudo_random, rotate_vec2};

/// Fan state machine (mirrors Fan.cs Update).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FanState {
    Inactive,
    DelayedStart,
    SpinUp,
    Spinning,
    SpinDown,
}

/// Persistent fan animation state.
pub(crate) struct FanEmitter {
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
pub(crate) struct FanParticle {
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

const FAN_VISUAL_FORCE_MAX: f32 = 10.0;
const FAN_RUNNING_ROTATION_SPEED: f32 = 600.0 * std::f32::consts::PI / 180.0;
const FAN_SPINDOWN_ROTATION_SPEED: f32 = 60.0 * std::f32::consts::PI / 180.0;
const FAN_SPINDOWN_TIME: f32 = 2.0;
const FAN_SPINDOWN_SNAP_EPSILON: f32 = 3.0 * std::f32::consts::PI / 180.0;

fn fan_puff_system() -> &'static unity_particles::UnityParticleSystemDef {
    &unity_particles::fan_puff_prefab()
        .expect("Fan puff particle prefab should be available")
        .system
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

pub(crate) fn reset_fan_emitter_for_build(emitter: &mut FanEmitter) {
    emitter.state = FanState::Inactive;
    emitter.counter = 0.0;
    emitter.force = 0.0;
    emitter.emitting = false;
    emitter.angle = 0.0;
    emitter.spin_down_start_force = 0.0;
    emitter.spin_down_angle_left = 0.0;
    emitter.burst_time = pseudo_random(emitter.sprite_index as u32 * 997);
}

pub(crate) fn start_fan_emitter_for_play(emitter: &mut FanEmitter) {
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

impl LevelRenderer {
    pub(super) fn update_fan_particles(&mut self, dt: f32) {
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
    }
}

/// Draw fan wind particles (cloud puffs from Particles_Sheet_01.png).
pub(crate) fn draw_fan_particles(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
