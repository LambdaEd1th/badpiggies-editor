//! Cloud sprite system: drifting cloud instances and per-theme configurations.

use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui;

use crate::data::{assets, bg_data};
use crate::domain::types::Vec2;

use super::Camera;

/// An individual cloud sprite that drifts horizontally and wraps.
pub(super) struct CloudInstance {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub center_x: f32,
    pub limits: f32,
    pub velocity: f32,
    pub opacity: f32,
    /// Sprite name for UV lookup.
    pub sprite_name: String,
    /// Atlas path for texture.
    pub atlas: String,
    /// Scale multipliers (from config).
    pub scale_x: f32,
    pub scale_y: f32,
}

/// Cloud config per set (mirrors Unity CloudSetPlateau etc.)
#[derive(Clone)]
pub(super) struct CloudConfig {
    pub max_clouds: usize,
    pub velocity: f32,
    pub limits: f32,
    pub height: f32,
    pub far_plane: f32,
    pub sprites: Vec<CloudSpriteInfo>,
}

#[derive(Clone)]
pub(super) struct CloudSpriteInfo {
    pub name: String,
    pub atlas: String,
    pub scale_x: f32,
    pub scale_y: f32,
}

struct LegacyCloudConfig {
    name: &'static str,
    max_clouds: usize,
    velocity: f32,
    limits: f32,
    height: f32,
    far_plane: f32,
    sprites: &'static [LegacyCloudSpriteInfo],
}

struct LegacyCloudSpriteInfo {
    name: &'static str,
    atlas: &'static str,
    scale_x: f32,
    scale_y: f32,
}

const LEGACY_CLOUD_CONFIGS: &[LegacyCloudConfig] = &[
    LegacyCloudConfig {
        name: "CloudPlateauSet",
        max_clouds: 8,
        velocity: 0.2,
        limits: 93.24,
        height: 5.0,
        far_plane: 1.0,
        sprites: &[
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_01_SET_0",
                atlas: "Background_Plateaus_Sheet_01.png",
                scale_x: 3.0,
                scale_y: 3.0,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_02_SET_0",
                atlas: "Background_Plateaus_Sheet_01.png",
                scale_x: 3.0,
                scale_y: 3.0,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_03_SET_0",
                atlas: "Background_Plateaus_Sheet_01.png",
                scale_x: 3.0,
                scale_y: 3.0,
            },
        ],
    },
    LegacyCloudConfig {
        name: "CloudJungleSet",
        max_clouds: 8,
        velocity: 0.2,
        limits: 93.24,
        height: 10.0,
        far_plane: 1.0,
        sprites: &[
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_01_SET",
                atlas: "Background_Jungle_Sheet_01.png",
                scale_x: 0.87,
                scale_y: 0.6525,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_02_SET",
                atlas: "Background_Jungle_Sheet_01.png",
                scale_x: 1.0875,
                scale_y: 0.87,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_03_SET",
                atlas: "Background_Jungle_Sheet_01.png",
                scale_x: 1.0,
                scale_y: 0.8,
            },
        ],
    },
    LegacyCloudConfig {
        name: "CloudNightSet",
        max_clouds: 5,
        velocity: 0.2,
        limits: 40.0,
        height: 2.5,
        far_plane: 1.0,
        sprites: &[
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_01_SET_1",
                atlas: "Background_Night_Sheet_01.png",
                scale_x: 2.0,
                scale_y: 2.0,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_02_SET_1",
                atlas: "Background_Night_Sheet_01.png",
                scale_x: 2.0,
                scale_y: 2.0,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_03_SET_1",
                atlas: "Background_Night_Sheet_01.png",
                scale_x: 2.0,
                scale_y: 2.0,
            },
        ],
    },
    LegacyCloudConfig {
        name: "CloudHalloweenSet",
        max_clouds: 5,
        velocity: 0.5,
        limits: 93.0,
        height: 4.0,
        far_plane: 1.0,
        sprites: &[
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_01_SET_2",
                atlas: "Background_Halloween_Sheet_01.png",
                scale_x: 1.5,
                scale_y: 1.5,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_02_SET_2",
                atlas: "Background_Halloween_Sheet_01.png",
                scale_x: 1.2,
                scale_y: 1.2,
            },
        ],
    },
    LegacyCloudConfig {
        name: "CloudLPASet",
        max_clouds: 10,
        velocity: 0.1,
        limits: 250.0,
        height: 0.84,
        far_plane: 1.0,
        sprites: &[
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_02_SET_3",
                atlas: "Background_Maya_Sheet_02.png",
                scale_x: 1.0,
                scale_y: 1.0,
            },
            LegacyCloudSpriteInfo {
                name: "Background_Cloud_03_SET_2",
                atlas: "Background_Maya_Sheet_02.png",
                scale_x: 1.0,
                scale_y: 1.0,
            },
        ],
    },
];

#[derive(Clone)]
struct ParsedCloudSpritePrefab {
    name: String,
    atlas: String,
    scale_x: f32,
    scale_y: f32,
}

pub(super) fn cloud_config(name: &str) -> Option<&'static CloudConfig> {
    static PREFAB_CLOUD_CONFIGS: OnceLock<HashMap<String, CloudConfig>> = OnceLock::new();
    static FALLBACK_CLOUD_CONFIGS: OnceLock<HashMap<String, CloudConfig>> = OnceLock::new();

    PREFAB_CLOUD_CONFIGS
        .get_or_init(load_cloud_configs_from_prefabs)
        .get(name)
        .or_else(|| {
            FALLBACK_CLOUD_CONFIGS
                .get_or_init(build_legacy_cloud_config_map)
                .get(name)
        })
}

fn build_legacy_cloud_config_map() -> HashMap<String, CloudConfig> {
    LEGACY_CLOUD_CONFIGS
        .iter()
        .map(|config| {
            (
                config.name.to_string(),
                CloudConfig {
                    max_clouds: config.max_clouds,
                    velocity: config.velocity,
                    limits: config.limits,
                    height: config.height,
                    far_plane: config.far_plane,
                    sprites: config
                        .sprites
                        .iter()
                        .map(|sprite| CloudSpriteInfo {
                            name: sprite.name.to_string(),
                            atlas: sprite.atlas.to_string(),
                            scale_x: sprite.scale_x,
                            scale_y: sprite.scale_y,
                        })
                        .collect(),
                },
            )
        })
        .collect()
}

fn load_cloud_configs_from_prefabs() -> HashMap<String, CloudConfig> {
    let cloud_sprite_prefabs = load_cloud_sprite_prefabs();
    let mut configs = HashMap::new();

    for prefab_path in assets::list_asset_paths("Prefab/", ".prefab") {
        if !is_cloud_set_prefab_path(&prefab_path) {
            continue;
        }

        let Some(prefab_text) = assets::read_asset_text(&format!("unity/prefabs/{prefab_path}"))
        else {
            continue;
        };

        if let Some((name, config)) = parse_cloud_set_prefab(&prefab_text, &cloud_sprite_prefabs) {
            configs.insert(name, config);
        }
    }

    configs
}

fn load_cloud_sprite_prefabs() -> HashMap<String, ParsedCloudSpritePrefab> {
    let mut prefabs = HashMap::new();

    for prefab_path in assets::list_asset_paths("Prefab/", ".prefab") {
        if !is_cloud_sprite_prefab_path(&prefab_path) {
            continue;
        }

        let Some(prefab_text) = assets::read_asset_text(&format!("unity/prefabs/{prefab_path}"))
        else {
            continue;
        };

        if let Some((root_transform_id, prefab)) =
            parse_cloud_sprite_prefab(&prefab_path, &prefab_text)
        {
            prefabs.insert(root_transform_id, prefab);
        }
    }

    prefabs
}

fn is_cloud_set_prefab_path(prefab_path: &str) -> bool {
    let Some(name) = prefab_path.strip_suffix(".prefab") else {
        return false;
    };
    name.starts_with("Cloud") && name.ends_with("Set")
}

fn is_cloud_sprite_prefab_path(prefab_path: &str) -> bool {
    prefab_path.starts_with("Background_Cloud_") && prefab_path.ends_with(".prefab")
}

fn parse_cloud_sprite_prefab(
    prefab_path: &str,
    prefab_text: &str,
) -> Option<(String, ParsedCloudSpritePrefab)> {
    let mut root_transform_id = None::<String>;
    let mut scale_x = 1.0;
    let mut scale_y = 1.0;
    let mut material_guid = None::<String>;

    for doc in prefab_text.split("--- ").skip(1) {
        let Some(header) = doc.lines().next().map(str::trim) else {
            continue;
        };
        let Some((type_id, file_id)) = parse_doc_header(header) else {
            continue;
        };

        match type_id {
            4 => {
                let father = field_value(doc, "m_Father: ").and_then(extract_file_id);
                if father.as_deref() != Some("0") {
                    continue;
                }
                root_transform_id = Some(file_id.to_string());
                if let Some(local_scale) = field_value(doc, "m_LocalScale: ") {
                    scale_x = parse_vec_component(local_scale, "x").unwrap_or(1.0);
                    scale_y = parse_vec_component(local_scale, "y").unwrap_or(1.0);
                }
            }
            23 => {
                if material_guid.is_none() {
                    material_guid = doc.lines().find_map(extract_guid);
                }
            }
            _ => {}
        }
    }

    let name = prefab_path.strip_suffix(".prefab")?.to_string();
    let root_transform_id = root_transform_id?;
    let material_guid = material_guid?;
    let atlas = bg_data::atlas_for_material_guid(&material_guid)?.to_string();

    Some((
        root_transform_id,
        ParsedCloudSpritePrefab {
            name,
            atlas,
            scale_x,
            scale_y,
        },
    ))
}

fn parse_cloud_set_prefab(
    prefab_text: &str,
    cloud_sprite_prefabs: &HashMap<String, ParsedCloudSpritePrefab>,
) -> Option<(String, CloudConfig)> {
    let mut name = None::<String>;
    let mut max_clouds = None::<usize>;
    let mut velocity = None::<f32>;
    let mut limits = None::<f32>;
    let mut height = None::<f32>;
    let mut far_plane = None::<f32>;
    let mut sprite_ids = Vec::new();
    let mut in_cloud_set = false;

    for line in prefab_text.lines() {
        let trimmed = line.trim();

        if name.is_none() {
            name = trimmed
                .strip_prefix("m_Name: ")
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }

        if let Some(value) = trimmed.strip_prefix("m_maxClouds: ") {
            max_clouds = value.parse::<usize>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_cloudVelocity: ") {
            velocity = value.parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_cloudLimits: ") {
            limits = value.parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_height: ") {
            height = value.parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_farPlane: ") {
            far_plane = value.parse::<f32>().ok();
            continue;
        }

        if trimmed == "m_cloudSet:" {
            in_cloud_set = true;
            continue;
        }

        if in_cloud_set {
            if let Some(rest) = trimmed.strip_prefix("- ") {
                if let Some(file_id) = extract_file_id(rest) {
                    sprite_ids.push(file_id);
                }
                continue;
            }
            if !trimmed.is_empty() && !trimmed.starts_with('{') {
                in_cloud_set = false;
            }
        }
    }

    let sprites: Vec<CloudSpriteInfo> = sprite_ids
        .into_iter()
        .filter_map(|sprite_id| cloud_sprite_prefabs.get(&sprite_id))
        .map(|sprite| CloudSpriteInfo {
            name: sprite.name.clone(),
            atlas: sprite.atlas.clone(),
            scale_x: sprite.scale_x,
            scale_y: sprite.scale_y,
        })
        .collect();

    if sprites.is_empty() {
        return None;
    }

    Some((
        name?,
        CloudConfig {
            max_clouds: max_clouds?,
            velocity: velocity?,
            limits: limits?,
            height: height?,
            far_plane: far_plane?,
            sprites,
        },
    ))
}

fn parse_doc_header(header: &str) -> Option<(u32, &str)> {
    let mut parts = header.split_whitespace();
    let type_id = parts.next()?.strip_prefix("!u!")?.parse().ok()?;
    let file_id = parts.next()?.strip_prefix('&')?;
    Some((type_id, file_id))
}

fn field_value<'a>(doc: &'a str, prefix: &str) -> Option<&'a str> {
    doc.lines()
        .find_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
}

fn extract_file_id(value: &str) -> Option<String> {
    let start = value.find("fileID: ")? + "fileID: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    Some(tail[..end].trim().to_string())
}

fn extract_guid(line: &str) -> Option<String> {
    let start = line.find("guid: ")? + "guid: ".len();
    let tail = &line[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    Some(tail[..end].trim().to_string())
}

fn parse_vec_component(value: &str, axis: &str) -> Option<f32> {
    value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
        .find_map(|part| {
            let (key, raw) = part.trim().split_once(':')?;
            (key.trim() == axis).then(|| raw.trim().parse::<f32>().ok()).flatten()
        })
}

/// Update cloud positions (drift + wrap) without drawing.
pub(super) fn update_cloud_positions(clouds: &mut [CloudInstance], dt: f32) {
    for cloud in clouds.iter_mut() {
        cloud.x += cloud.velocity * dt;
        if cloud.x > cloud.center_x + cloud.limits {
            cloud.x = cloud.center_x - cloud.limits;
        } else if cloud.x < cloud.center_x - cloud.limits {
            cloud.x = cloud.center_x + cloud.limits;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cloud_config;

    #[test]
    fn cloud_jungle_set_loads_from_prefab() {
        let config = cloud_config("CloudJungleSet").expect("missing CloudJungleSet config");
        assert_eq!(config.max_clouds, 8);
        assert!((config.velocity - 0.2).abs() < 1e-6);
        assert_eq!(config.sprites.len(), 3);
        assert_eq!(config.sprites[0].name, "Background_Cloud_02_SET");
        assert_eq!(config.sprites[1].name, "Background_Cloud_03_SET");
        assert_eq!(config.sprites[2].name, "Background_Cloud_01_SET");
        assert_eq!(config.sprites[0].atlas, "Background_Jungle_Sheet_01.png");
        assert!((config.sprites[2].scale_x - 0.87).abs() < 1e-6);
        assert!((config.sprites[2].scale_y - 0.6525).abs() < 1e-6);
    }

    #[test]
    fn cloud_lpa_set_resolves_sprite_prefabs_by_file_id() {
        let config = cloud_config("CloudLPASet").expect("missing CloudLPASet config");
        assert_eq!(config.sprites.len(), 2);
        assert_eq!(config.sprites[0].name, "Background_Cloud_02_SET_3");
        assert_eq!(config.sprites[1].name, "Background_Cloud_03_SET_2");
        assert_eq!(config.sprites[0].atlas, "Background_Maya_Sheet_02.png");
        assert_eq!(config.sprites[1].atlas, "Background_Maya_Sheet_02.png");
    }
}

/// Update cloud positions and draw them sorted by Z (farthest first).
pub(super) fn update_and_draw_clouds(
    clouds: &mut [CloudInstance],
    dt: f32,
    camera: &Camera,
    painter: &egui::Painter,
    canvas_center: egui::Vec2,
    rect: egui::Rect,
    tex_cache: &assets::TextureCache,
) {
    let mut cloud_order: Vec<usize> = (0..clouds.len()).collect();
    cloud_order.sort_by(|&a, &b| {
        clouds[b]
            .z
            .partial_cmp(&clouds[a].z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for ci in cloud_order {
        let cloud = &mut clouds[ci];
        cloud.x += cloud.velocity * dt;
        if cloud.x > cloud.center_x + cloud.limits {
            cloud.x = cloud.center_x - cloud.limits;
        } else if cloud.x < cloud.center_x - cloud.limits {
            cloud.x = cloud.center_x + cloud.limits;
        }
        if let Some(info) = crate::data::sprite_db::get_sprite_info(&cloud.sprite_name) {
            let hw = info.world_w * cloud.scale_x;
            let hh = info.world_h * cloud.scale_y;
            let center = camera.world_to_screen(
                Vec2 {
                    x: cloud.x + camera.center.x,
                    y: cloud.y,
                },
                canvas_center,
            );
            let sw = hw * 2.0 * camera.zoom;
            let sh = hh * 2.0 * camera.zoom;
            if center.x + sw < rect.left() - 50.0
                || center.x - sw > rect.right() + 50.0
                || center.y + sh < rect.top() - 50.0
                || center.y - sh > rect.bottom() + 50.0
            {
                continue;
            }
            let draw_rect = egui::Rect::from_center_size(center, egui::vec2(sw, sh));
            if let Some(tex_id) = tex_cache.get(&cloud.atlas) {
                let uv_rect = egui::Rect::from_min_max(
                    egui::pos2(info.uv.x, 1.0 - info.uv.y - info.uv.h),
                    egui::pos2(info.uv.x + info.uv.w, 1.0 - info.uv.y),
                );
                let alpha = (cloud.opacity * 255.0) as u8;
                let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                let mut mesh = egui::Mesh::with_texture(tex_id);
                mesh.add_rect_with_uv(draw_rect, uv_rect, tint);
                painter.add(egui::Shape::mesh(mesh));
            }
        }
    }
}
