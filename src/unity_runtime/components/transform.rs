//! `UnityEngine.Transform` (subset).

use crate::domain::object_deserializer::Quaternion;
use crate::domain::types::Vec3;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct Transform {
    pub local_position: Option<Vec3>,
    pub local_rotation: Option<Quaternion>,
    pub local_scale: Option<Vec3>,
    pub local_euler_angles: Option<Vec3>,
    /// Unrecognized field writes, preserved for lossless re-serialization.
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for Transform {
    fn component_suffix(&self) -> &str {
        "Transform"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_LocalPosition" => SceneValue::Vector3(self.local_position?),
            "m_LocalRotation" => SceneValue::Quaternion(self.local_rotation?),
            "m_LocalScale" => SceneValue::Vector3(self.local_scale?),
            "m_LocalEulerAnglesHint" => SceneValue::Vector3(self.local_euler_angles?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_LocalPosition", SceneValue::Vector3(v)) => self.local_position = Some(v),
            ("m_LocalRotation", SceneValue::Quaternion(q)) => self.local_rotation = Some(q),
            ("m_LocalScale", SceneValue::Vector3(v)) => self.local_scale = Some(v),
            ("m_LocalEulerAnglesHint", SceneValue::Vector3(v)) => self.local_euler_angles = Some(v),
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
