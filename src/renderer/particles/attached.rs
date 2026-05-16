//! Attached-emitter particle systems (rocket fire, turbo charger, magnet, fly swarm).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use eframe::egui;

use crate::data::assets;
use crate::data::unity_particles;
use crate::data::unity_particles::UnityParticleSystemDef;
use crate::domain::prefab_asset::PrefabAssetDocument;
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

const ATTACHED_EFFECT_KIND_COUNT: usize = 4;
const ATTACHED_BURST_TIME_EPSILON: f32 = 1e-4;
const ATTACHED_EMISSION_SPAWN_EPSILON: f32 = 1e-4;
const GAME_DATA_ASSET: &str = "unity/scriptableobject/GameData.asset";

struct RuntimeEffectPrefabNames {
    super_magnet_effect: Option<String>,
    turbo_charge_effect: Option<String>,
}

fn attached_effect_kind_index(kind: AttachedEffectKind) -> usize {
    match kind {
        AttachedEffectKind::RocketFire => 0,
        AttachedEffectKind::TurboCharger => 1,
        AttachedEffectKind::Magnet => 2,
        AttachedEffectKind::FlySwarm => 3,
    }
}

fn runtime_effect_prefab_names() -> &'static RuntimeEffectPrefabNames {
    static NAMES: OnceLock<RuntimeEffectPrefabNames> = OnceLock::new();
    NAMES.get_or_init(load_runtime_effect_prefab_names)
}

fn load_runtime_effect_prefab_names() -> RuntimeEffectPrefabNames {
    let Some(game_data) = assets::read_asset_text(GAME_DATA_ASSET) else {
        return RuntimeEffectPrefabNames {
            super_magnet_effect: None,
            turbo_charge_effect: None,
        };
    };

    RuntimeEffectPrefabNames {
        super_magnet_effect: game_data_asset_prefab_name(&game_data, "m_superMagnetEffect"),
        turbo_charge_effect: game_data_asset_prefab_name(&game_data, "m_turboChargeEffect"),
    }
}

fn game_data_asset_prefab_name(game_data: &str, field_name: &str) -> Option<String> {
    let file_id = game_data_asset_file_id(game_data, field_name)?;

    for prefab_path in assets::list_asset_paths("Prefab/", ".prefab") {
        let asset_path = format!("unity/prefabs/{prefab_path}");
        let Some(text) = assets::read_asset_text(&asset_path) else {
            continue;
        };
        if prefab_root_file_id(&text).as_deref() == Some(file_id.as_str()) {
            return prefab_path.strip_suffix(".prefab").map(str::to_string);
        }
    }

    None
}

fn game_data_asset_file_id(game_data: &str, field_name: &str) -> Option<String> {
    let prefix = format!("{field_name}: ");
    game_data
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(extract_file_id)
}

fn prefab_root_file_id(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("m_RootGameObject: ")
            .and_then(extract_file_id)
    })
}

fn extract_file_id(value: &str) -> Option<String> {
    let start = value.find("fileID: ")? + "fileID: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    let file_id = tail[..end].trim();
    (!file_id.is_empty()).then(|| file_id.to_string())
}

pub(crate) fn attached_effect_kind_for_sprite_name(name: &str) -> Option<AttachedEffectKind> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<AttachedEffectKind>>>> = OnceLock::new();

    let key = name.split(" (").next().unwrap_or(name).to_string();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(cached) = cache
        .lock()
        .expect("attached effect kind cache poisoned")
        .get(&key)
        .copied()
    {
        return cached;
    }

    let loaded = load_attached_effect_kind(&key);
    cache
        .lock()
        .expect("attached effect kind cache poisoned")
        .insert(key, loaded);
    loaded
}

fn load_attached_effect_kind(prefab_name: &str) -> Option<AttachedEffectKind> {
    let runtime_effects = runtime_effect_prefab_names();

    match prefab_name {
        name if runtime_effects.turbo_charge_effect.as_deref() == Some(name) => {
            return Some(AttachedEffectKind::TurboCharger);
        }
        name if runtime_effects.super_magnet_effect.as_deref() == Some(name) => {
            return Some(AttachedEffectKind::Magnet);
        }
        "SuperMagnet" => return Some(AttachedEffectKind::Magnet),
        name if name.starts_with("FlySwarm") => return Some(AttachedEffectKind::FlySwarm),
        _ => {}
    }

    let asset_path = format!("unity/prefabs/{prefab_name}.prefab");
    let text = assets::read_asset_text(&asset_path)?;
    let prefab = PrefabAssetDocument::parse(&text)?;

    if let Some(rocket) = prefab.root_component("Rocket")
        && (has_nonzero_file_id(rocket.field_file_id("m_particlesIgnitionInstance"))
            || has_nonzero_file_id(rocket.field_file_id("m_particlesFiringInstance")))
    {
        return Some(AttachedEffectKind::RocketFire);
    }

    if let Some(engine) = prefab.root_component("Engine")
        && (has_nonzero_file_id(engine.field_file_id("smokeEmitter"))
            || has_nonzero_file_id(engine.field_file_id("flameEmitter")))
    {
        return Some(AttachedEffectKind::TurboCharger);
    }

    None
}

fn has_nonzero_file_id(file_id: Option<String>) -> bool {
    matches!(file_id.as_deref(), Some(value) if value != "0")
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

fn attached_effect_texture_name(kind: AttachedEffectKind) -> Option<&'static str> {
    let mut names = attached_effect_systems(kind)
        .iter()
        .filter_map(|system| system.texture_name());
    let first = names.next()?;
    names.all(|name| name == first).then_some(first)
}

fn attached_effect_fallback_texture_name(kind: AttachedEffectKind) -> Option<&'static str> {
    match kind {
        AttachedEffectKind::RocketFire => Some(super::super::GLOW_ATLAS),
        AttachedEffectKind::TurboCharger
        | AttachedEffectKind::Magnet
        | AttachedEffectKind::FlySwarm => None,
    }
}

pub(crate) fn attached_effect_texture_names() -> [Option<&'static str>; ATTACHED_EFFECT_KIND_COUNT] {
    [
        attached_effect_texture_name(AttachedEffectKind::RocketFire),
        attached_effect_texture_name(AttachedEffectKind::TurboCharger),
        attached_effect_texture_name(AttachedEffectKind::Magnet),
        attached_effect_texture_name(AttachedEffectKind::FlySwarm),
    ]
}

pub(crate) fn attached_effect_draw_texture_names(
) -> [Option<&'static str>; ATTACHED_EFFECT_KIND_COUNT] {
    [
        attached_effect_texture_name(AttachedEffectKind::RocketFire)
            .or_else(|| attached_effect_fallback_texture_name(AttachedEffectKind::RocketFire)),
        attached_effect_texture_name(AttachedEffectKind::TurboCharger).or_else(|| {
            attached_effect_fallback_texture_name(AttachedEffectKind::TurboCharger)
        }),
        attached_effect_texture_name(AttachedEffectKind::Magnet)
            .or_else(|| attached_effect_fallback_texture_name(AttachedEffectKind::Magnet)),
        attached_effect_texture_name(AttachedEffectKind::FlySwarm)
            .or_else(|| attached_effect_fallback_texture_name(AttachedEffectKind::FlySwarm)),
    ]
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

fn attached_effect_spawn_ready(spawn_accum: f32) -> bool {
    spawn_accum + ATTACHED_EMISSION_SPAWN_EPSILON >= 1.0
}

fn attached_effect_burst_fired(
    prev_time: f32,
    new_time: f32,
    burst_time: f32,
    looping: bool,
) -> bool {
    if (new_time - prev_time).abs() <= f32::EPSILON {
        return false;
    }

    if !looping || prev_time <= new_time {
        (prev_time < burst_time && new_time + ATTACHED_BURST_TIME_EPSILON >= burst_time)
            || (burst_time.abs() <= ATTACHED_BURST_TIME_EPSILON
                && prev_time.abs() <= ATTACHED_BURST_TIME_EPSILON
                && new_time > prev_time)
    } else {
        burst_time > prev_time || new_time + ATTACHED_BURST_TIME_EPSILON >= burst_time
    }
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

fn update_attached_effect_particle(
    particle: &mut AttachedEffectParticle,
    system: &UnityParticleSystemDef,
    dt: f32,
) -> bool {
    particle.age += dt;
    if particle.age >= particle.lifetime {
        return false;
    }
    let life_t = particle.age / particle.lifetime;
    particle.vx += particle.fx * dt;
    particle.vy += particle.fy * dt;
    particle.x += particle.vx * dt;
    particle.y += particle.vy * dt;
    particle.rot += system.rotation_over_lifetime.sample(life_t, particle.rot_random) * dt;
    true
}

fn prewarm_attached_effect_system(
    emitter: &mut AttachedEffectEmitter,
    emitter_index: usize,
    system_index: usize,
    particles: &mut Vec<AttachedEffectParticle>,
) {
    let kind = emitter.kind;
    let system = &attached_effect_systems(kind)[system_index];
    if !system.prewarm {
        return;
    }

    let duration = attached_effect_duration(system);
    let mut local_particles = Vec::new();
    let mut simulated_time = 0.0;
    let mut spawn_accum = 0.0;
    let mut spawn_serial = 0u32;
    let mut remaining = duration;

    while remaining > 0.0 {
        let dt = remaining.min(1.0 / 60.0);
        let prev_time = simulated_time;
        let mut new_time = prev_time + dt;
        if system.looping {
            while new_time >= duration {
                new_time -= duration;
            }
        } else {
            new_time = new_time.min(duration);
        }
        simulated_time = new_time;

        let t_frac = (new_time / duration).clamp(0.0, 1.0);
        spawn_accum += dt * system.emission_rate.sample(t_frac, 0.0).max(0.0);
        while attached_effect_spawn_ready(spawn_accum)
            && local_particles.len() < system.max_particles
        {
            spawn_accum = (spawn_accum - 1.0).max(0.0);
            let seed = emitter_index as u32 * 1009
                + system_index as u32 * 313
                + spawn_serial * 17;
            attached_effect_spawn_particle(
                emitter,
                emitter_index,
                system_index,
                &mut local_particles,
                seed,
            );
            spawn_serial = spawn_serial.wrapping_add(1);
        }

        for (burst_index, burst) in system.bursts.iter().enumerate() {
            let cycle_count = burst.cycle_count.max(1);
            for cycle_index in 0..cycle_count {
                let burst_time = burst.time + cycle_index as f32 * burst.repeat_interval;
                let fired =
                    attached_effect_burst_fired(prev_time, new_time, burst_time, system.looping);
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
                    .min(system.max_particles.saturating_sub(local_particles.len()));
                for particle_index in 0..burst_count {
                    let seed = emitter_index as u32 * 857
                        + system_index as u32 * 131
                        + burst_index as u32 * 59
                        + particle_index as u32 * 23
                        + spawn_serial * 17;
                    attached_effect_spawn_particle(
                        emitter,
                        emitter_index,
                        system_index,
                        &mut local_particles,
                        seed,
                    );
                    spawn_serial = spawn_serial.wrapping_add(1);
                }
            }
        }

        let mut particle_index = 0;
        while particle_index < local_particles.len() {
            if !update_attached_effect_particle(&mut local_particles[particle_index], system, dt) {
                local_particles.swap_remove(particle_index);
                continue;
            }
            particle_index += 1;
        }

        remaining -= dt;
    }

    emitter.system_time[system_index] = simulated_time;
    emitter.spawn_accum[system_index] = spawn_accum;
    particles.extend(local_particles);
}

impl LevelRenderer {
    pub(crate) fn seed_attached_effect_particles(&mut self) {
        self.attached_effect_particles.clear();
        for emitter in &mut self.attached_effect_emitters {
            emitter.system_time.fill(0.0);
            emitter.spawn_accum.fill(0.0);
        }

        for emitter_index in 0..self.attached_effect_emitters.len() {
            let emitter = &mut self.attached_effect_emitters[emitter_index];
            let system_count = attached_effect_systems(emitter.kind).len();
            for system_index in 0..system_count {
                prewarm_attached_effect_system(
                    emitter,
                    emitter_index,
                    system_index,
                    &mut self.attached_effect_particles,
                );
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
                while attached_effect_spawn_ready(emitter.spawn_accum[system_index])
                    && system_count < system.max_particles
                {
                    emitter.spawn_accum[system_index] =
                        (emitter.spawn_accum[system_index] - 1.0).max(0.0);
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
                        let fired = attached_effect_burst_fired(
                            prev_time,
                            new_time,
                            burst_time,
                            system.looping,
                        );
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
            if !update_attached_effect_particle(particle, system, dt) {
                self.attached_effect_particles.swap_remove(attached_index);
                continue;
            }
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
    texture_ids: &[Option<egui::TextureId>; ATTACHED_EFFECT_KIND_COUNT],
) {
    for particle in particles {
        let system = &attached_effect_systems(particle.kind)[particle.system_index];
        let tex_id = texture_ids[attached_effect_kind_index(particle.kind)];
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
        let runtime_effects = runtime_effect_prefab_names();

        assert_eq!(
            runtime_effects.turbo_charge_effect.as_deref(),
            Some("TurboChargerEffect")
        );
        assert_eq!(
            runtime_effects.super_magnet_effect.as_deref(),
            Some("MagnetEffect")
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("Part_Rocket_01_SET"),
            Some(AttachedEffectKind::RocketFire)
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("Part_EngineBig_03_SET"),
            Some(AttachedEffectKind::TurboCharger)
        );
        assert_eq!(
            attached_effect_kind_for_sprite_name("Part_EngineSmall_05_SET"),
            None
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

    #[test]
    fn attached_effect_texture_names_follow_prefab_materials() {
        let texture_names = attached_effect_texture_names();

        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::RocketFire)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::TurboCharger)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::Magnet)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::FlySwarm)],
            Some("Particles_Sheet_01.png")
        );
    }

    #[test]
    fn attached_effect_draw_texture_names_only_fallback_rocket_fire() {
        let texture_names = attached_effect_draw_texture_names();

        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::RocketFire)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::TurboCharger)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::Magnet)],
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            texture_names[attached_effect_kind_index(AttachedEffectKind::FlySwarm)],
            Some("Particles_Sheet_01.png")
        );
    }

    #[test]
    fn rocket_fire_prewarm_tracks_fractional_emission_phase() {
        let mut emitter = AttachedEffectEmitter {
            world_x: 0.0,
            world_y: 0.0,
            rot: 0.0,
            kind: AttachedEffectKind::RocketFire,
            system_time: vec![0.0],
            spawn_accum: vec![0.0],
        };
        let mut particles = Vec::new();
        prewarm_attached_effect_system(&mut emitter, 0, 0, &mut particles);

        assert!((emitter.system_time[0] - 0.0).abs() < 0.0001);
        assert!((emitter.spawn_accum[0] - 0.5).abs() < 0.0001);
        assert_eq!(particles.len(), 2);
        assert!(particles.iter().all(|particle| particle.age > 0.0));
    }

    #[test]
    fn magnet_burst_fires_when_loop_wrap_hits_zero_boundary() {
        let system_count = attached_effect_systems(AttachedEffectKind::Magnet).len();
        let system = &attached_effect_systems(AttachedEffectKind::Magnet)[0];
        let duration = attached_effect_duration(system);
        let dt = 0.00005;

        let mut renderer = LevelRenderer::new(None);
        let mut system_time = vec![0.0; system_count];
        system_time[0] = duration - dt;
        renderer.attached_effect_emitters.push(AttachedEffectEmitter {
            world_x: 0.0,
            world_y: 0.0,
            rot: 0.0,
            kind: AttachedEffectKind::Magnet,
            system_time,
            spawn_accum: vec![0.0; system_count],
        });

        renderer.update_attached_effect_particles(dt);

        let burst_particles = renderer
            .attached_effect_particles
            .iter()
            .filter(|particle| particle.emitter_index == 0 && particle.system_index == 0)
            .count();
        assert!(attached_effect_burst_fired(duration - dt, 0.0, 0.0, true));
        assert_eq!(burst_particles, system.max_particles);
    }

    #[test]
    fn rocket_fire_emission_threshold_spawns_at_float_boundary() {
        let system_count = attached_effect_systems(AttachedEffectKind::RocketFire).len();
        let system = &attached_effect_systems(AttachedEffectKind::RocketFire)[0];
        let rate = system.emission_rate.sample(0.0, 0.0);
        let dt = (ATTACHED_EMISSION_SPAWN_EPSILON * 0.5) / rate;

        let mut renderer = LevelRenderer::new(None);
        renderer.attached_effect_emitters.push(AttachedEffectEmitter {
            world_x: 0.0,
            world_y: 0.0,
            rot: 0.0,
            kind: AttachedEffectKind::RocketFire,
            system_time: vec![0.0; system_count],
            spawn_accum: vec![1.0 - ATTACHED_EMISSION_SPAWN_EPSILON * 0.5; system_count],
        });

        renderer.update_attached_effect_particles(dt);

        let spawned = renderer
            .attached_effect_particles
            .iter()
            .filter(|particle| particle.emitter_index == 0 && particle.system_index == 0)
            .count();
        assert_eq!(spawned, 1);
        assert!(renderer.attached_effect_emitters[0].spawn_accum[0] <= ATTACHED_EMISSION_SPAWN_EPSILON);
    }
}
