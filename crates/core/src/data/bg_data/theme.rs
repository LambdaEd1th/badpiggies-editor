use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::domain::level::refs::MaterialShaderKind;

use super::parse::{
    GameObjectInfo, ParsedPrefab, SpriteComponent, asset_filename, is_sky_texture_asset,
    parse_prefab, read_embedded_text,
};
use super::tables::{bg_atlas_files, classify_group_layer, explicit_parallax_layer};
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

fn default_main_tex_st() -> [f32; 4] {
    [1.0, 1.0, 0.0, 0.0]
}

fn fallback_bg_shader_kind(
    fill_color: Option<[u8; 3]>,
    sky_texture: bool,
    alpha_blend: bool,
    alpha8bit: bool,
) -> MaterialShaderKind {
    if alpha8bit {
        MaterialShaderKind::CustomUnlitAlpha8BitColor
    } else if alpha_blend {
        if sky_texture {
            MaterialShaderKind::BuiltinUnlitTransparent
        } else {
            MaterialShaderKind::CustomUnlitColorTransparentGeometry
        }
    } else if fill_color.is_some() {
        MaterialShaderKind::CustomUnlitMonochrome
    } else {
        MaterialShaderKind::BuiltinUnlitTransparentCutout
    }
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
    let material_fill_color = crate::domain::level::refs::material_color_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_color_for_guid_prefix(material_guid));
    let material_tint = crate::domain::level::refs::material_color_rgba_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_color_rgba_for_guid_prefix(material_guid))
        .map(|[red, green, blue, alpha]| {
            [
                red as f32 / 255.0,
                green as f32 / 255.0,
                blue as f32 / 255.0,
                alpha as f32 / 255.0,
            ]
        })
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    let legacy_alpha_blend =
        crate::domain::level::refs::material_alpha_blend_for_guid(material_guid)
            || crate::domain::level::refs::material_alpha_blend_for_guid_prefix(material_guid);
    let legacy_alpha8bit = crate::domain::level::refs::material_alpha8bit_for_guid(material_guid)
        || crate::domain::level::refs::material_alpha8bit_for_guid_prefix(material_guid);
    let material_cutoff = crate::domain::level::refs::material_cutoff_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_cutoff_for_guid_prefix(material_guid))
        .unwrap_or(0.5);
    let custom_render_queue = crate::domain::level::refs::material_custom_render_queue_for_guid(
        material_guid,
    )
    .or_else(|| {
        crate::domain::level::refs::material_custom_render_queue_for_guid_prefix(material_guid)
    });
    let (atlas, sky_texture, fill_color) =
        if let Some(asset_name) = textureloader_materials.get(material_guid) {
            if is_sky_texture_asset(asset_name) {
                (None, Some(asset_filename(asset_name)), None)
            } else {
                (Some(asset_filename(asset_name)), None, None)
            }
        } else if let Some(atlas_name) = super::atlas_for_material_guid(material_guid) {
            (Some(atlas_name.to_string()), None, None)
        } else if let Some(fill_color) = material_fill_color {
            (None, None, Some(fill_color))
        } else {
            log::warn!(
                "Missing background material mapping for theme={}, sprite={}, guid={}",
                theme_name,
                game_object.name,
                material_guid
            );
            return None;
        };
    let shader_kind = crate::domain::level::refs::material_shader_kind_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_shader_kind_for_guid_prefix(material_guid))
        .unwrap_or_else(|| {
            fallback_bg_shader_kind(
                fill_color,
                sky_texture.is_some(),
                legacy_alpha_blend,
                legacy_alpha8bit,
            )
        });
    let main_tex_st = crate::domain::level::refs::material_main_tex_st_for_guid(material_guid)
        .or_else(|| crate::domain::level::refs::material_main_tex_st_for_guid_prefix(material_guid))
        .unwrap_or_else(default_main_tex_st);
    let atlas_soft_alpha_blend = atlas
        .as_deref()
        .is_some_and(|atlas_name| sprite_requires_soft_alpha_blend(atlas_name, sprite_component));
    let material_alpha_blend = matches!(
        shader_kind,
        MaterialShaderKind::CustomUnlitColorTransparentGeometry
            | MaterialShaderKind::CustomUnlitAlpha8BitColor
            | MaterialShaderKind::BuiltinUnlitTransparent
    );

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
        tint: material_tint,
        shader_kind,
        main_tex_st,
        custom_render_queue,
        alpha_blend: material_alpha_blend
            || group.layer == BgLayer::Camera
            || atlas_soft_alpha_blend,
        cutoff: material_cutoff,
    })
}

fn sprite_requires_soft_alpha_blend(atlas_name: &str, sprite_component: &SpriteComponent) -> bool {
    let Some(image) = background_atlas_image(atlas_name) else {
        return false;
    };

    let subdiv = sprite_component.subdiv;
    if subdiv <= 0.0 {
        return false;
    }

    let width = image.width() as f32;
    let height = image.height() as f32;
    let x0 = ((sprite_component.uv_x / subdiv) * width).round() as u32;
    let x1 = (((sprite_component.uv_x + sprite_component.width) / subdiv) * width).round() as u32;
    let y0 = (((subdiv - sprite_component.uv_y - sprite_component.height) / subdiv) * height)
        .round() as u32;
    let y1 = (((subdiv - sprite_component.uv_y) / subdiv) * height).round() as u32;
    let x0 = x0.min(image.width());
    let x1 = x1.min(image.width());
    let y0 = y0.min(image.height());
    let y1 = y1.min(image.height());
    if x0 >= x1 || y0 >= y1 {
        return false;
    }

    let mut saw_visible_pixel = false;
    for y in y0..y1 {
        for x in x0..x1 {
            let alpha = image.get_pixel(x, y).0[3];
            if alpha == 0 {
                continue;
            }
            saw_visible_pixel = true;
            if alpha >= 128 {
                return false;
            }
        }
    }

    saw_visible_pixel
}

fn background_atlas_image(atlas_name: &str) -> Option<&'static image::RgbaImage> {
    static IMAGES: OnceLock<HashMap<String, OnceLock<Option<image::RgbaImage>>>> = OnceLock::new();

    let images = IMAGES.get_or_init(|| {
        bg_atlas_files()
            .iter()
            .map(|atlas_name| (atlas_name.clone(), OnceLock::new()))
            .collect()
    });
    images
        .get(atlas_name)?
        .get_or_init(|| {
            let data =
                crate::data::assets::read_pathname(&format!("Assets/Texture2D/{atlas_name}"))?;
            image::load_from_memory(data.as_ref())
                .ok()
                .map(|image| image.to_rgba8())
        })
        .as_ref()
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

fn set_child_order_entry(child_order: &mut Vec<String>, root_order: i32, name: &str) {
    let Ok(index) = usize::try_from(root_order) else {
        return;
    };
    if child_order.len() <= index {
        child_order.resize(index + 1, String::new());
    }
    child_order[index] = name.to_string();
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
            set_child_order_entry(out.child_order, transform.root_order, &game_object.name);
            GroupContext {
                name: game_object.name.clone(),
                layer: classify_group_layer(&game_object.tag),
                origin: world_pos,
            }
        }
    };

    let group = if let Some(layer) = explicit_parallax_layer(&game_object.tag) {
        out.group_defaults
            .entry(game_object.name.clone())
            .or_insert(world_pos);
        GroupContext {
            name: game_object.name.clone(),
            layer,
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

struct ThemeSource {
    prefab_path: String,
    theme: OnceLock<Option<BgTheme>>,
}

fn theme_sources() -> &'static HashMap<String, ThemeSource> {
    static SOURCES: OnceLock<HashMap<String, ThemeSource>> = OnceLock::new();

    SOURCES.get_or_init(|| {
        let prefab_paths = crate::data::assets::list_pathnames(
            "Assets/Resources/environment/background/",
            ".prefab",
        );
        prefab_paths
            .into_iter()
            .filter_map(|prefab_path| {
                let prefab_name = Path::new(&prefab_path)
                    .file_stem()
                    .and_then(|stem| stem.to_str())?;
                let theme_name =
                    crate::data::assets::theme_name_for_background_prefab(prefab_name)?.to_string();
                Some((
                    theme_name,
                    ThemeSource {
                        prefab_path,
                        theme: OnceLock::new(),
                    },
                ))
            })
            .collect()
    })
}

/// Get background theme data by name.
pub fn get_theme(name: &str) -> Option<&'static BgTheme> {
    let source = theme_sources().get(name)?;
    source
        .theme
        .get_or_init(|| {
            // Establish shared dependency order before entering a per-theme cell.
            crate::domain::level::refs::prepare_material_lookup_tables();
            let _ = super::atlas_for_material_guid("");
            let theme = build_theme(name, &source.prefab_path, super::textureloader_materials());
            if theme.is_none() {
                log::error!(
                    "Failed to build embedded background theme {} from {}",
                    name,
                    source.prefab_path
                );
            }
            theme
        })
        .as_ref()
}
