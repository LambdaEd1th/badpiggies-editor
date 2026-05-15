use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::parse::{
    GameObjectInfo, ParsedPrefab, SpriteComponent, asset_filename, is_sky_texture_asset,
    load_textureloader_materials, parse_prefab, read_embedded_text,
};
use super::tables::{
    BG_THEME_PREFABS, alpha_blend_override, classify_group_layer, fill_color_override,
    supplemental_atlas_for_material, uses_own_group_context,
};
use super::types::{BgLayer, BgSprite, BgTheme};

#[derive(Clone)]
struct GroupContext {
    name: String,
    layer: BgLayer,
    origin: [f32; 3],
}

struct BgSpriteBuildInput<'a> {
    theme_name: &'a str,
    group: &'a GroupContext,
    game_object: &'a GameObjectInfo,
    world_pos: [f32; 3],
    world_scale: [f32; 3],
    sprite_component: &'a SpriteComponent,
    material_guid: &'a str,
    textureloader_materials: &'a HashMap<String, String>,
}

fn build_bg_sprite(input: BgSpriteBuildInput<'_>) -> Option<BgSprite> {
    let BgSpriteBuildInput {
        theme_name,
        group,
        game_object,
        world_pos,
        world_scale,
        sprite_component,
        material_guid,
        textureloader_materials,
    } = input;
    let fill_color = fill_color_override(theme_name, &game_object.name, &group.name);
    let (atlas, sky_texture) = if fill_color.is_some() {
        (None, None)
    } else if let Some(asset_name) = textureloader_materials.get(material_guid) {
        if is_sky_texture_asset(asset_name) {
            (None, Some(asset_filename(asset_name)))
        } else {
            (Some(asset_filename(asset_name)), None)
        }
    } else if let Some(atlas_name) = supplemental_atlas_for_material(material_guid) {
        (Some(atlas_name.to_string()), None)
    } else {
        log::warn!(
            "Missing background material mapping for theme={}, sprite={}, guid={}",
            theme_name,
            game_object.name,
            material_guid
        );
        return None;
    };

    let is_group_root = game_object.name == group.name;
    Some(BgSprite {
        name: game_object.name.clone(),
        atlas,
        fill_color,
        sky_texture,
        uv_x: sprite_component.uv_x,
        uv_y: sprite_component.uv_y,
        grid_w: sprite_component.width,
        grid_h: sprite_component.height,
        sprite_w: sprite_component.sprite_width,
        sprite_h: sprite_component.sprite_height,
        subdiv: sprite_component.subdiv,
        border: sprite_component.border,
        world_x: world_pos[0],
        world_y: world_pos[1],
        world_z: world_pos[2],
        scale_x: world_scale[0],
        scale_y: world_scale[1],
        layer: group.layer,
        local_x: if is_group_root {
            world_pos[0]
        } else {
            world_pos[0] - group.origin[0]
        },
        local_y: if is_group_root {
            world_pos[1]
        } else {
            world_pos[1] - group.origin[1]
        },
        parent_group: group.name.clone(),
        tint: [1.0, 1.0, 1.0, 1.0],
        alpha_blend: alpha_blend_override(theme_name, &game_object.name, &group.name, group.layer),
    })
}

fn combine_world_pos(
    parent_pos: [f32; 3],
    parent_scale: [f32; 3],
    local_pos: [f32; 3],
) -> [f32; 3] {
    [
        parent_pos[0] + parent_scale[0] * local_pos[0],
        parent_pos[1] + parent_scale[1] * local_pos[1],
        parent_pos[2] + parent_scale[2] * local_pos[2],
    ]
}

fn combine_world_scale(parent_scale: [f32; 3], local_scale: [f32; 3]) -> [f32; 3] {
    [
        parent_scale[0] * local_scale[0],
        parent_scale[1] * local_scale[1],
        parent_scale[2] * local_scale[2],
    ]
}

struct BgTraverseCtx<'a> {
    theme_name: &'a str,
    prefab: &'a ParsedPrefab,
    textureloader_materials: &'a HashMap<String, String>,
}

struct BgTraverseOutput<'a> {
    group_defaults: &'a mut HashMap<String, [f32; 3]>,
    child_order: &'a mut Vec<String>,
    sprites: &'a mut Vec<BgSprite>,
}

fn traverse_group(
    ctx: &BgTraverseCtx<'_>,
    transform_id: &str,
    parent_pos: [f32; 3],
    parent_scale: [f32; 3],
    group: Option<GroupContext>,
    out: &mut BgTraverseOutput<'_>,
) {
    let Some(transform) = ctx.prefab.transforms.get(transform_id) else {
        return;
    };
    let Some(game_object) = ctx.prefab.game_objects.get(&transform.game_object_id) else {
        return;
    };
    if !game_object.active {
        return;
    }

    let world_pos = combine_world_pos(parent_pos, parent_scale, transform.local_pos);
    let world_scale = combine_world_scale(parent_scale, transform.local_scale);
    let group = match group {
        Some(group) => group,
        None => {
            out.group_defaults
                .insert(game_object.name.clone(), world_pos);
            out.child_order.push(game_object.name.clone());
            GroupContext {
                name: game_object.name.clone(),
                layer: classify_group_layer(&game_object.tag, &game_object.name),
                origin: world_pos,
            }
        }
    };

    let group = if uses_own_group_context(ctx.theme_name, &game_object.name, &group.name) {
        out.group_defaults
            .entry(game_object.name.clone())
            .or_insert(world_pos);
        GroupContext {
            name: game_object.name.clone(),
            layer: classify_group_layer(&game_object.tag, &game_object.name),
            origin: world_pos,
        }
    } else {
        group
    };

    if let Some(sprite_component) = ctx.prefab.sprites.get(&transform.game_object_id)
        && let Some(material_guid) = ctx.prefab.renderers.get(&transform.game_object_id)
        && let Some(sprite) = build_bg_sprite(BgSpriteBuildInput {
            theme_name: ctx.theme_name,
            group: &group,
            game_object,
            world_pos,
            world_scale,
            sprite_component,
            material_guid,
            textureloader_materials: ctx.textureloader_materials,
        })
    {
        out.sprites.push(sprite);
    }

    let mut children = transform.children.clone();
    children.sort_by_key(|child_id| {
        ctx.prefab
            .transforms
            .get(child_id)
            .map(|child| child.root_order)
            .unwrap_or_default()
    });
    for child_id in children {
        traverse_group(
            ctx,
            &child_id,
            world_pos,
            world_scale,
            Some(group.clone()),
            out,
        );
    }
}

fn build_theme(
    theme_name: &str,
    prefab_path: &str,
    textureloader_materials: &HashMap<String, String>,
) -> Option<BgTheme> {
    let raw = read_embedded_text(prefab_path)?;
    let prefab = parse_prefab(&raw)?;
    let root_transform = prefab.transforms.get(&prefab.root_transform_id)?;

    let mut group_defaults = HashMap::new();
    let mut child_order = Vec::new();
    let mut sprites = Vec::new();
    let ctx = BgTraverseCtx {
        theme_name,
        prefab: &prefab,
        textureloader_materials,
    };

    let mut children = root_transform.children.clone();
    children.sort_by_key(|child_id| {
        prefab
            .transforms
            .get(child_id)
            .map(|child| child.root_order)
            .unwrap_or_default()
    });
    {
        let mut out = BgTraverseOutput {
            group_defaults: &mut group_defaults,
            child_order: &mut child_order,
            sprites: &mut sprites,
        };
        for child_id in children {
            traverse_group(
                &ctx,
                &child_id,
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                None,
                &mut out,
            );
        }
    }

    sprites.sort_by(|a, b| {
        a.layer
            .order()
            .cmp(&b.layer.order())
            .then_with(|| b.world_z.partial_cmp(&a.world_z).unwrap_or(Ordering::Equal))
    });

    Some(BgTheme {
        sprites,
        group_defaults,
        child_order,
    })
}

static BG_THEMES: OnceLock<HashMap<String, BgTheme>> = OnceLock::new();

fn build_themes() -> HashMap<String, BgTheme> {
    let textureloader_materials = load_textureloader_materials();
    let mut themes = HashMap::with_capacity(BG_THEME_PREFABS.len());
    for (theme_name, prefab_path) in BG_THEME_PREFABS {
        match build_theme(theme_name, prefab_path, &textureloader_materials) {
            Some(theme) => {
                themes.insert((*theme_name).to_string(), theme);
            }
            None => {
                log::error!(
                    "Failed to build embedded background theme {} from {}",
                    theme_name,
                    prefab_path
                );
            }
        }
    }
    themes
}

/// Get background theme data by name.
pub fn get_theme(name: &str) -> Option<&'static BgTheme> {
    let themes = BG_THEMES.get_or_init(build_themes);
    themes.get(name)
}
