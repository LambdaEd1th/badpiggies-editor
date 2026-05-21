//! `Bridge` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/Bridge.cs`.
//!
//! Renderer pipeline consumer: `renderer/compound_overrides.rs`.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct Bridge {
    pub step_length: Option<f32>,
    pub step_gap: Option<f32>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl Bridge {
    pub const DEFAULT_STEP_LENGTH: f32 = 1.0;
    pub const DEFAULT_STEP_GAP: f32 = 0.2;
}

impl UnityComponent for Bridge {
    fn component_suffix(&self) -> &str {
        "Bridge"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "stepLength" => SceneValue::Float(self.step_length?),
            "stepGap" => SceneValue::Float(self.step_gap?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("stepLength", SceneValue::Float(v)) => self.step_length = Some(v),
            ("stepGap", SceneValue::Float(v)) => self.step_gap = Some(v),
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
