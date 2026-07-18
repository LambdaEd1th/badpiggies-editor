//! `Sprite` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/Sprite.cs`.
//!
//! Not to be confused with `UnityEngine.Sprite`; this is the Bad-Piggies
//! atlas-backed sprite renderer.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone)]
pub struct Sprite {
    pub id: String,
    pub scale_x: f32,
    pub scale_y: f32,
    pub pivot_x: i32,
    pub pivot_y: i32,
    pub update_collider: bool,
    pub extra: Vec<(String, SceneValue)>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            id: String::new(),
            scale_x: 1.0,
            scale_y: 1.0,
            pivot_x: 0,
            pivot_y: 0,
            update_collider: true,
            extra: Vec::new(),
        }
    }
}

impl UnityComponent for Sprite {
    fn component_suffix(&self) -> &str {
        "Sprite"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_id" => SceneValue::String(self.id.clone()),
            "m_scaleX" => SceneValue::Float(self.scale_x),
            "m_scaleY" => SceneValue::Float(self.scale_y),
            "m_pivotX" => SceneValue::Integer(self.pivot_x),
            "m_pivotY" => SceneValue::Integer(self.pivot_y),
            "m_updateCollider" => SceneValue::Boolean(self.update_collider),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_id", SceneValue::String(v)) => self.id = v,
            ("m_scaleX", SceneValue::Float(v)) => self.scale_x = v,
            ("m_scaleY", SceneValue::Float(v)) => self.scale_y = v,
            ("m_pivotX", SceneValue::Integer(v)) => self.pivot_x = v,
            ("m_pivotY", SceneValue::Integer(v)) => self.pivot_y = v,
            ("m_updateCollider", SceneValue::Boolean(v)) => self.update_collider = v,
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
