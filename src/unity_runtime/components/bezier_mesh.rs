//! `MentalTools.BezierMesh`. C# source:
//! `Assets/Scripts/Assembly-CSharp/MentalTools/BezierMesh.cs`.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct BezierMesh {
    pub border_width: Option<f32>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl BezierMesh {
    pub const DEFAULT_BORDER_WIDTH: f32 = 0.0;
}

impl UnityComponent for BezierMesh {
    fn component_suffix(&self) -> &str {
        "BezierMesh"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        if name == "borderWidth" {
            Some(SceneValue::Float(self.border_width?))
        } else {
            None
        }
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("borderWidth", SceneValue::Float(v)) => {
                self.border_width = Some(v);
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
