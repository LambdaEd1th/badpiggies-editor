//! Resolve sprites from a `ParsedPrefab` using the runtime metadata + atlas tables.

use std::collections::HashMap;

use super::WORLD_SCALE;
use super::atlas::{atlas_for_material_guid, preferred_runtime_sprite_id, runtime_atlas_for};
use super::types::{
    ParsedPrefab, RuntimeSpriteComponent, RuntimeSpriteMeta, SpriteInfo, UNMANAGED_ATLAS,
};

pub(super) fn find_runtime_sprite_info(
    prefab_name: &str,
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
) -> Option<SpriteInfo> {
    if let Some(sprite_id) = preferred_runtime_sprite_id(prefab_name)
        && let Some(component) = parsed
            .runtime_sprites
            .iter()
            .find(|component| component.sprite_id == sprite_id)
        && let Some(info) =
            runtime_sprite_info_from_component(prefab_name, parsed, runtime_sprites, component)
    {
        return Some(info);
    }

    for component in &parsed.runtime_sprites {
        if let Some(info) =
            runtime_sprite_info_from_component(prefab_name, parsed, runtime_sprites, component)
        {
            return Some(info);
        }
    }

    None
}

fn runtime_sprite_info_from_component(
    prefab_name: &str,
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
    component: &RuntimeSpriteComponent,
) -> Option<SpriteInfo> {
    let meta = runtime_sprites.get(&component.sprite_id)?;
    let atlas = runtime_atlas_for(prefab_name, &meta.material_id).or_else(|| {
        parsed
            .renderers
            .get(&component.game_object_id)
            .and_then(|renderer| atlas_for_material_guid(&renderer.material_guid))
    })?;
    Some(SpriteInfo {
        atlas: atlas.to_string(),
        uv: meta.uv,
        world_w: meta.width * component.scale_x * WORLD_SCALE,
        world_h: meta.height * component.scale_y * WORLD_SCALE,
    })
}

pub(super) fn find_unmanaged_sprite_info(
    prefab_name: &str,
    parsed: &ParsedPrefab,
) -> Option<SpriteInfo> {
    // Regular GoalArea prefabs render their cloth via the dedicated GoalSprite mesh,
    // not via a rectangular sprite component. Falling back to the goal-achievement
    // child here produces the bogus square that regressed after GoalArea left the GPU path.
    if prefab_name.starts_with("GoalArea") {
        return None;
    }

    let root_transform_id = parsed
        .transforms
        .iter()
        .find(|(_, transform)| transform.father == "0")
        .map(|(id, _)| id.clone())?;
    find_unmanaged_sprite_info_at(&root_transform_id, parsed)
}

fn find_unmanaged_sprite_info_at(transform_id: &str, parsed: &ParsedPrefab) -> Option<SpriteInfo> {
    let transform = parsed.transforms.get(transform_id)?;
    let game_object = parsed.game_objects.get(&transform.game_object_id)?;
    if !game_object.active {
        return None;
    }

    if let Some(renderer) = parsed.renderers.get(&transform.game_object_id)
        && renderer.enabled
        && let Some(component) = parsed.unmanaged_sprites.get(&transform.game_object_id)
    {
        return Some(SpriteInfo {
            atlas: UNMANAGED_ATLAS.to_string(),
            uv: component.uv,
            world_w: component.world_w,
            world_h: component.world_h,
        });
    }

    for child_id in &transform.children {
        if let Some(info) = find_unmanaged_sprite_info_at(child_id, parsed) {
            return Some(info);
        }
    }

    None
}
