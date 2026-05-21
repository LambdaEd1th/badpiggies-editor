//! `UnityEngine.BoxCollider` / `BoxCollider2D` (subset).

use crate::domain::types::Vec3;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct BoxCollider {
    pub center: Option<Vec3>,
    pub size: Option<Vec3>,
    pub enabled: Option<bool>,
    pub is_trigger: Option<bool>,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for BoxCollider {
    fn component_suffix(&self) -> &str {
        "BoxCollider"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_Center" => SceneValue::Vector3(self.center?),
            "m_Size" => SceneValue::Vector3(self.size?),
            "m_Enabled" | "enabled" => SceneValue::Boolean(self.enabled?),
            "m_IsTrigger" | "isTrigger" => SceneValue::Boolean(self.is_trigger?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_Center", SceneValue::Vector3(v)) => self.center = Some(v),
            ("m_Size", SceneValue::Vector3(v)) => self.size = Some(v),
            ("m_Enabled" | "enabled", SceneValue::Boolean(v)) => self.enabled = Some(v),
            ("m_IsTrigger" | "isTrigger", SceneValue::Boolean(v)) => self.is_trigger = Some(v),
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
