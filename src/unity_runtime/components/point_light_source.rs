//! `PointLightSource` MonoBehaviour. C# source:
//! `Assets/Scripts/Assembly-CSharp/PointLightSource.cs`.
//!
//! Consumer pipeline: `renderer/dark_overlay/parse.rs`.

use crate::domain::types::Vec3;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct PointLightSource {
    pub size: Option<f32>,
    pub border_width: Option<f32>,
    pub beam_angle: Option<f32>,
    pub beam_cut: Option<f32>,
    pub vertex_count: Option<i32>,
    pub collider_size: Option<f32>,
    pub beam_arc_center: Option<Vec3>,
    pub base_light_size: Option<f32>,
    pub can_lit_objects: Option<bool>,
    pub can_collide: Option<bool>,
    pub can_be_lit: Option<bool>,
    pub uses_curves: Option<bool>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl PointLightSource {
    pub const DEFAULT_SIZE: f32 = 1.0;
    pub const DEFAULT_BORDER_WIDTH: f32 = 0.3;
    pub const DEFAULT_BEAM_ANGLE: f32 = 45.0;
    pub const DEFAULT_BEAM_CUT: f32 = 0.5;
    pub const DEFAULT_VERTEX_COUNT: i32 = 100;
    pub const DEFAULT_COLLIDER_SIZE: f32 = 0.0;
    pub const DEFAULT_BASE_LIGHT_SIZE: f32 = 0.5;
    pub const DEFAULT_USES_CURVES: bool = true;
}

impl UnityComponent for PointLightSource {
    fn component_suffix(&self) -> &str {
        "PointLightSource"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "size" => SceneValue::Float(self.size?),
            "borderWidth" => SceneValue::Float(self.border_width?),
            "beamAngle" => SceneValue::Float(self.beam_angle?),
            "beamCut" => SceneValue::Float(self.beam_cut?),
            "vertexCount" => SceneValue::Integer(self.vertex_count?),
            "colliderSize" => SceneValue::Float(self.collider_size?),
            "beamArcCenter" => SceneValue::Vector3(self.beam_arc_center?),
            "baseLightSize" => SceneValue::Float(self.base_light_size?),
            "canLitObjects" => SceneValue::Boolean(self.can_lit_objects?),
            "canCollide" => SceneValue::Boolean(self.can_collide?),
            "canBeLit" => SceneValue::Boolean(self.can_be_lit?),
            "usesCurves" => SceneValue::Boolean(self.uses_curves?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("size", SceneValue::Float(v)) => self.size = Some(v),
            ("borderWidth", SceneValue::Float(v)) => self.border_width = Some(v),
            ("beamAngle", SceneValue::Float(v)) => self.beam_angle = Some(v),
            ("beamCut", SceneValue::Float(v)) => self.beam_cut = Some(v),
            ("vertexCount", SceneValue::Integer(v)) => self.vertex_count = Some(v),
            ("colliderSize", SceneValue::Float(v)) => self.collider_size = Some(v),
            ("beamArcCenter", SceneValue::Vector3(v)) => self.beam_arc_center = Some(v),
            ("baseLightSize", SceneValue::Float(v)) => self.base_light_size = Some(v),
            ("canLitObjects", SceneValue::Boolean(v)) => self.can_lit_objects = Some(v),
            ("canCollide", SceneValue::Boolean(v)) => self.can_collide = Some(v),
            ("canBeLit", SceneValue::Boolean(v)) => self.can_be_lit = Some(v),
            ("usesCurves", SceneValue::Boolean(v)) => self.uses_curves = Some(v),
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
