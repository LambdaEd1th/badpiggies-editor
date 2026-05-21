//! `UnityEngine.Rigidbody` (subset).
//!
//! `ObjectDeserializer.SetProperty` applies a rename + cast quirk: the
//! source emits `isKinematic` as an Integer (0/1), so the walker has
//! already coerced it to a Boolean by the time we see it here.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone)]
pub struct Rigidbody {
    pub mass: f32,
    pub drag: f32,
    pub angular_drag: f32,
    pub use_gravity: bool,
    pub is_kinematic: bool,
    pub extra: Vec<(String, SceneValue)>,
}

impl Default for Rigidbody {
    fn default() -> Self {
        Self {
            mass: 1.0,
            drag: 0.0,
            angular_drag: 0.05,
            use_gravity: true,
            is_kinematic: false,
            extra: Vec::new(),
        }
    }
}

impl UnityComponent for Rigidbody {
    fn component_suffix(&self) -> &str {
        "Rigidbody"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_Mass" | "mass" => SceneValue::Float(self.mass),
            "m_Drag" | "drag" => SceneValue::Float(self.drag),
            "m_AngularDrag" | "angularDrag" => SceneValue::Float(self.angular_drag),
            "m_UseGravity" | "useGravity" => SceneValue::Boolean(self.use_gravity),
            "m_IsKinematic" | "isKinematic" => SceneValue::Boolean(self.is_kinematic),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_Mass" | "mass", SceneValue::Float(v)) => self.mass = v,
            ("m_Drag" | "drag", SceneValue::Float(v)) => self.drag = v,
            ("m_AngularDrag" | "angularDrag", SceneValue::Float(v)) => self.angular_drag = v,
            ("m_UseGravity" | "useGravity", SceneValue::Boolean(v)) => self.use_gravity = v,
            ("m_IsKinematic" | "isKinematic", SceneValue::Boolean(v)) => self.is_kinematic = v,
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
