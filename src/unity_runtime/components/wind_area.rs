//! `WindArea` MonoBehaviour.
//!
//! C# source: `Assets/Scripts/Assembly-CSharp/WindArea.cs`.
//! Fields read by the renderer pipeline (see `renderer/particles/wind.rs`):
//! `windDirectionHandle: Vec3`, `m_windPowerFactor: f32`.

use crate::domain::types::Vec3;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct WindArea {
    /// World-space target point the wind direction vector points toward.
    /// `None` when the override text did not contain this field — consumers
    /// must fall back to the prefab default.
    pub wind_direction_handle: Option<Vec3>,
    pub wind_power_factor: Option<f32>,
    pub calculate_particle_values: Option<bool>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl WindArea {
    pub const DEFAULT_WIND_DIRECTION_HANDLE: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const DEFAULT_WIND_POWER_FACTOR: f32 = 1.0;
    pub const DEFAULT_CALCULATE_PARTICLE_VALUES: bool = true;
}

impl UnityComponent for WindArea {
    fn component_suffix(&self) -> &str {
        "WindArea"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "windDirectionHandle" => SceneValue::Vector3(self.wind_direction_handle?),
            "m_windPowerFactor" => SceneValue::Float(self.wind_power_factor?),
            "m_calculateParticleValues" => SceneValue::Boolean(self.calculate_particle_values?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("windDirectionHandle", SceneValue::Vector3(v)) => {
                self.wind_direction_handle = Some(v)
            }
            ("m_windPowerFactor", SceneValue::Float(v)) => self.wind_power_factor = Some(v),
            ("m_calculateParticleValues", SceneValue::Boolean(v)) => {
                self.calculate_particle_values = Some(v)
            }
            _ => return false,
        }
        true
    }

    fn extra_mut(&mut self) -> &mut Vec<(String, SceneValue)> {
        &mut self.extra
    }

    fn extra(&self) -> &[(String, SceneValue)] {
        &self.extra
    }

    unity_component_boilerplate!();
}
