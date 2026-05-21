//! Resolve sprites from a `ParsedPrefab` using the runtime metadata + atlas tables.

use std::collections::HashMap;

use super::WORLD_SCALE;
use super::atlas::{
    atlas_for_material_guid, is_representative_runtime_sprite_name, is_runtime_atlas_filename,
    runtime_unique_atlas_for_material_id,
};
use super::types::{
    ParsedPrefab, RuntimeSpriteComponent, RuntimeSpriteMeta, SpriteInfo, UNMANAGED_ATLAS,
};

pub(super) fn find_runtime_sprite_info(
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
) -> Option<SpriteInfo> {
    if let Some(component) = preferred_runtime_sprite_component(parsed)
        && let Some(info) = runtime_sprite_info_from_component(parsed, runtime_sprites, component)
    {
        return Some(info);
    }

    for component in &parsed.runtime_sprites {
        if let Some(info) = runtime_sprite_info_from_component(parsed, runtime_sprites, component)
        {
            return Some(info);
        }
    }

    None
}

fn preferred_runtime_sprite_component<'a>(
    parsed: &'a ParsedPrefab,
) -> Option<&'a RuntimeSpriteComponent> {
    parsed
        .runtime_sprites
        .iter()
        .find(|component| {
            parsed
                .game_objects
                .get(&component.game_object_id)
                .is_some_and(|game_object| is_representative_runtime_sprite_name(&game_object.name))
        })
}

fn runtime_sprite_info_from_component(
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
    component: &RuntimeSpriteComponent,
) -> Option<SpriteInfo> {
    let meta = runtime_sprites.get(&component.sprite_id)?;
    let atlas = preferred_renderer_runtime_atlas(parsed, component)
        .or_else(|| runtime_unique_atlas_for_material_id(&meta.material_id))
        .or_else(|| {
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

fn preferred_renderer_runtime_atlas(
    parsed: &ParsedPrefab,
    component: &RuntimeSpriteComponent,
) -> Option<&'static str> {
    let renderer = parsed.renderers.get(&component.game_object_id)?;
    let texture = crate::data::assets::effect_texture_name_for_material_guid(&renderer.material_guid)?;

    is_runtime_atlas_filename(texture).then_some(texture)
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

#[cfg(test)]
mod tests {
    use super::{preferred_runtime_sprite_component, runtime_sprite_info_from_component};
    use crate::data::assets;
    use crate::data::sprite_db::parse::parse_prefab;
    use crate::data::sprite_db::runtime::load_runtime_sprites;

    fn selected_runtime_child_name(prefab_name: &str) -> Option<String> {
        let path = format!("Assets/Prefab/{prefab_name}.prefab");
        let bytes = assets::read_pathname(&path)?;
        let text = String::from_utf8_lossy(bytes.as_ref());
        let parsed = parse_prefab(&text);
        let runtime_sprites = load_runtime_sprites();

        if let Some(component) = preferred_runtime_sprite_component(&parsed)
            && runtime_sprite_info_from_component(&parsed, &runtime_sprites, component).is_some()
        {
            return parsed
                .game_objects
                .get(&component.game_object_id)
                .map(|game_object| game_object.name.clone());
        }

        parsed.runtime_sprites.iter().find_map(|component| {
            runtime_sprite_info_from_component(&parsed, &runtime_sprites, component).and_then(
                |_| {
                    parsed
                        .game_objects
                        .get(&component.game_object_id)
                        .map(|game_object| game_object.name.clone())
                },
            )
        })
    }

    fn runtime_child_material_and_atlas(
        prefab_name: &str,
        child_name: &str,
    ) -> Option<(String, String)> {
        let path = format!("Assets/Prefab/{prefab_name}.prefab");
        let bytes = assets::read_pathname(&path)?;
        let text = String::from_utf8_lossy(bytes.as_ref());
        let parsed = parse_prefab(&text);
        let runtime_sprites = load_runtime_sprites();
        let component = parsed.runtime_sprites.iter().find(|component| {
            parsed
                .game_objects
                .get(&component.game_object_id)
                .is_some_and(|game_object| game_object.name == child_name)
        })?;
        let info = runtime_sprite_info_from_component(&parsed, &runtime_sprites, component)?;
        let material_id = runtime_sprites.get(&component.sprite_id)?.material_id.clone();
        Some((material_id, info.atlas))
    }

    #[test]
    fn semantic_priority_prefers_action_children() {
        assert_eq!(
            selected_runtime_child_name("AskAboutNotifications").as_deref(),
            Some("CloseButton")
        );
        assert_eq!(
            selected_runtime_child_name("DailyChallengeDialog").as_deref(),
            Some("BackButton")
        );
        assert_eq!(
            selected_runtime_child_name("CoinSalePopup").as_deref(),
            Some("OKButton")
        );
        assert_eq!(
            selected_runtime_child_name("NoFreeSlotsPopup").as_deref(),
            Some("BuyButton")
        );
        assert_eq!(
            selected_runtime_child_name("RewardPopup").as_deref(),
            Some("ClaimButton")
        );
        assert_eq!(
            selected_runtime_child_name("LevelRowUnlockPanel").as_deref(),
            Some("OpenPopupButton")
        );
    }

    #[test]
    fn semantic_priority_prefers_representative_icons_and_loading() {
        assert_eq!(
            selected_runtime_child_name("CakeRaceReplayEntry").as_deref(),
            Some("TrackIcon")
        );
        assert_eq!(
            selected_runtime_child_name("ResourceBar").as_deref(),
            Some("LevelIcon")
        );
        assert_eq!(
            selected_runtime_child_name("ScrapButton").as_deref(),
            Some("SoftCurrencyIcon")
        );
        assert_eq!(
            selected_runtime_child_name("SnoutButton").as_deref(),
            Some("SoftCurrencyIcon")
        );
        assert_eq!(
            selected_runtime_child_name("LeaderboardDialog").as_deref(),
            Some("Loading")
        );
        assert_eq!(
            selected_runtime_child_name("SeasonEndDialog").as_deref(),
            Some("Loading")
        );
        assert_eq!(
            selected_runtime_child_name("LeaderboardEntry").as_deref(),
            Some("InfoButton")
        );
        assert_eq!(
            selected_runtime_child_name("SingleLeaderboardEntry").as_deref(),
            Some("InfoButton")
        );
    }

    #[test]
    fn renderer_backed_atlas_resolution_handles_conflicting_runtime_material_ids() {
        let (material_id, atlas) = runtime_child_material_and_atlas(
            "DailyChallengeDialog",
            "ToggleCheats",
        )
        .expect("missing DailyChallengeDialog ToggleCheats runtime sprite info");
        assert_eq!(material_id, "f300c561f75e74380a11f80d4d2647f3");
        assert_eq!(atlas, "MenuAtlas.png");

        let (material_id, atlas) =
            runtime_child_material_and_atlas("PurchasePiggyPackIAP", "TextBox")
                .expect("missing PurchasePiggyPackIAP TextBox runtime sprite info");
        assert_eq!(material_id, "a286b652d38de4df384036482abc0571");
        assert_eq!(atlas, "MenuAtlas2.png");
    }
}
