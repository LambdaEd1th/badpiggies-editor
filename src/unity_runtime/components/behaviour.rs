//! `UnityEngine.Behaviour` — placeholder MonoBehaviour root.
//!
//! Concrete Bad-Piggies MonoBehaviours (WindArea, Bridge, Fan, Engine, …)
//! ship as their own component types in P1+. This one exists for tests that
//! exercise the `m_Enabled` rename quirk.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct Behaviour {
    pub enabled: bool,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for Behaviour {
    fn component_suffix(&self) -> &str {
        "Behaviour"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        if name == "m_Enabled" || name == "enabled" {
            Some(SceneValue::Boolean(self.enabled))
        } else {
            None
        }
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_Enabled" | "enabled", SceneValue::Boolean(v)) => {
                self.enabled = v;
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
