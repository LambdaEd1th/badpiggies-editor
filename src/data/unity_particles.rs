//! Unity prefab particle system data and parsers.

mod math;
mod parse;
mod prefabs;
mod types;

pub use prefabs::{
    WindAreaParticlePrefab, bird_sleep_prefab, fan_puff_prefab, fly_swarm_prefab,
    magnet_effect_prefab, rocket_fire_prefab, turbo_charger_prefab, wind_area_prefab,
};
pub use types::{ParticleColor, ParticleCurve, UnityParticleSystemDef};

#[cfg(test)]
mod tests;
