use std::collections::HashMap;
use std::sync::OnceLock;

use serde_yaml::{Mapping, Value};

use crate::domain::types::{Vec2, Vec3};

use super::assets;
use super::unity_anim::HermiteKey;

const BIRD_SLEEP_PREFAB_ASSET: &str = "unity/prefabs/Bird_Red.prefab";
const FAN_PREFAB_ASSET: &str = "unity/prefabs/Fan.prefab";
const MAGNET_EFFECT_PREFAB_ASSET: &str = "unity/prefabs/MagnetEffect.prefab";
const ROCKET_FIRE_PREFAB_ASSET: &str = "unity/prefabs/Particles_RocketFire_01_SET.prefab";
const TURBO_CHARGER_PREFAB_ASSET: &str = "unity/prefabs/TurboChargerEffect.prefab";
const WIND_AREA_PREFAB_ASSET: &str = "unity/prefabs/WindArea.prefab";

static BIRD_SLEEP_PREFAB: OnceLock<Option<BirdSleepParticlePrefab>> = OnceLock::new();
static FAN_PUFF_PREFAB: OnceLock<Option<FanPuffParticlePrefab>> = OnceLock::new();
static MAGNET_EFFECT_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static ROCKET_FIRE_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static TURBO_CHARGER_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static WIND_AREA_PREFAB: OnceLock<Option<WindAreaParticlePrefab>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParticleColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ParticleColor {
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: lerp(self.r, other.r, t),
            g: lerp(self.g, other.g, t),
            b: lerp(self.b, other.b, t),
            a: lerp(self.a, other.a, t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnityColorGradient {
    pub color_keys: Vec<(f32, ParticleColor)>,
    pub alpha_keys: Vec<(f32, f32)>,
}

impl UnityColorGradient {
    pub fn constant(color: ParticleColor) -> Self {
        Self {
            color_keys: vec![(0.0, color), (1.0, color)],
            alpha_keys: vec![(0.0, color.a), (1.0, color.a)],
        }
    }

    pub fn sample(&self, time: f32) -> ParticleColor {
        let color = sample_gradient_color(&self.color_keys, time);
        ParticleColor {
            a: sample_gradient_alpha(&self.alpha_keys, time),
            ..color
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleColorGradient {
    pub mode: i32,
    pub min_color: ParticleColor,
    pub max_color: ParticleColor,
    pub min_gradient: UnityColorGradient,
    pub max_gradient: UnityColorGradient,
}

impl ParticleColorGradient {
    pub fn constant(color: ParticleColor) -> Self {
        Self {
            mode: 0,
            min_color: color,
            max_color: color,
            min_gradient: UnityColorGradient::constant(color),
            max_gradient: UnityColorGradient::constant(color),
        }
    }

    pub fn sample(&self, time: f32, random: f32) -> ParticleColor {
        let random = random.clamp(0.0, 1.0);
        match self.mode {
            2 => self
                .min_gradient
                .sample(time)
                .lerp(self.max_gradient.sample(time), random),
            3 => self.min_color.lerp(self.max_color, random),
            _ => self.max_gradient.sample(time),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleCurve {
    pub mode: i32,
    pub scalar: f32,
    pub min_scalar: f32,
    pub max_curve: Vec<HermiteKey>,
    pub min_curve: Vec<HermiteKey>,
}

impl ParticleCurve {
    pub fn constant(value: f32) -> Self {
        Self {
            mode: 0,
            scalar: value,
            min_scalar: value,
            max_curve: Vec::new(),
            min_curve: Vec::new(),
        }
    }

    pub fn sample(&self, time: f32, random: f32) -> f32 {
        let random = random.clamp(0.0, 1.0);
        match self.mode {
            1 => self.scalar * sample_hermite(&self.max_curve, time, 1.0),
            2 => {
                let min_value = sample_hermite(&self.min_curve, time, 1.0);
                let max_value = sample_hermite(&self.max_curve, time, 1.0);
                self.scalar * lerp(min_value, max_value, random)
            }
            3 => lerp(self.min_scalar, self.scalar, random),
            _ => self.scalar,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleUvModule {
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub row_index: u32,
    pub animation_type: i32,
    pub frame_over_time: ParticleCurve,
}

impl ParticleUvModule {
    pub fn sample_frame_index(&self, random: f32) -> u32 {
        let tiles_x = self.tiles_x.max(1);
        let frame_span = if self.animation_type == 1 {
            tiles_x
        } else {
            tiles_x.saturating_mul(self.tiles_y.max(1))
        };
        if self.frame_over_time.mode == 3 {
            let min_frame =
                (self.frame_over_time.min_scalar.max(0.0) * frame_span as f32).floor();
            let max_frame = (self.frame_over_time.scalar.max(0.0) * frame_span as f32).floor();
            let min_frame = min_frame.min(max_frame) as u32;
            let max_frame = max_frame.max(min_frame as f32) as u32;
            let frame_count = max_frame.saturating_sub(min_frame) + 1;
            let offset = ((frame_count as f32) * random.clamp(0.0, 0.999_999)).floor() as u32;
            (min_frame + offset).min(frame_span - 1)
        } else {
            ((self.frame_over_time.sample(0.0, random).max(0.0) * frame_span as f32).floor()
                as u32)
                .min(frame_span - 1)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParticleBurst {
    pub time: f32,
    pub count: ParticleCurve,
    pub cycle_count: u32,
    pub repeat_interval: f32,
}

impl ParticleBurst {
    pub fn sample_count(&self, random: f32) -> usize {
        self.count.sample(0.0, random).round().max(0.0) as usize
    }
}

#[derive(Debug, Clone)]
pub struct UnityParticleSystemDef {
    pub name: String,
    pub local_position: Vec3,
    pub local_rotation: [f32; 4],
    pub duration: f32,
    pub play_on_awake: bool,
    pub prewarm: bool,
    pub looping: bool,
    pub max_particles: usize,
    pub start_lifetime: ParticleCurve,
    pub start_speed: ParticleCurve,
    pub start_color: ParticleColorGradient,
    pub start_size: ParticleCurve,
    pub start_rotation: ParticleCurve,
    pub emission_rate: ParticleCurve,
    pub color_over_lifetime_enabled: bool,
    pub color_over_lifetime: ParticleColorGradient,
    pub size_over_lifetime: ParticleCurve,
    pub rotation_over_lifetime: ParticleCurve,
    pub velocity_x: ParticleCurve,
    pub velocity_y: ParticleCurve,
    pub velocity_z: ParticleCurve,
    pub velocity_world_space: bool,
    pub force_x: ParticleCurve,
    pub force_y: ParticleCurve,
    pub force_z: ParticleCurve,
    pub force_world_space: bool,
    pub shape_scale: Vec3,
    pub shape_radius: f32,
    pub bursts: Vec<ParticleBurst>,
    pub uv_module: ParticleUvModule,
}

impl UnityParticleSystemDef {
    pub fn projected_right_xy(&self) -> Vec2 {
        let (right, _, _) = quaternion_axes(self.local_rotation);
        normalize_xy(right.0, right.1)
    }

    pub fn projected_up_xy(&self) -> Vec2 {
        let (_, up, _) = quaternion_axes(self.local_rotation);
        normalize_xy(up.0, up.1)
    }

    pub fn projected_forward_xy(&self) -> Vec2 {
        let (_, _, forward) = quaternion_axes(self.local_rotation);
        normalize_xy(forward.0, forward.1)
    }

    pub fn projected_ellipsoid_half_extents_xy(&self) -> Vec2 {
        let (right, up, forward) = quaternion_axes(self.local_rotation);
        let ax = self.shape_scale.x * self.shape_radius;
        let ay = self.shape_scale.y * self.shape_radius;
        let az = self.shape_scale.z * self.shape_radius;
        Vec2 {
            x: ((ax * right.0).powi(2) + (ay * up.0).powi(2) + (az * forward.0).powi(2)).sqrt(),
            y: ((ax * right.1).powi(2) + (ay * up.1).powi(2) + (az * forward.1).powi(2)).sqrt(),
        }
    }

    pub fn projected_ellipsoid_half_extents_xz(&self) -> Vec2 {
        let (right, up, forward) = quaternion_axes(self.local_rotation);
        let ax = self.shape_scale.x * self.shape_radius;
        let ay = self.shape_scale.y * self.shape_radius;
        let az = self.shape_scale.z * self.shape_radius;
        Vec2 {
            x: ((ax * right.0).powi(2) + (ay * up.0).powi(2) + (az * forward.0).powi(2)).sqrt(),
            y: ((ax * right.2).powi(2) + (ay * up.2).powi(2) + (az * forward.2).powi(2)).sqrt(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BirdSleepParticlePrefab {
    pub system: UnityParticleSystemDef,
}

#[derive(Debug, Clone)]
pub struct FanPuffParticlePrefab {
    pub system: UnityParticleSystemDef,
}

#[derive(Debug, Clone)]
pub struct GenericParticlePrefab {
    pub systems: Vec<UnityParticleSystemDef>,
}

#[derive(Debug, Clone)]
pub struct WindAreaParticlePrefab {
    pub wind_direction: Vec2,
    pub power_factor: f32,
    pub systems: Vec<UnityParticleSystemDef>,
}

pub fn bird_sleep_prefab() -> Option<&'static BirdSleepParticlePrefab> {
    BIRD_SLEEP_PREFAB
        .get_or_init(|| load_bird_sleep_prefab(BIRD_SLEEP_PREFAB_ASSET))
        .as_ref()
}

pub fn fan_puff_prefab() -> Option<&'static FanPuffParticlePrefab> {
    FAN_PUFF_PREFAB
        .get_or_init(|| load_fan_puff_prefab(FAN_PREFAB_ASSET))
        .as_ref()
}

pub fn magnet_effect_prefab() -> Option<&'static GenericParticlePrefab> {
    MAGNET_EFFECT_PREFAB
        .get_or_init(|| load_generic_particle_prefab(MAGNET_EFFECT_PREFAB_ASSET))
        .as_ref()
}

pub fn rocket_fire_prefab() -> Option<&'static GenericParticlePrefab> {
    ROCKET_FIRE_PREFAB
        .get_or_init(|| load_generic_particle_prefab(ROCKET_FIRE_PREFAB_ASSET))
        .as_ref()
}

pub fn turbo_charger_prefab() -> Option<&'static GenericParticlePrefab> {
    TURBO_CHARGER_PREFAB
        .get_or_init(|| load_generic_particle_prefab(TURBO_CHARGER_PREFAB_ASSET))
        .as_ref()
}

pub fn wind_area_prefab() -> Option<&'static WindAreaParticlePrefab> {
    WIND_AREA_PREFAB
        .get_or_init(|| load_wind_area_prefab(WIND_AREA_PREFAB_ASSET))
        .as_ref()
}

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

fn load_bird_sleep_prefab(asset_key: &str) -> Option<BirdSleepParticlePrefab> {
    let text = assets::read_asset_text(asset_key)?;
    let parsed = parse_prefab(&text);
    let system = parsed.particle_systems.iter().find_map(|doc| {
        let game_object = parsed.game_objects.get(&doc.game_object_id)?;
        if game_object.name != "Particles_Bird_Sleep" || !game_object.active {
            return None;
        }
        let transform = parsed.transforms.values().find(|transform| {
            transform.game_object_id == doc.game_object_id
        })?;
        Some(build_particle_system(doc, game_object, transform))
    })?;
    Some(BirdSleepParticlePrefab { system })
}

fn load_fan_puff_prefab(asset_key: &str) -> Option<FanPuffParticlePrefab> {
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

fn load_generic_particle_prefab(asset_key: &str) -> Option<GenericParticlePrefab> {
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

fn load_wind_area_prefab(asset_key: &str) -> Option<WindAreaParticlePrefab> {
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
            .unwrap_or_else(|| ParticleColorGradient::constant(ParticleColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            })),
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
            .unwrap_or_else(|| ParticleColorGradient::constant(ParticleColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            })),
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

fn sample_gradient_color(keys: &[(f32, ParticleColor)], time: f32) -> ParticleColor {
    let time = time.clamp(0.0, 1.0);
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for window in keys.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if time <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            return c0.lerp(c1, (time - t0) / span);
        }
    }
    keys[keys.len() - 1].1
}

fn sample_gradient_alpha(keys: &[(f32, f32)], time: f32) -> f32 {
    let time = time.clamp(0.0, 1.0);
    if time <= keys[0].0 {
        return keys[0].1;
    }
    for window in keys.windows(2) {
        let (t0, a0) = window[0];
        let (t1, a1) = window[1];
        if time <= t1 {
            let span = (t1 - t0).max(f32::EPSILON);
            return lerp(a0, a1, (time - t0) / span);
        }
    }
    keys[keys.len() - 1].1
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
    value.as_bool().or_else(|| value_as_i64(value).map(|value| value != 0))
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

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t
}

fn normalize_xy(x: f32, y: f32) -> Vec2 {
    let len = (x * x + y * y).sqrt();
    if len <= f32::EPSILON {
        return Vec2 { x: 0.0, y: 0.0 };
    }
    Vec2 {
        x: x / len,
        y: y / len,
    }
}

fn quaternion_axes(quat: [f32; 4]) -> ((f32, f32, f32), (f32, f32, f32), (f32, f32, f32)) {
    let [x, y, z, w] = quat;
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;

    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);

    ((m00, m10, m20), (m01, m11, m21), (m02, m12, m22))
}

fn sample_hermite(keys: &[(f32, f32, f32, f32)], time: f32, fallback: f32) -> f32 {
    let n = keys.len();
    if n == 0 {
        return fallback;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    if time >= keys[n - 1].0 {
        return keys[n - 1].1;
    }

    let mut index = 0;
    while index < n - 2 && keys[index + 1].0 < time {
        index += 1;
    }

    let (t0, v0, _, out_slope) = keys[index];
    let (t1, v1, in_slope, _) = keys[index + 1];
    let dt = t1 - t0;
    let s = (time - t0) / dt;
    let s2 = s * s;
    let s3 = s2 * s;

    (2.0 * s3 - 3.0 * s2 + 1.0) * v0
        + (s3 - 2.0 * s2 + s) * (out_slope * dt)
        + (-2.0 * s3 + 3.0 * s2) * v1
        + (s3 - s2) * (in_slope * dt)
}

#[cfg(test)]
mod tests {
    use crate::domain::types::Vec3;

    use super::{
        bird_sleep_prefab, fan_puff_prefab, magnet_effect_prefab, rocket_fire_prefab,
        turbo_charger_prefab, wind_area_prefab, ParticleColor,
    };

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_color_close(actual: ParticleColor, expected: ParticleColor) {
        assert_close(actual.r, expected.r);
        assert_close(actual.g, expected.g);
        assert_close(actual.b, expected.b);
        assert_close(actual.a, expected.a);
    }

    #[test]
    fn wind_area_prefab_loads_particle_values() {
        let prefab = wind_area_prefab().expect("wind area prefab should parse");
        let system = prefab.systems.first().expect("wind area particle system");

        assert_eq!(prefab.systems.len(), 3);
        assert_close(prefab.wind_direction.x, 4.8851843);
        assert_close(prefab.wind_direction.y, 0.0);
        assert_close(prefab.power_factor, 1.5);
        assert_close(system.start_lifetime.sample(0.0, 0.0), 4.444445);
        assert_close(system.start_speed.sample(0.0, 0.0), 6.0);
        assert_close(system.start_speed.sample(0.0, 1.0), 9.0);
        assert_close(system.start_size.sample(0.0, 0.0), 0.4);
        assert_close(system.start_size.sample(0.0, 1.0), 0.7);
        assert!(!system.color_over_lifetime_enabled);
        assert_color_close(
            system.start_color.sample(0.0, 0.0),
            ParticleColor {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        );
        assert_eq!(system.max_particles, 20);
        assert_close(system.emission_rate.sample(0.0, 0.0), 1.0);
        assert_eq!(system.uv_module.tiles_x, 16);
        assert_eq!(system.uv_module.tiles_y, 16);
        assert_eq!(system.uv_module.row_index, 2);
        assert_eq!(system.uv_module.sample_frame_index(0.0), 4);
        assert_eq!(system.uv_module.sample_frame_index(0.999), 6);

        let dir = system.projected_forward_xy();
        assert_close(dir.x, 0.61064816);
        assert_close(dir.y, -0.79190195);
        let side = system.projected_up_xy();
        assert_close(side.x, 0.79190195);
        assert_close(side.y, 0.6106482);
    }

    #[test]
    fn bird_sleep_prefab_loads_particle_values() {
        let prefab = bird_sleep_prefab().expect("bird sleep prefab should parse");
        let system = &prefab.system;

        assert!(!system.play_on_awake);
        assert!(!system.prewarm);
        assert_close(system.local_position.y, 0.5);
        assert_close(system.start_lifetime.sample(0.0, 0.0), 1.0);
        assert_close(system.start_lifetime.sample(0.0, 1.0), 2.0);
        assert_close(system.start_size.sample(0.0, 0.0), 0.7);
        assert_close(system.emission_rate.sample(0.0, 0.0), 2.0);
        assert!(!system.color_over_lifetime_enabled);
        assert_eq!(system.max_particles, 5);
        assert_eq!(system.uv_module.tiles_x, 8);
        assert_eq!(system.uv_module.tiles_y, 8);
        assert_eq!(system.uv_module.row_index, 2);
        assert_eq!(system.uv_module.sample_frame_index(0.0), 6);
        assert_close(system.size_over_lifetime.sample(0.355244, 0.0), 0.7);
        assert_close(system.rotation_over_lifetime.sample(0.0, 0.0), 0.0);
        assert_close(system.rotation_over_lifetime.sample(0.0, 1.0), 0.5235988);
        assert_close(system.velocity_y.sample(0.0, 0.0), 0.49414596);
        assert_close(system.velocity_y.sample(0.0, 1.0), 0.3103452);

        let extents = system.projected_ellipsoid_half_extents_xy();
        assert_close(extents.x, 0.60200006);
        assert_close(extents.y, 0.518);
    }

    #[test]
    fn fan_puff_prefab_loads_particle_values() {
        let prefab = fan_puff_prefab().expect("fan puff prefab should parse");
        let system = &prefab.system;

        assert_eq!(system.name, "Particles");
        assert_close(system.duration, 1.0);
        assert!(!system.play_on_awake);
        assert!(system.looping);
        assert_close(system.local_position.y, 0.6365275);
        assert_eq!(system.max_particles, 40);
        assert_close(system.start_lifetime.sample(0.0, 0.0), 0.7);
        assert_close(system.start_lifetime.sample(0.0, 1.0), 1.5);
        assert_close(system.start_size.sample(0.0, 0.0), 1.2);
        assert!(!system.color_over_lifetime_enabled);
        assert_close(system.velocity_x.sample(0.0, 0.0), 0.1);
        assert_close(system.velocity_x.sample(0.0, 1.0), -0.1);
        assert_close(system.velocity_y.sample(0.0, 0.0), 10.0);
        assert_close(system.velocity_y.sample(0.0, 1.0), 3.0);
        assert_close(system.force_y.sample(0.0, 0.0), -0.5);
        assert_close(system.uv_module.sample_frame_index(0.0) as f32, 3.0);
        assert_eq!(system.bursts.len(), 4);
        assert_close(system.bursts[0].time, 0.0);
        assert_close(system.bursts[1].time, 0.25);
        assert_close(system.bursts[2].time, 0.5);
        assert_close(system.bursts[3].time, 0.75);
        assert_eq!(system.bursts[0].sample_count(0.0), 1);
        assert_eq!(system.bursts[0].cycle_count, 1);
        assert_close(system.bursts[0].repeat_interval, 0.01);
        assert!(system.size_over_lifetime.sample(0.913, 0.5) > 0.12);
        assert!(system.rotation_over_lifetime.sample(0.0, 0.0) > 2.7);
        assert!(system.rotation_over_lifetime.sample(1.0, 0.0) < 0.5);
    }

    #[test]
    fn rocket_fire_prefab_loads_particle_values() {
        let prefab = rocket_fire_prefab().expect("rocket fire prefab should parse");
        let system = prefab.systems.first().expect("rocket fire system");

        assert_eq!(prefab.systems.len(), 1);
        assert_eq!(system.name, "Particles_RocketFire_01_SET");
        assert_close(system.duration, 0.5);
        assert!(system.play_on_awake);
        assert!(system.prewarm);
        assert_close(system.start_lifetime.sample(0.0, 0.0), 0.5);
        assert_close(system.start_speed.sample(0.0, 0.0), 0.5);
        assert_close(system.start_speed.sample(0.0, 1.0), 2.0);
        assert_close(system.start_size.sample(0.0, 0.0), 0.2);
        assert_close(system.start_size.sample(0.0, 1.0), 0.4);
        assert!(!system.color_over_lifetime_enabled);
        assert_close(system.local_position.x, Vec3::default().x);
        assert_close(system.local_position.y, Vec3::default().y);
        assert_close(system.local_position.z, Vec3::default().z);
        assert_eq!(system.local_rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(system.uv_module.tiles_x, 1);
        assert_eq!(system.uv_module.tiles_y, 1);
    }

    #[test]
    fn turbo_charger_prefab_parses_enabled_color_gradient() {
        let prefab = turbo_charger_prefab().expect("turbo charger prefab should parse");
        let system = prefab.systems.first().expect("turbo charger system");

        assert_eq!(prefab.systems.len(), 1);
        assert_eq!(system.name, "TurboChargerEffect");
        assert!(system.color_over_lifetime_enabled);
        assert_eq!(system.uv_module.tiles_x, 8);
        assert_eq!(system.uv_module.tiles_y, 8);
        assert_eq!(system.uv_module.row_index, 2);

        let start = system.color_over_lifetime.sample(0.0, 0.0);
        let end = system.color_over_lifetime.sample(1.0, 0.0);
        assert!(start.r > 0.68 && start.g > 0.68 && start.b > 0.68);
        assert!(end.r < 0.01 && end.g < 0.01 && end.b < 0.01);
        assert_close(end.a, 1.0);

        let start_color = system.start_color.sample(0.0, 0.0);
        let end_color = system.start_color.sample(1.0, 0.0);
        assert!(start_color.r < 0.01 && start_color.g < 0.01 && start_color.b < 0.01);
        assert!(end_color.r < 0.01 && end_color.g < 0.01 && end_color.b < 0.01);
    }

    #[test]
    fn magnet_effect_prefab_loads_multiple_particle_systems() {
        let prefab = magnet_effect_prefab().expect("magnet effect prefab should parse");

        assert_eq!(prefab.systems.len(), 3);
        assert_eq!(prefab.systems[0].name, "MagnetEffect");
        assert_eq!(prefab.systems[1].name, "MagnetBolts");
        assert_eq!(prefab.systems[2].name, "Particle System");
        assert_eq!(prefab.systems[0].bursts.len(), 2);
        assert_eq!(prefab.systems[0].uv_module.tiles_x, 16);
        assert_eq!(prefab.systems[0].uv_module.tiles_y, 8);

        let color_mid = prefab.systems[0].color_over_lifetime.sample(0.5, 0.0);
        assert!(color_mid.g > 0.4);
        assert!(color_mid.b > 0.7);
    }
}