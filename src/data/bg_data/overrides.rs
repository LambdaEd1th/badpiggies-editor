use std::collections::HashMap;

use crate::domain::types::Vec3;
use crate::unity_runtime::scene::{Scene, SceneValue};

use super::types::{BgOverrides, BgSprite, BgTheme};

fn scene_value_as_vec3(value: &SceneValue) -> Option<Vec3> {
    match value {
        SceneValue::Vector3(value) => Some(*value),
        SceneValue::Generic(entries) => {
            if let Some(value) = entries.iter().find_map(|(name, value)| {
                (name == "data").then(|| scene_value_as_vec3(value)).flatten()
            }) {
                return Some(value);
            }

            let mut x = None;
            let mut y = None;
            let mut z = None;
            for (name, value) in entries {
                match (name.as_str(), value) {
                    ("x", SceneValue::Float(value)) => x = Some(*value),
                    ("x", SceneValue::Integer(value)) => x = Some(*value as f32),
                    ("y", SceneValue::Float(value)) => y = Some(*value),
                    ("y", SceneValue::Integer(value)) => y = Some(*value as f32),
                    ("z", SceneValue::Float(value)) => z = Some(*value),
                    ("z", SceneValue::Integer(value)) => z = Some(*value as f32),
                    _ => {}
                }
            }

            Some(Vec3 {
                x: x.unwrap_or(0.0),
                y: y.unwrap_or(0.0),
                z: z.unwrap_or(0.0),
            })
        }
        _ => None,
    }
}

/// Parse BG override text from a BackgroundObject's PrefabOverrideData.
pub fn parse_bg_overrides(raw: &str) -> BgOverrides {
    let mut result = BgOverrides::default();
    let mut current_group = String::new();
    let mut current_sprite = String::new();
    // "group" or "sprite" — what m_LocalPosition applies to
    let mut parsing_for = "";

    for line in raw.lines() {
        let stripped = line.trim_end_matches('\r');
        let depth = stripped.len() - stripped.trim_start_matches('\t').len();
        let content = stripped.trim();
        if content.is_empty() {
            continue;
        }

        if depth == 1 && content.starts_with("GameObject ") {
            current_group = content[11..].trim().to_string();
            current_sprite.clear();
            parsing_for = "";
        } else if depth == 2 && content.starts_with("GameObject ") {
            current_sprite = content[11..].trim().to_string();
            parsing_for = "";
        } else if depth == 2 && content == "Component UnityEngine.Transform" {
            parsing_for = "group";
        } else if depth == 3 && content == "Component UnityEngine.Transform" {
            parsing_for = "sprite";
        } else if content.starts_with("Float ")
            && let Some(rest) = content.strip_prefix("Float ")
        {
            // Parse "x = 1.23" or "y = -4.56"
            let parts: Vec<&str> = rest.splitn(2, '=').collect();
            if parts.len() == 2 {
                let axis = parts[0].trim();
                if let Ok(val) = parts[1].trim().parse::<f32>() {
                    let (target_name, target_map) = match parsing_for {
                        "group" => (&current_group, &mut result.groups),
                        "sprite" => (&current_sprite, &mut result.sprites),
                        _ => continue,
                    };
                    if !target_name.is_empty() {
                        let entry = target_map
                            .entry(target_name.clone())
                            .or_insert([None, None, None]);
                        match axis {
                            "x" => entry[0] = Some(val),
                            "y" => entry[1] = Some(val),
                            "z" => entry[2] = Some(val),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    result
}

/// Apply BG overrides to a theme's sprites, returning modified copies with updated positions.
pub fn apply_bg_overrides(theme: &BgTheme, overrides: &BgOverrides) -> Vec<BgSprite> {
    if overrides.groups.is_empty() && overrides.sprites.is_empty() {
        return theme.sprites.clone();
    }
    theme
        .sprites
        .iter()
        .map(|s| {
            let defaults = match theme.group_defaults.get(&s.parent_group) {
                Some(d) => d,
                None => return s.clone(),
            };
            let group_ovr = overrides.groups.get(&s.parent_group);
            let sprite_ovr = overrides.sprites.get(&s.name);
            if group_ovr.is_none() && sprite_ovr.is_none() {
                return s.clone();
            }
            let gx = group_ovr.and_then(|o| o[0]).unwrap_or(defaults[0]);
            let gy = group_ovr.and_then(|o| o[1]).unwrap_or(defaults[1]);
            let gz = group_ovr.and_then(|o| o[2]).unwrap_or(defaults[2]);
            // When a sprite IS its own group parent (name == parentGroup),
            // its default localY already equals the group default position
            // (both represent the same Transform.m_LocalPosition). Using
            // localY here would double-count the offset. Treat it as 0.
            let is_group_root = s.name == s.parent_group;
            let lx = sprite_ovr.and_then(|o| o[0]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_x
            });
            let ly = sprite_ovr.and_then(|o| o[1]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_y
            });
            let new_x = gx + lx;
            let new_y = gy + ly;
            let sprite_local_z = s.world_z - defaults[2];
            let new_z = gz + sprite_local_z;
            let mut out = s.clone();
            out.world_x = new_x;
            out.world_y = new_y;
            out.world_z = new_z;
            out
        })
        .collect()
}

/// Parse PositionSerializer `childLocalPositions` into group overrides.
///
/// EP6 background prefabs use a `PositionSerializer` component with an array
/// of child positions (indexed by `m_RootOrder`).  The `child_order` slice
/// maps each array index to the corresponding background group name so we
/// can produce the same `BgOverrides` struct that `apply_bg_overrides` expects.
pub fn parse_position_serializer_overrides(raw: &str, child_order: &[String]) -> BgOverrides {
    let mut result = BgOverrides {
        groups: HashMap::new(),
        sprites: HashMap::new(),
    };

    if child_order.is_empty() {
        return result;
    }

    let Some((scene, _root)) = Scene::from_override_text(raw) else {
        return result;
    };

    for (_, component) in scene.iter_components() {
        if component.behavior.component_suffix() != "PositionSerializer" {
            continue;
        }

        let Some((_, SceneValue::Generic(entries))) = component
            .behavior
            .extra()
            .iter()
            .find(|(name, _)| name == "childLocalPositions")
        else {
            continue;
        };

        for (name, value) in entries {
            let Ok(idx) = name.parse::<usize>() else {
                continue;
            };
            if idx >= child_order.len() || child_order[idx].is_empty() {
                continue;
            }
            let Some(value) = scene_value_as_vec3(value) else {
                continue;
            };

            result.groups.insert(
                child_order[idx].clone(),
                [Some(value.x), Some(value.y), Some(value.z)],
            );
        }
    }

    result
}
