//! `MentalTools.BezierCurve`. C# source:
//! `Assets/Scripts/Assembly-CSharp/MentalTools/BezierCurve.cs`.

use crate::domain::types::Vec3;
use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone, Copy)]
pub struct BezierNode {
    pub position: Vec3,
    pub tangent0: Vec3,
    pub tangent1: Vec3,
}

#[derive(Debug, Clone, Default)]
pub struct BezierCurve {
    pub bezier_point_count: Option<i32>,
    pub looped: Option<bool>,
    pub nodes: Option<Vec<BezierNode>>,
    pub extra: Vec<(String, SceneValue)>,
}

#[allow(dead_code)]
impl BezierCurve {
    pub const DEFAULT_BEZIER_POINT_COUNT: i32 = 10;
    pub const DEFAULT_LOOPED: bool = false;
}

impl UnityComponent for BezierCurve {
    fn component_suffix(&self) -> &str {
        "BezierCurve"
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        Some(match name {
            "bezierPointCount" => SceneValue::Integer(self.bezier_point_count?),
            "loop" => SceneValue::Boolean(self.looped?),
            _ => return None,
        })
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, &value) {
            ("bezierPointCount", SceneValue::Integer(v)) => {
                self.bezier_point_count = Some(*v);
                true
            }
            ("loop", SceneValue::Boolean(v)) => {
                self.looped = Some(*v);
                true
            }
            ("bezierCurve", SceneValue::Generic(entries)) => {
                self.nodes = Some(decode_nodes(entries));
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

/// Decode the `Generic bezierCurve { Array nodes { ... } }` shape — the
/// scene host stores arrays as `Generic([("size", Integer), ("0", elem), ...])`
/// and each element is `Generic([("data", Generic([("position", Vec3), ...]))])`.
fn decode_nodes(entries: &[(String, SceneValue)]) -> Vec<BezierNode> {
    let mut nodes_entries: &[(String, SceneValue)] = &[];
    for (name, value) in entries {
        if name == "nodes" {
            if let SceneValue::Generic(inner) = value {
                nodes_entries = inner.as_slice();
            }
        }
    }

    let mut size: usize = 0;
    let mut indexed: Vec<(usize, BezierNode)> = Vec::new();
    for (name, value) in nodes_entries {
        if name == "size" {
            if let SceneValue::Integer(n) = value {
                size = (*n).max(0) as usize;
            }
            continue;
        }
        let Ok(idx) = name.parse::<usize>() else {
            continue;
        };
        if let SceneValue::Generic(elem) = value {
            if let Some(n) = decode_node(elem) {
                indexed.push((idx, n));
            }
        }
    }
    let zero = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut out = vec![
        BezierNode {
            position: zero,
            tangent0: zero,
            tangent1: zero,
        };
        size
    ];
    for (idx, n) in indexed {
        if idx < out.len() {
            out[idx] = n;
        }
    }
    out
}

fn decode_node(entries: &[(String, SceneValue)]) -> Option<BezierNode> {
    // The element reader unwraps any `Generic data { ... }` wrapper so the
    // position/tangent fields appear directly here. Still tolerate the
    // wrapped form in case the shape changes.
    let data: &[(String, SceneValue)] = entries
        .iter()
        .find_map(|(n, v)| {
            if n == "data" {
                if let SceneValue::Generic(inner) = v {
                    return Some(inner.as_slice());
                }
            }
            None
        })
        .unwrap_or(entries);
    let mut position = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut tangent0 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    let mut tangent1 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    for (n, v) in data {
        match (n.as_str(), v) {
            ("position", SceneValue::Vector3(p)) => {
                position = Vec3 { x: p.x, y: p.y, z: p.z };
            }
            ("tangent0", SceneValue::Vector3(p)) => {
                tangent0 = Vec3 { x: p.x, y: p.y, z: p.z };
            }
            ("tangent1", SceneValue::Vector3(p)) => {
                tangent1 = Vec3 { x: p.x, y: p.y, z: p.z };
            }
            _ => {}
        }
    }
    Some(BezierNode {
        position,
        tangent0,
        tangent1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unity_runtime::scene::Scene;

    const OVERRIDE: &str = concat!(
        "GameObject LitArea\n",
        "\tComponent MentalTools.BezierCurve\n",
        "\t\tInteger bezierPointCount = 6\n",
        "\t\tGeneric bezierCurve\n",
        "\t\t\tArray nodes\n",
        "\t\t\t\tArraySize size = 2\n",
        "\t\t\t\tElement 0\n",
        "\t\t\t\t\tGeneric data\n",
        "\t\t\t\t\t\tVector3 position\n",
        "\t\t\t\t\t\t\tFloat x = 0\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
        "\t\t\t\t\t\tVector3 tangent0\n",
        "\t\t\t\t\t\t\tFloat x = 1\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
        "\t\t\t\t\t\tVector3 tangent1\n",
        "\t\t\t\t\t\t\tFloat x = -1\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
        "\t\t\t\tElement 1\n",
        "\t\t\t\t\tGeneric data\n",
        "\t\t\t\t\t\tVector3 position\n",
        "\t\t\t\t\t\t\tFloat x = 4\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
        "\t\t\t\t\t\tVector3 tangent0\n",
        "\t\t\t\t\t\t\tFloat x = 1\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
        "\t\t\t\t\t\tVector3 tangent1\n",
        "\t\t\t\t\t\t\tFloat x = -1\n",
        "\t\t\t\t\t\t\tFloat y = 0\n",
    );

    #[test]
    fn parses_bezier_curve_nodes() {
        let (scene, root) = Scene::from_override_text(OVERRIDE).expect("parse");
        let (_, bc) = scene
            .get_component_of::<BezierCurve>(root)
            .expect("BezierCurve attached");
        assert_eq!(bc.bezier_point_count, Some(6));
        let nodes = bc.nodes.as_ref().expect("nodes parsed");
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].position.x, 0.0);
        assert_eq!(nodes[1].position.x, 4.0);
        assert_eq!(nodes[0].tangent0.x, 1.0);
        assert_eq!(nodes[1].tangent1.x, -1.0);
    }
}
