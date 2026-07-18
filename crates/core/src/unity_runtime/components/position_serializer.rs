//! `PositionSerializer` MonoBehaviour. C# source:
//! `Assets/Scripts/Assembly-CSharp/PositionSerializer.cs`.
//!
//! The `childLocalPositions: Vec<Vector3>` array is consumed by the
//! background theme groups pipeline; it spills to [`Self::extra`] under the
//! `childLocalPositions` key as `SceneValue::Generic` (see
//! `Scene::set_array`).

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{GameObjectId, Scene, SceneValue};

#[derive(Debug, Clone, Default)]
pub struct PositionSerializer {
    /// `ObjectReference prefab = N` from the override block (the int is an
    /// index into the level's reference table).
    pub prefab: Option<GameObjectId>,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for PositionSerializer {
    fn component_suffix(&self) -> &str {
        "PositionSerializer"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        if name == "prefab" {
            self.prefab.map(SceneValue::ObjectReference)
        } else {
            None
        }
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("prefab", SceneValue::ObjectReference(go)) => {
                self.prefab = Some(go);
                true
            }
            ("prefab", SceneValue::Null) => {
                self.prefab = None;
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
