use std::collections::HashMap;

use super::types::{BgOverrides, BgSprite, BgTheme};

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

    if !raw.contains("PositionSerializer") || child_order.is_empty() {
        return result;
    }

    let mut current_element: Option<usize> = None;
    let mut current_pos: [Option<f32>; 3] = [None, None, None];

    for line in raw.lines() {
        let content = line.trim();
        if let Some(rest) = content.strip_prefix("Element ") {
            // Flush previous element
            if let Some(idx) = current_element
                && idx < child_order.len()
                && !child_order[idx].is_empty()
            {
                result.groups.insert(child_order[idx].clone(), current_pos);
            }
            current_element = rest.trim().parse::<usize>().ok();
            current_pos = [None, None, None];
        } else if let Some(rest) = content.strip_prefix("Float ")
            && let Some((axis, val_str)) = rest.split_once('=')
            && let Ok(val) = val_str.trim().parse::<f32>()
        {
            match axis.trim() {
                "x" => current_pos[0] = Some(val),
                "y" => current_pos[1] = Some(val),
                "z" => current_pos[2] = Some(val),
                _ => {}
            }
        }
    }
    // Flush last element
    if let Some(idx) = current_element
        && idx < child_order.len()
        && !child_order[idx].is_empty()
    {
        result.groups.insert(child_order[idx].clone(), current_pos);
    }

    result
}
