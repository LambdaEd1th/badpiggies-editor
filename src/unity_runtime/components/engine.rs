//! `Engine` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/Engine.cs`.
//!
//! Most properties are not serialized into the override block. The
//! `smokeEmitter` / `flameEmitter` ParticleSystem references are resolved at
//! runtime; the only persisted scalar in our data set is `m_running`.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct Engine {
    pub running: bool,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for Engine {
    fn component_suffix(&self) -> &str {
        "Engine"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        if name == "m_running" {
            Some(SceneValue::Boolean(self.running))
        } else {
            None
        }
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_running", SceneValue::Boolean(v)) => {
                self.running = v;
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
