//! Cloud sprite system: drifting cloud instances and per-theme configurations.

use eframe::egui;

use crate::assets;
use crate::types::Vec2;

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
pub(super) struct CloudConfig {
    pub max_clouds: usize,
    pub velocity: f32,
    pub limits: f32,
    pub height: f32,
    pub far_plane: f32,
    pub sprites: &'static [CloudSpriteInfo],
}

pub(super) struct CloudSpriteInfo {
    pub name: &'static str,
    pub atlas: &'static str,
    pub scale_x: f32,
    pub scale_y: f32,
}

pub(super) const CLOUD_CONFIGS: &[(&str, CloudConfig)] = &[
    (
        "CloudPlateauSet",
        CloudConfig {
            max_clouds: 8,
            velocity: 0.2,
            limits: 93.24,
            height: 5.0,
            far_plane: 1.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_0",
                    atlas: "Background_Plateaus_Sheet_01.png",
                    scale_x: 3.0,
                    scale_y: 3.0,
                },
            ],
        },
    ),
    (
        "CloudJungleSet",
        CloudConfig {
            max_clouds: 8,
            velocity: 0.2,
            limits: 93.24,
            height: 10.0,
            far_plane: 1.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 0.87,
                    scale_y: 0.6525,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 1.0875,
                    scale_y: 0.87,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET",
                    atlas: "Background_Jungle_Sheet_01.png",
                    scale_x: 1.0,
                    scale_y: 0.8,
                },
            ],
        },
    ),
    (
        "CloudNightSet",
        CloudConfig {
            max_clouds: 5,
            velocity: 0.2,
            limits: 40.0,
            height: 2.5,
            far_plane: 1.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_1",
                    atlas: "Background_Night_Sheet_01.png",
                    scale_x: 2.0,
                    scale_y: 2.0,
                },
            ],
        },
    ),
    (
        "CloudHalloweenSet",
        CloudConfig {
            max_clouds: 5,
            velocity: 0.5,
            limits: 93.0,
            height: 4.0,
            far_plane: 1.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_01_SET_2",
                    atlas: "Background_Halloween_Sheet_01.png",
                    scale_x: 1.5,
                    scale_y: 1.5,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_2",
                    atlas: "Background_Halloween_Sheet_01.png",
                    scale_x: 1.2,
                    scale_y: 1.2,
                },
            ],
        },
    ),
    (
        "CloudLPASet",
        CloudConfig {
            max_clouds: 10,
            velocity: 0.1,
            limits: 250.0,
            height: 0.84,
            far_plane: 1.0,
            sprites: &[
                CloudSpriteInfo {
                    name: "Background_Cloud_02_SET_3",
                    atlas: "Background_Maya_Sheet_02.png",
                    scale_x: 1.0,
                    scale_y: 1.0,
                },
                CloudSpriteInfo {
                    name: "Background_Cloud_03_SET_2",
                    atlas: "Background_Maya_Sheet_02.png",
                    scale_x: 1.0,
                    scale_y: 1.0,
                },
            ],
        },
    ),
];

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
        if let Some(info) = crate::sprite_db::get_sprite_info(&cloud.sprite_name) {
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
