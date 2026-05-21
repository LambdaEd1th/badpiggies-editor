//! `UnityEngine.ParticleSystem` — subset that
//! [`ObjectDeserializer`](crate::domain::object_deserializer) exposes.
//!
//! The deserializer drives `set_particle_start_lifetime` /
//! `set_particle_start_speed` hooks (see
//! [`RuntimeHost`](crate::domain::object_deserializer::RuntimeHost)) which
//! the [`Scene`] adapter forwards onto the typed component. Any other module
//! field (`EmissionModule rate`, `ShapeModule`, …) lands in [`Self::extra`]
//! via the generic `set_field` spill path.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct ParticleSystem {
    pub start_lifetime: Option<f32>,
    pub start_speed: Option<f32>,
    pub emission_rate: Option<f32>,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for ParticleSystem {
    fn component_suffix(&self) -> &str {
        "ParticleSystem"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "startLifetime" => SceneValue::Float(self.start_lifetime?),
            "startSpeed" => SceneValue::Float(self.start_speed?),
            "emissionRate" => SceneValue::Float(self.emission_rate?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        // The deserializer routes module values through dedicated hook
        // methods on `RuntimeHost`, not `set_field`, so these arms only fire
        // when something else hand-writes the typed field.
        match (name, value) {
            ("startLifetime", SceneValue::Float(v)) => {
                self.start_lifetime = Some(v);
                true
            }
            ("startSpeed", SceneValue::Float(v)) => {
                self.start_speed = Some(v);
                true
            }
            ("emissionRate", SceneValue::Float(v)) => {
                self.emission_rate = Some(v);
                true
            }
            _ => false,
        }
    }

    fn extra_mut(&mut self) -> &mut Vec<(String, SceneValue)> {
        &mut self.extra
    }

    fn extra(&self) -> &[(String, SceneValue)] {
        &self.extra
    }

    unity_component_boilerplate!();
}
