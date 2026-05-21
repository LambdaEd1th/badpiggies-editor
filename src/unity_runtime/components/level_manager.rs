//! `LevelManager` MonoBehaviour. C# source: `Assets/Scripts/Assembly-CSharp/LevelManager.cs`.
//!
//! Renderer pipeline consumers:
//! - `renderer/grid.rs` — reads [`construction_grid_rows`] + [`grid_cell_prefab`]
//! - `renderer/dark_overlay/parse.rs` — reads [`dark_level`]
//! - `renderer/level_setup/mod.rs` — reads [`camera_limits`]

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

/// Camera limits rectangle: `top_left.x, top_left.y, size.x, size.y`.
pub type CameraLimits = [f32; 4];

#[derive(Debug, Clone, Default)]
pub struct LevelManager {
    pub dark_level: Option<bool>,
    /// Raw object-reference index for `m_gridCellPrefab` (typically resolves to
    /// the `GridCellLight` prefab when present). `Some(_)` means "override
    /// sets it"; the editor doesn't have the prefab table to resolve it.
    pub grid_cell_prefab: Option<i32>,
    /// One bitmask per construction-grid row (bit set = cell available).
    pub construction_grid_rows: Option<Vec<i32>>,
    /// Camera rectangle: top-left + size.
    pub camera_limits: Option<CameraLimits>,
    pub extra: Vec<(String, SceneValue)>,
}

impl UnityComponent for LevelManager {
    fn component_suffix(&self) -> &str {
        "LevelManager"
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        match (name, &value) {
            ("m_darkLevel", SceneValue::Boolean(b)) => {
                self.dark_level = Some(*b);
                true
            }
            ("m_cameraLimits", SceneValue::Generic(entries)) => {
                let mut tl = [0.0_f32; 2];
                let mut sz = [0.0_f32; 2];
                for (n, v) in entries {
                    match (n.as_str(), v) {
                        ("topLeft", SceneValue::Vector2(p)) => {
                            tl = [p.x, p.y];
                        }
                        ("size", SceneValue::Vector2(p)) => {
                            sz = [p.x, p.y];
                        }
                        _ => {}
                    }
                }
                self.camera_limits = Some([tl[0], tl[1], sz[0], sz[1]]);
                true
            }
            ("m_constructionGridRows", SceneValue::Generic(entries)) => {
                self.construction_grid_rows = Some(decode_int_array(entries));
                true
            }
            _ => false,
        }
    }

    fn set_object_reference_index(
        &mut self,
        _scene: &mut Scene,
        name: &str,
        index: i32,
    ) -> bool {
        if name == "m_gridCellPrefab" {
            self.grid_cell_prefab = Some(index);
            true
        } else {
            false
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

/// Decode the array-of-int shape produced by the Scene host's `set_array`:
/// `[("size", Integer(N)), ("0", value), ("1", value), ...]`. Elements with
/// missing indices default to zero (matches Unity's `IList<T>` sparse
/// semantics). Array elements are themselves the integer values directly, OR
/// nested `Generic([("data", Integer(v))])` when the source stream wrapped
/// each element in a `data` Generic.
fn decode_int_array(entries: &[(String, SceneValue)]) -> Vec<i32> {
    let mut size: usize = 0;
    let mut indexed: Vec<(usize, i32)> = Vec::new();
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
        let scalar = match value {
            SceneValue::Integer(v) => Some(*v),
            SceneValue::Float(v) => Some(*v as i32),
            SceneValue::Generic(inner) => inner.iter().find_map(|(n, v)| match (n.as_str(), v) {
                ("data", SceneValue::Integer(v)) => Some(*v),
                ("data", SceneValue::Float(v)) => Some(*v as i32),
                _ => None,
            }),
            _ => None,
        };
        if let Some(v) = scalar {
            indexed.push((idx, v));
        }
    }
    let mut out = vec![0_i32; size];
    for (idx, v) in indexed {
        if idx < out.len() {
            out[idx] = v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unity_runtime::scene::Scene;

    const OVERRIDE: &str = concat!(
        "GameObject LevelManager\n",
        "\tComponent LevelManager\n",
        "\t\tBoolean m_darkLevel = True\n",
        "\t\tArray m_constructionGridRows\n",
        "\t\t\tArraySize size = 4\n",
        "\t\t\tElement 0\n",
        "\t\t\t\tInteger data = 15\n",
        "\t\t\tElement 1\n",
        "\t\t\t\tInteger data = 15\n",
        "\t\t\tElement 2\n",
        "\t\t\t\tInteger data = 0\n",
        "\t\t\tElement 3\n",
        "\t\t\t\tInteger data = 2\n",
        "\t\tObjectReference m_gridCellPrefab = 6\n",
        "\t\tGeneric m_cameraLimits\n",
        "\t\t\tVector2 topLeft\n",
        "\t\t\t\tFloat x = -10\n",
        "\t\t\t\tFloat y = 5\n",
        "\t\t\tVector2 size\n",
        "\t\t\t\tFloat x = 20\n",
        "\t\t\t\tFloat y = -8\n",
    );

    #[test]
    fn parses_level_manager_fields() {
        let (scene, root) = Scene::from_override_text(OVERRIDE).expect("override parses");
        let (_, lm) = scene
            .get_component_of::<LevelManager>(root)
            .expect("LevelManager attached");
        assert_eq!(lm.dark_level, Some(true));
        assert_eq!(lm.grid_cell_prefab, Some(6));
        assert_eq!(lm.construction_grid_rows.as_deref(), Some(&[15, 15, 0, 2][..]));
        assert_eq!(lm.camera_limits, Some([-10.0, 5.0, 20.0, -8.0]));
    }
}
