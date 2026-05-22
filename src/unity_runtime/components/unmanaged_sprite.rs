//! `UnmanagedSprite` MonoBehaviour. C# source:
//! `Assets/Scripts/Assembly-CSharp/UnmanagedSprite.cs`.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone)]
pub struct UnmanagedSprite {
    pub texture_width: i32,
    pub texture_height: i32,
    pub scale: f32,
    pub sprite_width: i32,
    pub sprite_height: i32,
    pub uv_x: i32,
    pub uv_y: i32,
    pub width: i32,
    pub height: i32,
    pub atlas_grid_subdivisions: i32,
    pub border: i32,
    pub update_collider: bool,
    pub extra: Vec<(String, SceneValue)>,
}

impl Default for UnmanagedSprite {
    fn default() -> Self {
        Self {
            texture_width: 0,
            texture_height: 0,
            scale: 1.0,
            sprite_width: 0,
            sprite_height: 0,
            uv_x: 0,
            uv_y: 0,
            width: 16,
            height: 16,
            atlas_grid_subdivisions: 16,
            border: 0,
            update_collider: false,
            extra: Vec::new(),
        }
    }
}

impl UnityComponent for UnmanagedSprite {
    fn component_suffix(&self) -> &str {
        "UnmanagedSprite"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_textureWidth" => SceneValue::Integer(self.texture_width),
            "m_textureHeight" => SceneValue::Integer(self.texture_height),
            "m_scale" => SceneValue::Float(self.scale),
            "m_spriteWidth" => SceneValue::Integer(self.sprite_width),
            "m_spriteHeight" => SceneValue::Integer(self.sprite_height),
            "m_UVx" => SceneValue::Integer(self.uv_x),
            "m_UVy" => SceneValue::Integer(self.uv_y),
            "m_width" => SceneValue::Integer(self.width),
            "m_height" => SceneValue::Integer(self.height),
            "m_atlasGridSubdivisions" => SceneValue::Integer(self.atlas_grid_subdivisions),
            "m_border" => SceneValue::Integer(self.border),
            "m_updateCollider" => SceneValue::Boolean(self.update_collider),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_textureWidth", SceneValue::Integer(v)) => self.texture_width = v,
            ("m_textureHeight", SceneValue::Integer(v)) => self.texture_height = v,
            ("m_scale", SceneValue::Float(v)) => self.scale = v,
            ("m_spriteWidth", SceneValue::Integer(v)) => self.sprite_width = v,
            ("m_spriteHeight", SceneValue::Integer(v)) => self.sprite_height = v,
            ("m_UVx", SceneValue::Integer(v)) => self.uv_x = v,
            ("m_UVy", SceneValue::Integer(v)) => self.uv_y = v,
            ("m_width", SceneValue::Integer(v)) => self.width = v,
            ("m_height", SceneValue::Integer(v)) => self.height = v,
            ("m_atlasGridSubdivisions", SceneValue::Integer(v)) => self.atlas_grid_subdivisions = v,
            ("m_border", SceneValue::Integer(v)) => self.border = v,
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
