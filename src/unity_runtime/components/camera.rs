//! `UnityEngine.Camera` (subset of serializable fields).

use crate::domain::types::Color;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone)]
pub struct Camera {
    pub background_color: Color,
    pub orthographic: bool,
    pub orthographic_size: f32,
    pub near_clip_plane: f32,
    pub far_clip_plane: f32,
    pub field_of_view: f32,
    pub depth: f32,
    pub culling_mask: i32,
    pub clear_flags: i32,
    pub extra: Vec<(String, SceneValue)>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            background_color: Color::default(),
            orthographic: false,
            orthographic_size: 5.0,
            near_clip_plane: 0.3,
            far_clip_plane: 1000.0,
            field_of_view: 60.0,
            depth: 0.0,
            culling_mask: -1,
            clear_flags: 1,
            extra: Vec::new(),
        }
    }
}

impl UnityComponent for Camera {
    fn component_suffix(&self) -> &str {
        "Camera"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "m_BackGroundColor" => SceneValue::Color(self.background_color),
            "orthographic" | "m_Orthographic" => SceneValue::Boolean(self.orthographic),
            "orthographic size" | "orthographic_size" | "orthographicSize"
            | "m_OrthographicSize" => SceneValue::Float(self.orthographic_size),
            "near clip plane" | "nearClipPlane" => SceneValue::Float(self.near_clip_plane),
            "far clip plane" | "farClipPlane" => SceneValue::Float(self.far_clip_plane),
            "field of view" | "fieldOfView" => SceneValue::Float(self.field_of_view),
            "m_Depth" | "depth" => SceneValue::Float(self.depth),
            "m_CullingMask" => SceneValue::Integer(self.culling_mask),
            "m_ClearFlags" => SceneValue::Integer(self.clear_flags),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_BackGroundColor", SceneValue::Color(v)) => self.background_color = v,
            ("orthographic" | "m_Orthographic", SceneValue::Boolean(v)) => self.orthographic = v,
            (
                "orthographic size" | "orthographic_size" | "orthographicSize"
                | "m_OrthographicSize",
                SceneValue::Float(v),
            ) => self.orthographic_size = v,
            ("near clip plane" | "nearClipPlane", SceneValue::Float(v)) => self.near_clip_plane = v,
            ("far clip plane" | "farClipPlane", SceneValue::Float(v)) => self.far_clip_plane = v,
            ("field of view" | "fieldOfView", SceneValue::Float(v)) => self.field_of_view = v,
            ("m_Depth" | "depth", SceneValue::Float(v)) => self.depth = v,
            ("m_CullingMask", SceneValue::Integer(v)) => self.culling_mask = v,
            ("m_ClearFlags", SceneValue::Integer(v)) => self.clear_flags = v,
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
