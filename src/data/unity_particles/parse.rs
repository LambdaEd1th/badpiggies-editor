//! YAML prefab parsing for Unity particle systems.

use std::collections::HashMap;

use serde_yaml::{Mapping, Value};

use crate::data::assets;
use crate::data::unity_anim::HermiteKey;
use crate::domain::types::{Vec2, Vec3};

use super::prefabs::{
    BirdSleepParticlePrefab, FanPuffParticlePrefab, GenericParticlePrefab, WindAreaParticlePrefab,
};
use super::types::{
    ParticleBurst, ParticleColor, ParticleColorGradient, ParticleCurve, ParticleUvModule,
    UnityColorGradient, UnityParticleSystemDef,
};

#[derive(Debug, Clone)]
struct GameObjectInfo {
    name: String,
    active: bool,
}

#[derive(Debug, Clone)]
struct TransformInfo {
    game_object_id: String,
    local_position: Vec3,
    local_rotation: [f32; 4],
}

#[derive(Debug, Clone)]
struct ParticleSystemDoc {
    game_object_id: String,
    duration: f32,
    play_on_awake: bool,
    prewarm: bool,
    looping: bool,
    max_particles: usize,
    start_lifetime: ParticleCurve,
    start_speed: ParticleCurve,
    start_color: ParticleColorGradient,
    start_size: ParticleCurve,
    start_rotation: ParticleCurve,
    emission_rate: ParticleCurve,
    color_over_lifetime_enabled: bool,
    color_over_lifetime: ParticleColorGradient,
    size_over_lifetime: ParticleCurve,
    rotation_over_lifetime: ParticleCurve,
    velocity_x: ParticleCurve,
    velocity_y: ParticleCurve,
    velocity_z: ParticleCurve,
    velocity_world_space: bool,
    force_x: ParticleCurve,
    force_y: ParticleCurve,
    force_z: ParticleCurve,
    force_world_space: bool,
    shape_scale: Vec3,
    shape_radius: f32,
    bursts: Vec<ParticleBurst>,
    uv_module: ParticleUvModule,
}

#[derive(Debug, Clone)]
struct MonoBehaviourDoc {
    game_object_id: String,
    fields: Mapping,
}

#[derive(Default)]
struct ParsedParticlePrefab {
    root_game_object_id: Option<String>,
    game_objects: HashMap<String, GameObjectInfo>,
    transforms: HashMap<String, TransformInfo>,
    particle_systems: Vec<ParticleSystemDoc>,
    mono_behaviours: Vec<MonoBehaviourDoc>,
}

pub(super) fn load_bird_sleep_prefab(asset_key: &str) -> Option<BirdSleepParticlePrefab> {
    let text = assets::read_asset_text(asset_key)?;
    let parsed = parse_prefab(&text);
    let system = parsed.particle_systems.iter().find_map(|doc| {
        let game_object = parsed.game_objects.get(&doc.game_object_id)?;
        if game_object.name != "Particles_Bird_Sleep" || !game_object.active {
            return None;
        }
        let transform = parsed
            .transforms
            .values()
            .find(|transform| transform.game_object_id == doc.game_object_id)?;
        Some(build_particle_system(doc, game_object, transform))
    })?;
    Some(BirdSleepParticlePrefab { system })
}

pub(super) fn load_fan_puff_prefab(asset_key: &str) -> Option<FanPuffParticlePrefab> {
    let text = assets::read_asset_text(asset_key)?;
    let parsed = parse_prefab(&text);
    let system = parsed.particle_systems.iter().find_map(|doc| {
        let game_object = parsed.game_objects.get(&doc.game_object_id)?;
        if game_object.name != "Particles" || !game_object.active {
            return None;
        }
        let transform = parsed
            .transforms
            .values()
            .find(|transform| transform.game_object_id == doc.game_object_id)?;
        Some(build_particle_system(doc, game_object, transform))
    })?;
    Some(FanPuffParticlePrefab { system })
}

pub(super) fn load_generic_particle_prefab(asset_key: &str) -> Option<GenericParticlePrefab> {
    let text = assets::read_asset_text(asset_key)?;
    let parsed = parse_prefab(&text);
    let mut systems = Vec::new();

    for doc in &parsed.particle_systems {
        let Some(game_object) = parsed.game_objects.get(&doc.game_object_id) else {
            continue;
        };
        if !game_object.active {
            continue;
        }
        let Some(transform) = parsed
            .transforms
            .values()
            .find(|transform| transform.game_object_id == doc.game_object_id)
        else {
            continue;
        };
        let transform = if parsed.root_game_object_id.as_deref() == Some(doc.game_object_id.as_str())
        {
            TransformInfo {
                game_object_id: transform.game_object_id.clone(),
                local_position: Vec3::default(),
                local_rotation: [0.0, 0.0, 0.0, 1.0],
            }
        } else {
            transform.clone()
        };
        systems.push(build_particle_system(doc, game_object, &transform));
    }

    (!systems.is_empty()).then_some(GenericParticlePrefab { systems })
}

pub(super) fn load_wind_area_prefab(asset_key: &str) -> Option<WindAreaParticlePrefab> {
    let text = assets::read_asset_text(asset_key)?;
    let parsed = parse_prefab(&text);

    let root_transform = parsed.transforms.values().find(|transform| {
        parsed
            .game_objects
            .get(&transform.game_object_id)
            .is_some_and(|game_object| game_object.name == "WindArea")
    })?;

    let root_fields = parsed.mono_behaviours.iter().find_map(|doc| {
        (doc.game_object_id == root_transform.game_object_id).then_some(&doc.fields)
    })?;

    let handle = map_get(root_fields, "windDirectionHandle")
        .and_then(value_as_vec3)
        .unwrap_or_default();
    let wind_direction = Vec2 {
        x: handle.x - root_transform.local_position.x,
        y: handle.z - root_transform.local_position.z,
    };
    let power_factor = map_get(root_fields, "m_windPowerFactor")
        .and_then(value_as_f32)
        .unwrap_or(1.0);

    let mut systems = Vec::new();
    for doc in &parsed.particle_systems {
        let Some(game_object) = parsed.game_objects.get(&doc.game_object_id) else {
            continue;
        };
        if !game_object.active || !game_object.name.starts_with("WindEffect") {
            continue;
        }
        let Some(transform) = parsed
            .transforms
            .values()
            .find(|transform| transform.game_object_id == doc.game_object_id)
        else {
            continue;
        };
        systems.push(build_particle_system(doc, game_object, transform));
    }

    (!systems.is_empty()).then_some(WindAreaParticlePrefab {
        wind_direction,
        power_factor,
        systems,
    })
}

fn build_particle_system(
    doc: &ParticleSystemDoc,
    game_object: &GameObjectInfo,
    transform: &TransformInfo,
) -> UnityParticleSystemDef {
    UnityParticleSystemDef {
        name: game_object.name.clone(),
        local_position: transform.local_position,
        local_rotation: transform.local_rotation,
        duration: doc.duration,
        play_on_awake: doc.play_on_awake,
        prewarm: doc.prewarm,
        looping: doc.looping,
        max_particles: doc.max_particles,
        start_lifetime: doc.start_lifetime.clone(),
        start_speed: doc.start_speed.clone(),
        start_color: doc.start_color.clone(),
        start_size: doc.start_size.clone(),
        start_rotation: doc.start_rotation.clone(),
        emission_rate: doc.emission_rate.clone(),
        color_over_lifetime_enabled: doc.color_over_lifetime_enabled,
        color_over_lifetime: doc.color_over_lifetime.clone(),
        size_over_lifetime: doc.size_over_lifetime.clone(),
        rotation_over_lifetime: doc.rotation_over_lifetime.clone(),
        velocity_x: doc.velocity_x.clone(),
        velocity_y: doc.velocity_y.clone(),
        velocity_z: doc.velocity_z.clone(),
        velocity_world_space: doc.velocity_world_space,
        force_x: doc.force_x.clone(),
        force_y: doc.force_y.clone(),
        force_z: doc.force_z.clone(),
        force_world_space: doc.force_world_space,
        shape_scale: doc.shape_scale,
        shape_radius: doc.shape_radius,
        bursts: doc.bursts.clone(),
        uv_module: doc.uv_module.clone(),
    }
}

fn parse_prefab(text: &str) -> ParsedParticlePrefab {
    let mut parsed = ParsedParticlePrefab::default();

    for doc in text.split("--- ").skip(1) {
        let mut lines = doc.lines();
        let Some(header) = lines.next().map(str::trim) else {
            continue;
        };
        let Some((type_id, file_id)) = parse_doc_header(header) else {
            continue;
        };

        let body = lines.collect::<Vec<_>>().join("\n");
        let Ok(value) = serde_yaml::from_str::<Value>(&body) else {
            continue;
        };
        let Some((kind, fields)) = doc_root_mapping(&value) else {
            continue;
        };

        match (type_id, kind) {
            (1001, "Prefab") => {
                parsed.root_game_object_id = parse_prefab_root_game_object(fields);
            }
            (1, "GameObject") => {
                if let Some(info) = parse_game_object(fields, &file_id) {
                    parsed.game_objects.insert(info.0, info.1);
                }
            }
            (4, "Transform") => {
                if let Some(info) = parse_transform(fields, &file_id) {
                    parsed.transforms.insert(info.0, info.1);
                }
            }
            (198, "ParticleSystem") => {
                if let Some(info) = parse_particle_system(fields) {
                    parsed.particle_systems.push(info);
                }
            }
            (114, "MonoBehaviour") => {
                if let Some(info) = parse_mono_behaviour(fields) {
                    parsed.mono_behaviours.push(info);
                }
            }
            _ => {}
        }
    }

    parsed
}

fn parse_game_object(fields: &Mapping, file_id: &str) -> Option<(String, GameObjectInfo)> {
    let name = map_get(fields, "m_Name")?.as_str()?.to_string();
    let active = map_get(fields, "m_IsActive")
        .and_then(value_as_bool)
        .unwrap_or(true);
    Some((file_id.to_string(), GameObjectInfo { name, active }))
}

fn parse_prefab_root_game_object(fields: &Mapping) -> Option<String> {
    parse_file_id(map_get(fields, "m_RootGameObject")?)
}

fn parse_transform(fields: &Mapping, file_id: &str) -> Option<(String, TransformInfo)> {
    let game_object_id = parse_file_id(map_get(fields, "m_GameObject")?)?;
    let local_position = map_get(fields, "m_LocalPosition")
        .and_then(value_as_vec3)
        .unwrap_or_default();
    let local_rotation = map_get(fields, "m_LocalRotation")
        .and_then(value_as_quat)
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    Some((
        file_id.to_string(),
        TransformInfo {
            game_object_id,
            local_position,
            local_rotation,
        },
    ))
}

fn parse_particle_system(fields: &Mapping) -> Option<ParticleSystemDoc> {
    let game_object_id = parse_file_id(map_get(fields, "m_GameObject")?)?;
    let initial = map_get(fields, "InitialModule").and_then(Value::as_mapping);
    let emission = map_get(fields, "EmissionModule").and_then(Value::as_mapping);
    let color_module = map_get(fields, "ColorModule").and_then(Value::as_mapping);
    let size_module = map_get(fields, "SizeModule").and_then(Value::as_mapping);
    let rotation_module = map_get(fields, "RotationModule").and_then(Value::as_mapping);
    let velocity_module = map_get(fields, "VelocityModule").and_then(Value::as_mapping);
    let force_module = map_get(fields, "ForceModule").and_then(Value::as_mapping);
    let shape_module = map_get(fields, "ShapeModule").and_then(Value::as_mapping);
    let uv_module = map_get(fields, "UVModule").and_then(Value::as_mapping);

    Some(ParticleSystemDoc {
        game_object_id,
        duration: map_get(fields, "lengthInSec")
            .and_then(value_as_f32)
            .unwrap_or(1.0),
        play_on_awake: map_get(fields, "playOnAwake")
            .and_then(value_as_bool)
            .unwrap_or(true),
        prewarm: map_get(fields, "prewarm")
            .and_then(value_as_bool)
            .unwrap_or(false),
        looping: map_get(fields, "looping")
            .and_then(value_as_bool)
            .unwrap_or(true),
        max_particles: initial
            .and_then(|map| map_get(map, "maxNumParticles"))
            .and_then(value_as_i64)
            .unwrap_or(0)
            .max(0) as usize,
        start_lifetime: initial
            .and_then(|map| map_get(map, "startLifetime"))
            .map(|value| parse_particle_curve(value, 1.0))
            .unwrap_or_else(|| ParticleCurve::constant(1.0)),
        start_speed: initial
            .and_then(|map| map_get(map, "startSpeed"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        start_color: initial
            .and_then(|map| map_get(map, "startColor"))
            .map(parse_particle_color_gradient)
            .unwrap_or_else(|| {
                ParticleColorGradient::constant(ParticleColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            }),
        start_size: initial
            .and_then(|map| map_get(map, "startSize"))
            .map(|value| parse_particle_curve(value, 1.0))
            .unwrap_or_else(|| ParticleCurve::constant(1.0)),
        start_rotation: initial
            .and_then(|map| map_get(map, "startRotation"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        emission_rate: emission
            .and_then(|map| map_get(map, "rateOverTime"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        color_over_lifetime_enabled: color_module
            .and_then(|map| map_get(map, "enabled"))
            .and_then(value_as_bool)
            .unwrap_or(false),
        color_over_lifetime: color_module
            .and_then(|map| map_get(map, "gradient"))
            .map(parse_particle_color_gradient)
            .unwrap_or_else(|| {
                ParticleColorGradient::constant(ParticleColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            }),
        size_over_lifetime: size_module
            .and_then(|map| map_get(map, "curve"))
            .map(|value| parse_particle_curve(value, 1.0))
            .unwrap_or_else(|| ParticleCurve::constant(1.0)),
        rotation_over_lifetime: rotation_module
            .and_then(|map| map_get(map, "curve"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        velocity_x: velocity_module
            .and_then(|map| map_get(map, "x"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        velocity_y: velocity_module
            .and_then(|map| map_get(map, "y"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        velocity_z: velocity_module
            .and_then(|map| map_get(map, "z"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        velocity_world_space: velocity_module
            .and_then(|map| map_get(map, "inWorldSpace"))
            .and_then(value_as_bool)
            .unwrap_or(false),
        force_x: force_module
            .and_then(|map| map_get(map, "x"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        force_y: force_module
            .and_then(|map| map_get(map, "y"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        force_z: force_module
            .and_then(|map| map_get(map, "z"))
            .map(|value| parse_particle_curve(value, 0.0))
            .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        force_world_space: force_module
            .and_then(|map| map_get(map, "inWorldSpace"))
            .and_then(value_as_bool)
            .unwrap_or(false),
        shape_scale: shape_module
            .and_then(|map| map_get(map, "m_Scale"))
            .and_then(value_as_vec3)
            .unwrap_or(Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            }),
        shape_radius: shape_module
            .and_then(|map| map_get(map, "radius"))
            .and_then(Value::as_mapping)
            .and_then(|map| map_get(map, "value"))
            .and_then(value_as_f32)
            .unwrap_or(1.0),
        bursts: emission.map(parse_particle_bursts).unwrap_or_default(),
        uv_module: ParticleUvModule {
            tiles_x: uv_module
                .and_then(|map| map_get(map, "tilesX"))
                .and_then(value_as_i64)
                .unwrap_or(1)
                .max(1) as u32,
            tiles_y: uv_module
                .and_then(|map| map_get(map, "tilesY"))
                .and_then(value_as_i64)
                .unwrap_or(1)
                .max(1) as u32,
            row_index: uv_module
                .and_then(|map| map_get(map, "rowIndex"))
                .and_then(value_as_i64)
                .unwrap_or(0)
                .max(0) as u32,
            animation_type: uv_module
                .and_then(|map| map_get(map, "animationType"))
                .and_then(value_as_i64)
                .unwrap_or(0) as i32,
            frame_over_time: uv_module
                .and_then(|map| map_get(map, "frameOverTime"))
                .map(|value| parse_particle_curve(value, 0.0))
                .unwrap_or_else(|| ParticleCurve::constant(0.0)),
        },
    })
}

fn parse_particle_bursts(map: &Mapping) -> Vec<ParticleBurst> {
    let Some(sequence) = map_get(map, "m_Bursts").and_then(Value::as_sequence) else {
        return Vec::new();
    };

    sequence
        .iter()
        .filter_map(Value::as_mapping)
        .map(|burst| ParticleBurst {
            time: map_get(burst, "time").and_then(value_as_f32).unwrap_or(0.0),
            count: map_get(burst, "countCurve")
                .map(|value| parse_particle_curve(value, 0.0))
                .unwrap_or_else(|| ParticleCurve::constant(0.0)),
            cycle_count: map_get(burst, "cycleCount")
                .and_then(value_as_i64)
                .unwrap_or(1)
                .max(0) as u32,
            repeat_interval: map_get(burst, "repeatInterval")
                .and_then(value_as_f32)
                .unwrap_or(0.0),
        })
        .collect()
}

fn parse_mono_behaviour(fields: &Mapping) -> Option<MonoBehaviourDoc> {
    let game_object_id = parse_file_id(map_get(fields, "m_GameObject")?)?;
    Some(MonoBehaviourDoc {
        game_object_id,
        fields: fields.clone(),
    })
}

fn parse_particle_curve(value: &Value, default_scalar: f32) -> ParticleCurve {
    let Some(map) = value.as_mapping() else {
        return ParticleCurve::constant(default_scalar);
    };

    ParticleCurve {
        mode: map_get(map, "minMaxState")
            .and_then(value_as_i64)
            .unwrap_or(0) as i32,
        scalar: map_get(map, "scalar")
            .and_then(value_as_f32)
            .unwrap_or(default_scalar),
        min_scalar: map_get(map, "minScalar")
            .and_then(value_as_f32)
            .unwrap_or(default_scalar),
        max_curve: map_get(map, "maxCurve")
            .and_then(Value::as_mapping)
            .map(parse_hermite_keys)
            .unwrap_or_default(),
        min_curve: map_get(map, "minCurve")
            .and_then(Value::as_mapping)
            .map(parse_hermite_keys)
            .unwrap_or_default(),
    }
}

fn parse_particle_color_gradient(value: &Value) -> ParticleColorGradient {
    let Some(map) = value.as_mapping() else {
        return ParticleColorGradient::constant(ParticleColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
    };

    let min_color = map_get(map, "minColor")
        .and_then(value_as_particle_color)
        .unwrap_or(ParticleColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
    let max_color = map_get(map, "maxColor")
        .and_then(value_as_particle_color)
        .unwrap_or(min_color);

    ParticleColorGradient {
        mode: map_get(map, "minMaxState")
            .and_then(value_as_i64)
            .unwrap_or(0) as i32,
        min_color,
        max_color,
        min_gradient: map_get(map, "minGradient")
            .and_then(Value::as_mapping)
            .map(parse_unity_color_gradient)
            .unwrap_or_else(|| UnityColorGradient::constant(min_color)),
        max_gradient: map_get(map, "maxGradient")
            .and_then(Value::as_mapping)
            .map(parse_unity_color_gradient)
            .unwrap_or_else(|| UnityColorGradient::constant(max_color)),
    }
}

fn parse_unity_color_gradient(map: &Mapping) -> UnityColorGradient {
    let color_key_count = map_get(map, "m_NumColorKeys")
        .and_then(value_as_i64)
        .unwrap_or(2)
        .clamp(0, 8) as usize;
    let alpha_key_count = map_get(map, "m_NumAlphaKeys")
        .and_then(value_as_i64)
        .unwrap_or(2)
        .clamp(0, 8) as usize;

    let mut color_keys = Vec::with_capacity(color_key_count.max(2));
    for index in 0..color_key_count {
        let key_name = format!("key{index}");
        let time_name = format!("ctime{index}");
        let Some(color) = map_get(map, &key_name).and_then(value_as_particle_color) else {
            continue;
        };
        let time = map_get(map, &time_name)
            .and_then(value_as_f32)
            .unwrap_or(0.0)
            / 65535.0;
        color_keys.push((time.clamp(0.0, 1.0), color));
    }

    let mut alpha_keys = Vec::with_capacity(alpha_key_count.max(2));
    for index in 0..alpha_key_count {
        let key_name = format!("key{index}");
        let time_name = format!("atime{index}");
        let Some(color) = map_get(map, &key_name).and_then(value_as_particle_color) else {
            continue;
        };
        let time = map_get(map, &time_name)
            .and_then(value_as_f32)
            .unwrap_or(0.0)
            / 65535.0;
        alpha_keys.push((time.clamp(0.0, 1.0), color.a));
    }

    color_keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    alpha_keys.sort_by(|a, b| a.0.total_cmp(&b.0));

    if color_keys.is_empty() {
        color_keys.push((0.0, ParticleColor::default()));
        color_keys.push((1.0, ParticleColor::default()));
    }
    if alpha_keys.is_empty() {
        alpha_keys.push((0.0, 1.0));
        alpha_keys.push((1.0, 1.0));
    }

    UnityColorGradient {
        color_keys,
        alpha_keys,
    }
}

fn parse_hermite_keys(map: &Mapping) -> Vec<HermiteKey> {
    let Some(sequence) = map_get(map, "m_Curve").and_then(Value::as_sequence) else {
        return Vec::new();
    };

    sequence
        .iter()
        .filter_map(Value::as_mapping)
        .filter_map(|key| {
            Some((
                map_get(key, "time").and_then(value_as_f32)?,
                map_get(key, "value").and_then(value_as_f32)?,
                map_get(key, "inSlope").and_then(value_as_f32)?,
                map_get(key, "outSlope").and_then(value_as_f32)?,
            ))
        })
        .collect()
}

fn parse_doc_header(header: &str) -> Option<(u32, String)> {
    let mut parts = header.split_whitespace();
    let type_id = parts.next()?.strip_prefix("!u!")?.parse().ok()?;
    let file_id = parts.next()?.strip_prefix('&')?.to_string();
    Some((type_id, file_id))
}

fn doc_root_mapping(value: &Value) -> Option<(&str, &Mapping)> {
    let mapping = value.as_mapping()?;
    let (key, fields) = mapping.iter().next()?;
    Some((key.as_str()?, fields.as_mapping()?))
}

fn map_get<'a>(map: &'a Mapping, key: &str) -> Option<&'a Value> {
    map.iter()
        .find_map(|(candidate, value)| (candidate.as_str() == Some(key)).then_some(value))
}

fn parse_file_id(value: &Value) -> Option<String> {
    let mapping = value.as_mapping()?;
    let value = map_get(mapping, "fileID")?;
    if let Some(raw) = value_as_i64(value) {
        return Some(raw.to_string());
    }
    value.as_str().map(str::to_string)
}

fn value_as_bool(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value_as_i64(value).map(|value| value != 0))
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|value| value as i64))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

fn value_as_f32(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| value.as_i64().map(|value| value as f32))
        .or_else(|| value.as_str()?.parse::<f32>().ok())
}

fn value_as_vec3(value: &Value) -> Option<Vec3> {
    let mapping = value.as_mapping()?;
    Some(Vec3 {
        x: map_get(mapping, "x").and_then(value_as_f32).unwrap_or(0.0),
        y: map_get(mapping, "y").and_then(value_as_f32).unwrap_or(0.0),
        z: map_get(mapping, "z").and_then(value_as_f32).unwrap_or(0.0),
    })
}

fn value_as_quat(value: &Value) -> Option<[f32; 4]> {
    let mapping = value.as_mapping()?;
    Some([
        map_get(mapping, "x").and_then(value_as_f32).unwrap_or(0.0),
        map_get(mapping, "y").and_then(value_as_f32).unwrap_or(0.0),
        map_get(mapping, "z").and_then(value_as_f32).unwrap_or(0.0),
        map_get(mapping, "w").and_then(value_as_f32).unwrap_or(1.0),
    ])
}

fn value_as_particle_color(value: &Value) -> Option<ParticleColor> {
    let mapping = value.as_mapping()?;
    Some(ParticleColor {
        r: map_get(mapping, "r").and_then(value_as_f32).unwrap_or(0.0),
        g: map_get(mapping, "g").and_then(value_as_f32).unwrap_or(0.0),
        b: map_get(mapping, "b").and_then(value_as_f32).unwrap_or(0.0),
        a: map_get(mapping, "a").and_then(value_as_f32).unwrap_or(1.0),
    })
}
