//! `CameraPreview` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/CameraPreview.cs`.
//!
//! Renderer pipeline consumers:
//! - `renderer/level_setup/mod.rs` — reads [`CameraPreview::control_points`] for
//!   sandbox levels whose camera tour is serialised directly in the scene.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

/// One Catmull-Rom control point on the camera preview route.
#[derive(Debug, Clone, Default)]
pub struct CameraControlPoint {
    /// World-space XY position of the camera.
    pub position: [f32; 2],
    /// Orthographic half-size (zoom) at this point.
    pub zoom: f32,
}

#[derive(Debug, Clone, Default)]
pub struct CameraPreview {
    /// Serialised control points (`m_controlPoints`).
    /// Empty for levels where the route is built dynamically at runtime.
    pub control_points: Vec<CameraControlPoint>,
    /// Total preview animation time in seconds (`m_animationTime`).
    pub animation_time: f32,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for CameraPreview {
    fn component_suffix(&self) -> &str {
        "CameraPreview"
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, value) {
            ("m_animationTime", SceneValue::Float(v)) => {
                self.animation_time = v;
                true
            }
            ("m_controlPoints", SceneValue::Generic(entries)) => {
                self.control_points = decode_control_points(&entries);
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

/// Decode the array-of-generic shape used by `m_controlPoints`:
/// ```text
/// Array m_controlPoints
///     ArraySize size = N
///     Element 0
///         Generic data
///             Vector2 position
///                 Float x = …
///                 Float y = …
///             Float zoom = …
///     Element 1
///         …
/// ```
///
/// The host wraps this as:
/// `Generic([("size", Integer(N)), ("0", Generic([("position", Vector2), ("zoom", Float)])), …])`
fn decode_control_points(entries: &[(String, SceneValue)]) -> Vec<CameraControlPoint> {
    let mut size: usize = 0;
    let mut indexed: Vec<(usize, CameraControlPoint)> = Vec::new();

    for (name, value) in entries {
        if name == "size" {
            if let SceneValue::Integer(n) = value {
                size = (*n).max(0) as usize;
            }
            continue;
        }
        let Ok(idx) = name.parse::<usize>() else {
            continue;
        };

        // Each element value is Generic([("position", Vector2), ("zoom", Float)])
        // (the "Generic data" name is consumed by the array reader; children
        // are directly in the element value — no extra "data" wrapper).
        let SceneValue::Generic(data) = value else {
            continue;
        };

        let mut pt = CameraControlPoint::default();
        for (field, fval) in data {
            match (field.as_str(), fval) {
                ("position", SceneValue::Vector2(p)) => {
                    pt.position = [p.x, p.y];
                }
                ("zoom", SceneValue::Float(z)) => {
                    pt.zoom = *z;
                }
                _ => {}
            }
        }
        indexed.push((idx, pt));
    }

    indexed.sort_by_key(|(i, _)| *i);
    let mut out = Vec::with_capacity(size);
    for (_, pt) in indexed {
        out.push(pt);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unity_runtime::scene::Scene;

    const OVERRIDE: &str = concat!(
        "GameObject GameCamera\n",
        "\tComponent CameraPreview\n",
        "\t\tFloat m_animationTime = 6\n",
        "\t\tArray m_controlPoints\n",
        "\t\t\tArraySize size = 3\n",
        "\t\t\tElement 0\n",
        "\t\t\t\tGeneric data\n",
        "\t\t\t\t\tVector2 position\n",
        "\t\t\t\t\t\tFloat x = -60\n",
        "\t\t\t\t\t\tFloat y = 50\n",
        "\t\t\t\t\tFloat zoom = 20\n",
        "\t\t\tElement 1\n",
        "\t\t\t\tGeneric data\n",
        "\t\t\t\t\tVector2 position\n",
        "\t\t\t\t\t\tFloat x = 40\n",
        "\t\t\t\t\t\tFloat y = 10\n",
        "\t\t\t\t\tFloat zoom = 15\n",
        "\t\t\tElement 2\n",
        "\t\t\t\tGeneric data\n",
        "\t\t\t\t\tVector2 position\n",
        "\t\t\t\t\t\tFloat x = -20\n",
        "\t\t\t\t\t\tFloat y = 60\n",
        "\t\t\t\t\tFloat zoom = 5\n",
    );

    #[test]
    fn parses_camera_preview_control_points() {
        let (scene, root) = Scene::from_override_text(OVERRIDE).expect("override parses");
        let (_, cp) = scene
            .get_component_of::<CameraPreview>(root)
            .expect("CameraPreview attached");

        assert_eq!(cp.animation_time, 6.0);
        assert_eq!(cp.control_points.len(), 3);

        assert_eq!(cp.control_points[0].position, [-60.0, 50.0]);
        assert!((cp.control_points[0].zoom - 20.0).abs() < 1e-4);

        assert_eq!(cp.control_points[1].position, [40.0, 10.0]);
        assert!((cp.control_points[1].zoom - 15.0).abs() < 1e-4);

        assert_eq!(cp.control_points[2].position, [-20.0, 60.0]);
        assert!((cp.control_points[2].zoom - 5.0).abs() < 1e-4);
    }
}
