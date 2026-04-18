//! Particle systems: fan wind, leaf wind, bird Zzz particles.

use eframe::egui;

use crate::types::Vec2;

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
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub wobble_phase: f32,
    pub wobble_freq: f32,
    pub rot: f32,
    pub rot_speed: f32,
}

/// Wind area zone definition.
pub(super) struct WindAreaDef {
    pub center_x: f32,
    pub center_y: f32,
    pub half_w: f32,
    pub half_h: f32,
}

/// Leaf UV frame within 16×16 Particles_Sheet_01 atlas.
/// Row 2 from top → UV Y = 13/16, columns 4/5/6.
pub(super) const LEAF_TILES: f32 = 16.0;
pub(super) const LEAF_ROW_UV: f32 = 13.0 / 16.0; // (16 - 2 - 1) / 16
pub(super) const LEAF_COLS: [u8; 3] = [4, 5, 6];

/// A single wind leaf particle.
pub(super) struct WindParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age: f32,
    pub lifetime: f32,
    pub rot: f32,
    pub rot_speed: f32,
    pub y_phase: f32,
    pub size: f32,
    /// Which leaf column (0..3 index into LEAF_COLS) for UV frame selection.
    pub leaf_col: u8,
}

/// Spawn a wind leaf particle in the given area.
pub(super) fn spawn_wind_particle(area: &WindAreaDef, particles: &mut Vec<WindParticle>) {
    let seed = particles.len() as u32;
    let x = area.center_x - area.half_w + pseudo_random(seed.wrapping_mul(3)) * area.half_w * 0.3;
    let y = area.center_y - area.half_h
        + pseudo_random(seed.wrapping_mul(7).wrapping_add(1)) * area.half_h * 2.0;
    let size = 0.4 + pseudo_random(seed.wrapping_mul(11).wrapping_add(5)) * 0.3;
    let speed = 6.0 + pseudo_random(seed.wrapping_mul(13).wrapping_add(9)) * 3.0;
    let angle = -0.15 + pseudo_random(seed.wrapping_mul(17).wrapping_add(3)) * 0.3;
    let leaf_col = (pseudo_random(seed.wrapping_mul(31).wrapping_add(13)) * 3.0) as u8;
    particles.push(WindParticle {
        x,
        y,
        vx: speed * angle.cos(),
        vy: speed * angle.sin() * 0.3,
        age: 0.0,
        lifetime: 3.5 + pseudo_random(seed.wrapping_mul(19).wrapping_add(7)) * 2.0,
        rot: 0.0,
        rot_speed: (0.17 + pseudo_random(seed.wrapping_mul(23)) * 2.97)
            * if seed.is_multiple_of(2) { 1.0 } else { -1.0 },
        y_phase: pseudo_random(seed.wrapping_mul(29).wrapping_add(11)),
        size,
        leaf_col: leaf_col.min(2),
    });
}

/// Fan state machine (mirrors Fan.cs Update).
#[derive(Clone, Copy, PartialEq)]
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
    /// Normalized force (0..1).
    pub force: f32,
    /// Whether particle emission is on.
    pub emitting: bool,
    /// Propeller rotation angle (rad).
    pub angle: f32,
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
    pub age: f32,
    pub lifetime: f32,
    pub start_size: f32,
    pub rot: f32,
    pub rot_speed: f32,
}

// Zzz particle constants (used in both update and draw)
pub(super) const ZZZ_SIZE_PEAK_T: f32 = 0.355244;

use super::LevelRenderer;

impl LevelRenderer {
    /// Advance all particle systems by `dt` seconds (fan, wind, zzz).
    pub(super) fn update_particles(&mut self, dt: f32) {
        // ── Fan state machine update ──
        for emitter in &mut self.fan_emitters {
            const SPINDOWN_TIME: f32 = 2.0;
            match emitter.state {
                FanState::DelayedStart => {
                    emitter.counter += dt;
                    if emitter.counter >= emitter.delayed_start {
                        emitter.state = FanState::SpinUp;
                        emitter.counter = 0.0;
                        emitter.emitting = true;
                    }
                }
                FanState::SpinUp => {
                    emitter.counter += dt;
                    if emitter.counter >= emitter.start_time {
                        emitter.state = FanState::Spinning;
                        emitter.counter = 0.0;
                        emitter.force = 1.0;
                    } else {
                        let t = emitter.counter / emitter.start_time;
                        emitter.force = t * t; // spinupRamp: t²
                    }
                }
                FanState::Spinning => {
                    emitter.force = 1.0;
                    if !emitter.always_on {
                        emitter.counter += dt;
                        if emitter.counter >= emitter.on_time {
                            emitter.emitting = false;
                            emitter.state = FanState::SpinDown;
                            emitter.counter = 0.0;
                        }
                    }
                }
                FanState::SpinDown => {
                    emitter.counter += dt;
                    let t = (emitter.counter / SPINDOWN_TIME).min(1.0);
                    emitter.force = 1.0 - t;
                    if t >= 1.0 {
                        emitter.state = FanState::Inactive;
                        emitter.counter = 0.0;
                        emitter.force = 0.0;
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
            // Update propeller angle
            const FAN_ROTATION_SPEED: f32 = 600.0 * std::f32::consts::PI / 180.0; // 10.472 rad/s
            emitter.angle += FAN_ROTATION_SPEED * emitter.force * dt;
        }

        // ── Fan particle burst emission ──
        const BURST_TIMES: [f32; 4] = [0.00, 0.25, 0.50, 0.75];
        const FAN_PARTICLE_MAX: usize = 40;
        for ei in 0..self.fan_emitters.len() {
            let prev_t = self.fan_emitters[ei].burst_time;
            self.fan_emitters[ei].burst_time += dt;
            if self.fan_emitters[ei].burst_time >= 1.0 {
                self.fan_emitters[ei].burst_time -= 1.0;
            }
            let new_t = self.fan_emitters[ei].burst_time;
            if !self.fan_emitters[ei].emitting {
                continue;
            }
            for &bt in &BURST_TIMES {
                let fired = (prev_t <= bt && new_t > bt)
                    || (prev_t > new_t && (prev_t <= bt || new_t > bt));
                if fired && self.fan_particles.len() < FAN_PARTICLE_MAX {
                    let e = &self.fan_emitters[ei];
                    let seed = (self.time * 1000.0) as u32
                        + ei as u32 * 773
                        + self.fan_particles.len() as u32 * 419;
                    let r1 = pseudo_random(seed);
                    let r2 = pseudo_random(seed.wrapping_add(1));
                    let r3 = pseudo_random(seed.wrapping_add(2));
                    let r4 = pseudo_random(seed.wrapping_add(3));
                    let ox = (r1 - 0.5) * 0.98; // ±0.49 spread along X
                    let ly = 0.6365_f32; // local Y offset from fan center
                    let cos_r = e.rot.cos();
                    let sin_r = e.rot.sin();
                    let px = e.world_x + ox * cos_r - ly * sin_r;
                    let py = e.world_y + ox * sin_r + ly * cos_r;
                    let local_vy = 3.0 + r2 * 7.0;
                    let local_vx = (r3 - 0.5) * 0.2;
                    let vx = local_vx * cos_r - local_vy * sin_r;
                    let vy = local_vx * sin_r + local_vy * cos_r;
                    self.fan_particles.push(FanParticle {
                        x: px,
                        y: py,
                        vx,
                        vy,
                        age: 0.0,
                        lifetime: 0.7 + r4 * 0.8,
                        start_size: 1.2,
                        rot: pseudo_random(seed.wrapping_add(4)) * std::f32::consts::PI,
                        rot_speed: std::f32::consts::FRAC_PI_4
                            + pseudo_random(seed.wrapping_add(5)) * std::f32::consts::PI * 0.75,
                    });
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
            p.vy -= 0.5 * dt; // gravity/deceleration
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            let spin_rate = p.rot_speed * (1.0 - t * 0.85);
            p.rot += spin_rate * dt;
            fi += 1;
        }

        // ── Wind leaf particle update ──
        // Spawn new particles
        for a in 0..self.wind_areas.len() {
            self.wind_spawn_accum[a] += dt;
            let area_count = self
                .wind_particles
                .iter()
                .filter(|p| {
                    (p.x - self.wind_areas[a].center_x).abs() < self.wind_areas[a].half_w * 1.5
                })
                .count();
            if self.wind_spawn_accum[a] >= 1.0 && area_count < 20 {
                self.wind_spawn_accum[a] -= 1.0;
                spawn_wind_particle(&self.wind_areas[a], &mut self.wind_particles);
            }
        }
        // Update particles
        let mut i = 0;
        while i < self.wind_particles.len() {
            let p = &mut self.wind_particles[i];
            p.age += dt;
            if p.age >= p.lifetime {
                self.wind_particles.swap_remove(i);
                continue;
            }
            let t_frac = p.age / p.lifetime;
            p.x += p.vx * dt;
            let y_osc = ((t_frac + p.y_phase) * std::f32::consts::TAU).sin() * 0.5;
            p.y += (p.vy + y_osc) * dt;
            p.rot += p.rot_speed * dt;
            i += 1;
        }

        // ── Zzz particle update (bird sleeping) ──
        const ZZZ_EMIT_RATE: f32 = 2.0;
        const ZZZ_MAX_PER_BIRD: usize = 5;
        const ZZZ_START_SIZE: f32 = 0.49; // 0.7 * 0.7
        const ZZZ_SPAWN_OFFSET_Y: f32 = 0.5;
        const ZZZ_SPAWN_SPREAD_X: f32 = 0.6;
        const ZZZ_SPAWN_SPREAD_Y: f32 = 0.52;

        // Spawn new Zzz particles
        for bi in 0..self.bird_positions.len() {
            if bi < self.zzz_emit_accum.len() {
                self.zzz_emit_accum[bi] += dt;
                while self.zzz_emit_accum[bi] >= 1.0 / ZZZ_EMIT_RATE
                    && self.zzz_particles.len() < ZZZ_MAX_PER_BIRD * self.bird_positions.len()
                {
                    self.zzz_emit_accum[bi] -= 1.0 / ZZZ_EMIT_RATE;
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
                    self.zzz_particles.push(ZzzParticle {
                        x: bx + (r1 - 0.5) * 2.0 * ZZZ_SPAWN_SPREAD_X,
                        y: by + ZZZ_SPAWN_OFFSET_Y + (r2 - 0.5) * 2.0 * ZZZ_SPAWN_SPREAD_Y,
                        vy: 0.31 + r3 * 0.18,
                        age: 0.0,
                        lifetime: 1.0 + r4,
                        start_size: ZZZ_START_SIZE,
                        wobble_phase: r5 * std::f32::consts::TAU,
                        wobble_freq: 0.8 + pseudo_random(seed.wrapping_add(5)) * 0.4,
                        rot: 0.0, // Unity startRotation = 0 (constant)
                        rot_speed: pseudo_random(seed.wrapping_add(6)) * 30.0_f32.to_radians(),
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
            // X wobble: velocity curve from VelocityModule, scalar=2, amplitude ~0.7
            let wobble_vx =
                (p.wobble_phase + p.age * p.wobble_freq * std::f32::consts::TAU).sin() * 1.4;
            p.x += wobble_vx * dt;
            p.y += p.vy * dt;
            p.rot += p.rot_speed * dt;
            zi += 1;
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
    for p in particles {
        let life_t = p.age / p.lifetime;
        let size_scale = if life_t < ZZZ_SIZE_PEAK_T {
            let g = life_t / ZZZ_SIZE_PEAK_T;
            g * g * (3.0 - 2.0 * g)
        } else {
            let s = (life_t - ZZZ_SIZE_PEAK_T) / (1.0 - ZZZ_SIZE_PEAK_T);
            1.0 - s * s * (3.0 - 2.0 * s)
        };
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
            const ZZZ_U0: f32 = 6.0 / 8.0;
            const ZZZ_U1: f32 = 7.0 / 8.0;
            const ZZZ_V0: f32 = 1.0 - 6.0 / 8.0;
            const ZZZ_V1: f32 = 1.0 - 5.0 / 8.0;
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
                uv: egui::pos2(ZZZ_U0, ZZZ_V0),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tr,
                uv: egui::pos2(ZZZ_U1, ZZZ_V0),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: br,
                uv: egui::pos2(ZZZ_U1, ZZZ_V1),
                color: tint,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: bl,
                uv: egui::pos2(ZZZ_U0, ZZZ_V1),
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
    for p in particles {
        let t_frac = p.age / p.lifetime;
        let size_scale = if t_frac < 0.136 {
            t_frac / 0.136 * 0.32
        } else if t_frac < 0.845 {
            0.32 + (t_frac - 0.136) / (0.845 - 0.136) * 0.18
        } else if t_frac < 0.913 {
            0.50 + (t_frac - 0.845) / (0.913 - 0.845) * 0.20
        } else {
            0.70 * (1.0 - (t_frac - 0.913) / (1.0 - 0.913))
        } * 0.2;
        let sz = p.start_size * size_scale;
        let center = camera.world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
        if !rect.expand(30.0).contains(center) {
            continue;
        }
        let alpha = if t_frac > 0.85 {
            ((1.0 - t_frac) / 0.15 * 255.0) as u8
        } else {
            255
        };
        let hw = sz * camera.zoom * 0.5;
        let hh = hw;
        if let Some(tex_id) = tex_id {
            let u0 = 3.0 / 8.0;
            let u1 = 4.0 / 8.0;
            let v0 = 0.0 / 8.0;
            let v1 = 1.0 / 8.0;
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
    for p in particles {
        let t_frac = p.age / p.lifetime;
        let alpha = if t_frac < 0.056 {
            t_frac / 0.056
        } else if t_frac > 0.85 {
            (1.0 - t_frac) / 0.15
        } else {
            1.0
        };
        let sz = p.size * camera.zoom;
        if sz < 0.5 {
            continue;
        }
        let center = camera.world_to_screen(Vec2 { x: p.x, y: p.y }, canvas_center);
        if !rect.expand(20.0).contains(center) {
            continue;
        }

        let col = LEAF_COLS[p.leaf_col as usize] as f32;
        let u0 = col / LEAF_TILES;
        let u1 = (col + 1.0) / LEAF_TILES;
        let v0 = 1.0 - LEAF_ROW_UV - 1.0 / LEAF_TILES;
        let v1 = 1.0 - LEAF_ROW_UV;

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
