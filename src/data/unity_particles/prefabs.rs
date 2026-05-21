//! Unity prefab structs and the public OnceLock-cached accessors.

use std::sync::OnceLock;

use crate::domain::types::Vec2;

use super::parse::{
    load_bird_sleep_prefab, load_fan_puff_prefab, load_generic_particle_prefab,
    load_rocket_fire_prefab, load_wind_area_prefab,
};
use super::types::UnityParticleSystemDef;

const BIRD_SLEEP_PREFAB_ASSET: &str = "Assets/Prefab/Bird_Red.prefab";
const FAN_PREFAB_ASSET: &str = "Assets/Prefab/Fan.prefab";
const FLY_SWARM_PREFAB_ASSET: &str = "Assets/Prefab/FlySwarm.prefab";
const MAGNET_EFFECT_PREFAB_ASSET: &str = "Assets/Prefab/MagnetEffect.prefab";
const ROCKET_FIRE_PREFAB_ASSET: &str = "Assets/Prefab/Particles_RocketFire_01_SET.prefab";
const ROCKET_PART_PREFAB_ASSET: &str = "Assets/Prefab/Part_Rocket_01_SET.prefab";
const TURBO_CHARGER_PREFAB_ASSET: &str = "Assets/Prefab/TurboChargerEffect.prefab";
const WIND_AREA_PREFAB_ASSET: &str = "Assets/Prefab/WindArea.prefab";

static BIRD_SLEEP_PREFAB: OnceLock<Option<BirdSleepParticlePrefab>> = OnceLock::new();
static FAN_PUFF_PREFAB: OnceLock<Option<FanPuffParticlePrefab>> = OnceLock::new();
static FLY_SWARM_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static MAGNET_EFFECT_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static ROCKET_FIRE_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static TURBO_CHARGER_PREFAB: OnceLock<Option<GenericParticlePrefab>> = OnceLock::new();
static WIND_AREA_PREFAB: OnceLock<Option<WindAreaParticlePrefab>> = OnceLock::new();

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
    #[cfg_attr(not(test), allow(dead_code))]
    pub wind_direction: Vec2,
    #[cfg_attr(not(test), allow(dead_code))]
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

pub fn fly_swarm_prefab() -> Option<&'static GenericParticlePrefab> {
    FLY_SWARM_PREFAB
        .get_or_init(|| load_generic_particle_prefab(FLY_SWARM_PREFAB_ASSET))
        .as_ref()
}

pub fn magnet_effect_prefab() -> Option<&'static GenericParticlePrefab> {
    MAGNET_EFFECT_PREFAB
        .get_or_init(|| load_generic_particle_prefab(MAGNET_EFFECT_PREFAB_ASSET))
        .as_ref()
}

pub fn rocket_fire_prefab() -> Option<&'static GenericParticlePrefab> {
    ROCKET_FIRE_PREFAB
        .get_or_init(|| load_rocket_fire_prefab(ROCKET_FIRE_PREFAB_ASSET, ROCKET_PART_PREFAB_ASSET))
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
