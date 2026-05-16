use std::collections::HashMap;

use crate::domain::prefab_override_host::{
    RuntimeComponentContext, RuntimeOnDataLoadedHook,
    apply_runtime_on_data_loaded_hooks_with_prefab_asset,
};
use crate::domain::prefab_override_runtime::{RuntimeOverrideDocument, RuntimeOverrideNode};

use super::types::{BgOverrides, BgSprite, BgTheme};

const BACKGROUND_OBJECT_PREFAB_ASSET: &str = "unity/background/backgroundobject.prefab";

/// Parse BG overrides using Unity-like runtime ordering.
///
/// `Transform` fields are applied first from the generic override document,
/// then `PositionSerializer.childLocalPositions` may overwrite group
/// positions by root-order, mirroring Unity's post-apply `OnDataLoaded`
/// behavior for EP6 background prefabs.
pub fn parse_runtime_bg_overrides(raw: &str, child_order: &[String]) -> BgOverrides {
    let mut result = BgOverrides::default();
    let document = RuntimeOverrideDocument::parse(raw);

    for root in document.roots_of_type("GameObject") {
        for group in root.children_of_type("GameObject") {
            if let Some(position) = local_position_override(group) {
                result.groups.insert(group.name.clone(), position);
            }

            for sprite in group.children_of_type("GameObject") {
                if let Some(position) = local_position_override(sprite) {
                    result.sprites.insert(sprite.name.clone(), position);
                }
            }
        }
    }

    let hook = PositionSerializerBgHook { child_order };
    apply_runtime_on_data_loaded_hooks_with_prefab_asset(
        &document,
        &mut result,
        BACKGROUND_OBJECT_PREFAB_ASSET,
        &[&hook],
    );

    result
}

fn local_position_override(node: &RuntimeOverrideNode) -> Option<[Option<f32>; 3]> {
    let local_position = node
        .component("Transform")?
        .child("Vector3", "m_LocalPosition")?;

    let out = local_position.partial_vec3();

    if out.iter().all(Option::is_none) {
        None
    } else {
        Some(out)
    }
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
    let document = RuntimeOverrideDocument::parse(raw);
    let hook = PositionSerializerBgHook { child_order };
    apply_runtime_on_data_loaded_hooks_with_prefab_asset(
        &document,
        &mut result,
        BACKGROUND_OBJECT_PREFAB_ASSET,
        &[&hook],
    );
    result
}

struct PositionSerializerBgHook<'a> {
    child_order: &'a [String],
}

impl RuntimeOnDataLoadedHook<BgOverrides> for PositionSerializerBgHook<'_> {
    fn component_suffix(&self) -> &'static str {
        "PositionSerializer"
    }

    fn on_data_loaded(&self, context: RuntimeComponentContext<'_>, result: &mut BgOverrides) {
        if self.child_order.is_empty() {
            return;
        }

        let Some(array) = context
            .component
            .and_then(|component| component.child("Array", "childLocalPositions"))
            .and_then(|node| node.as_array())
        else {
            return;
        };

        for element in array.iter() {
            let idx = element.index;
            if idx >= self.child_order.len() || self.child_order[idx].is_empty() {
                continue;
            }
            result.groups.insert(
                self.child_order[idx].clone(),
                partial_position_override(&element.value),
            );
        }
    }
}

fn partial_position_override(node: &RuntimeOverrideNode) -> [Option<f32>; 3] {
    if node.node_type == "Vector3" {
        node.partial_vec3()
    } else {
        node.find_descendant(&|child| child.node_type == "Vector3" && child.name == "data")
            .map(RuntimeOverrideNode::partial_vec3)
            .unwrap_or([None, None, None])
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_position_serializer_overrides, parse_runtime_bg_overrides};

    const POSITION_SERIALIZER_OVERRIDE: &str = "GameObject BackgroundObject\n\tComponent PositionSerializer\n\t\tObjectReference prefab = 4\n\t\tArray childLocalPositions\n\t\t\tArraySize size = 7\n\t\t\tElement 0\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = 62.22481\n\t\t\t\tFloat z = 50\n\t\t\tElement 1\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = 7.8\n\t\t\t\tFloat z = 40\n\t\t\tElement 5\n\t\t\t\tVector3 data\n\t\t\t\tFloat z = -5\n\t\t\tElement 6\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = -14.15611\n";

    const COMBINED_BG_OVERRIDE: &str = "GameObject BackgroundObject\n\tGameObject Far\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = 1\n\t\t\t\tFloat y = 2\n\t\t\t\tFloat z = 3\n\tComponent PositionSerializer\n\t\tArray childLocalPositions\n\t\t\tArraySize size = 1\n\t\t\tElement 0\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = 20\n\t\t\t\tFloat z = 30\n";

    #[test]
    fn parses_position_serializer_child_local_positions_from_ast() {
        let child_order = vec![
            "Sky".to_string(),
            "Far".to_string(),
            String::new(),
            String::new(),
            String::new(),
            "Near".to_string(),
            "FG".to_string(),
        ];

        let overrides = parse_position_serializer_overrides(POSITION_SERIALIZER_OVERRIDE, &child_order);

        assert_eq!(overrides.groups.get("Sky"), Some(&[None, Some(62.22481), Some(50.0)]));
        assert_eq!(overrides.groups.get("Far"), Some(&[None, Some(7.8), Some(40.0)]));
        assert_eq!(overrides.groups.get("Near"), Some(&[None, None, Some(-5.0)]));
        assert_eq!(overrides.groups.get("FG"), Some(&[None, Some(-14.15611), None]));
        assert_eq!(overrides.groups.len(), 4);
    }

    #[test]
    fn runtime_bg_overrides_apply_position_serializer_after_transform() {
        let child_order = vec!["Far".to_string()];

        let overrides = parse_runtime_bg_overrides(COMBINED_BG_OVERRIDE, &child_order);

        assert_eq!(overrides.groups.get("Far"), Some(&[None, Some(20.0), Some(30.0)]));
    }
}
