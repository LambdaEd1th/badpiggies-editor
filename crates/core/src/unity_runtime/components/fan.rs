//! `Fan` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/Fan.cs`.
//!
//! Renderer pipeline consumer: `renderer/compound_overrides.rs`.
//! AnimationCurve fields (`verticalRamp`, `horizontalRamp`, `spinupRamp`)
//! land in [`Self::extra`] via the host's `set_animation_curve` adapter.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct Fan {
    pub target_force: Option<f32>,
    pub always_on: Option<bool>,
    pub delayed_start: Option<f32>,
    pub start_time: Option<f32>,
    pub off_time: Option<f32>,
    pub on_time: Option<f32>,
    pub hearing_distance: Option<f32>,
    pub counter: Option<f32>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl Fan {
    pub const DEFAULT_HEARING_DISTANCE: f32 = 1000.0;
}

impl UnityComponent for Fan {
    fn component_suffix(&self) -> &str {
        "Fan"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "targetForce" => SceneValue::Float(self.target_force?),
            "alwaysOn" => SceneValue::Boolean(self.always_on?),
            "delayedStart" => SceneValue::Float(self.delayed_start?),
            "startTime" => SceneValue::Float(self.start_time?),
            "offTime" => SceneValue::Float(self.off_time?),
            "onTime" => SceneValue::Float(self.on_time?),
            "hearingDistance" => SceneValue::Float(self.hearing_distance?),
            "counter" => SceneValue::Float(self.counter?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("targetForce", SceneValue::Float(v)) => self.target_force = Some(v),
            ("alwaysOn", SceneValue::Boolean(v)) => self.always_on = Some(v),
            ("delayedStart", SceneValue::Float(v)) => self.delayed_start = Some(v),
            ("startTime", SceneValue::Float(v)) => self.start_time = Some(v),
            ("offTime", SceneValue::Float(v)) => self.off_time = Some(v),
            ("onTime", SceneValue::Float(v)) => self.on_time = Some(v),
            ("hearingDistance", SceneValue::Float(v)) => self.hearing_distance = Some(v),
            ("counter", SceneValue::Float(v)) => self.counter = Some(v),
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
