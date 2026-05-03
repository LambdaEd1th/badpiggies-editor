//! Drawing functions for background sprites and layers.

use eframe::egui;

use crate::data::assets;
use crate::data::bg_data::{self, BgSprite};
use crate::domain::types::Vec2;

use super::super::{Camera, DrawCtx, bg_shader};
use super::cache::{BgGpuState, BgLayerCache, WORLD_SCALE, bg_sprite_x_animation_offset};

pub fn draw_background(
    painter: &egui::Painter,
    rect: egui::Rect,
    camera: &Camera,
    canvas_center: egui::Vec2,
    theme: Option<&str>,
) {
    // Sky color fill
    let sky_color = theme
        .map(assets::sky_top_color)
        .unwrap_or(egui::Color32::from_rgb(0x28, 0x2c, 0x34));
    painter.rect_filled(rect, 0.0, sky_color);

    // Ground fill band below a certain Y
    let ground_y = -6.0; // approximately where the ground plane is
    let ground_screen = camera.world_to_screen(
        Vec2 {
            x: 0.0,
            y: ground_y,
        },
        canvas_center,
    );

    if ground_screen.y < rect.bottom() {
        let ground_color = theme
            .map(assets::ground_color)
            .unwrap_or(egui::Color32::from_rgb(0x22, 0x44, 0x44));
        let ground_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), ground_screen.y),
            rect.right_bottom(),
        );
        painter.rect_filled(ground_rect, 0.0, ground_color);
    }
}

/// Draw parallax background sprite layers for the given theme.
///
/// `z_range` filters sprites by worldZ (real Unity world Z coordinate):
/// only sprites with `z_range.0 <= worldZ < z_range.1` are drawn.
/// Sprites are rendered in sorted order (farthest / highest worldZ first).
pub fn draw_bg_layers(
    ctx: &DrawCtx<'_>,
    theme_name: &str,
    time: f64,
    z_range: (f32, f32), // (inclusive min, exclusive max)
    cache: &BgLayerCache,
    mut gpu: Option<&mut BgGpuState<'_>>,
) {
    let rect = ctx.canvas_rect;
    let camera = ctx.camera;
    let theme = match bg_data::get_theme(theme_name) {
        Some(t) => t,
        None => return,
    };

    let sprites = cache.sprites(theme);

    // Draw sprites in Z-sorted order (farthest first = back-to-front).
    // Pre-compute wave/foam Y offsets
    let wave_y_offset = wave_offset(time);
    let foam_y_offset = foam_offset(time);

    for &i in &cache.sorted_indices {
        let sprite = &sprites[i];
        // World-Z range filter
        let z = sprite.world_z;
        if z < z_range.0 || z >= z_range.1 {
            continue;
        }

        let name_lower = &cache.name_lower[i];
        let anim_x = bg_sprite_x_animation_offset(name_lower, time, &sprite.layer);
        // Wave/foam Y offset (Dummy is in OceanAnimRoot, same as Waves)
        let anim_y = if name_lower == "waves" || name_lower == "dummy" {
            wave_y_offset
        } else if name_lower == "foam" {
            foam_y_offset
        } else {
            0.0
        };

        if let Some(&(block_width, speed)) = cache.tile_info.get(&i) {
            let apparent_x = camera.center.x * (1.0 - speed);
            let shift = (apparent_x / block_width).round() * block_width;
            // Dynamic copy count: enough to cover the viewport at any zoom
            let viewport_w = rect.width() / camera.zoom;
            let n = (viewport_w / block_width).ceil() as i32 + 1;
            for copy in -n..=n {
                let x_offset = copy as f32 * block_width + shift + anim_x;
                draw_bg_sprite_offset(ctx, sprite, x_offset, anim_y, false, &mut gpu);
            }
        } else {
            let extend_fill_like = sprite.fill_color.is_some()
                || sprite.sky_texture.is_some()
                || name_lower.contains("fill")
                || cache.singleton_set.contains(&i);
            draw_bg_sprite_offset(ctx, sprite, anim_x, anim_y, extend_fill_like, &mut gpu);
        }
    }
}

// ── Animation helpers ────────────────────────────────────────────────────

/// Hermite spline evaluation (Unity AnimationCurve equivalent).
pub(in crate::renderer) fn hermite(keys: &[(f32, f32, f32, f32)], time: f32) -> f32 {
    let n = keys.len();
    if n == 0 {
        return 0.0;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }
    if time >= keys[n - 1].0 {
        return keys[n - 1].1;
    }
    let mut i = 0;
    while i < n - 2 && keys[i + 1].0 < time {
        i += 1;
    }
    let (t0, v0, _, out_s) = keys[i];
    let (t1, v1, in_s, _) = keys[i + 1];
    let dt = t1 - t0;
    let s = (time - t0) / dt;
    let s2 = s * s;
    let s3 = s2 * s;
    (2.0 * s3 - 3.0 * s2 + 1.0) * v0
        + (s3 - 2.0 * s2 + s) * (out_s * dt)
        + (-2.0 * s3 + 3.0 * s2) * v1
        + (s3 - s2) * (in_s * dt)
}

/// Wave Y offset (6-second hermite loop).
fn wave_offset(time: f64) -> f32 {
    // (t, value, inSlope, outSlope)
    const KEYS: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, -0.006681, -0.006681),
        (2.366667, 1.0, 0.001194, 0.001194),
        (3.7, 0.6954712, -0.383697, -0.383697),
        (6.0, 0.0, 0.0, 0.0),
    ];
    let t = (time % 6.0) as f32;
    hermite(KEYS, t)
}

/// Foam Y offset (6-second hermite loop).
///
/// The animation curve stores absolute `localPosition.y` values for
/// `FoamAnimRoot`.  At rest the animation sets Y = 0.774923, but the
/// prefab default that bg-data.toml baked into `worldY` used the
/// *prefab* value 1.146046.  We subtract the prefab Y so the returned
/// delta is relative to the baked position.
fn foam_offset(time: f64) -> f32 {
    const KEYS: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.774923, 0.0, 0.0),
        (0.016667, 0.774923, 0.0, 0.0),
        (2.466667, 1.796472, 0.332467, 0.332467),
        (3.0, 1.893086, 0.006075, 0.002644),
        (3.7, 1.739951, -0.399446, -0.399446),
        (6.0, 0.774923, 0.0, 0.0),
    ];
    // FoamAnimRoot prefab localY (baked into bg-data.toml worldY).
    const PREFAB_Y: f32 = 1.146046;
    let t = (time % 6.0) as f32;
    hermite(KEYS, t) - PREFAB_Y
}

fn draw_bg_sprite_offset(
    ctx: &DrawCtx,
    sprite: &BgSprite,
    x_offset: f32,
    y_offset: f32,
    extend_fill_like: bool,
    gpu: &mut Option<&mut BgGpuState<'_>>,
) {
    let painter = ctx.painter;
    let rect = ctx.canvas_rect;
    let camera = ctx.camera;
    let canvas_center = ctx.canvas_center;
    let tex_cache = ctx.tex_cache;
    let speed = sprite.layer.parallax_speed();

    // Match TS renderer: parent parallax group is positioned at camX * speed.
    // World→screen transform then naturally yields apparent shift of camX * (1 - speed).
    let world_x = sprite.world_x + camera.center.x * speed + x_offset;

    // Foreground vertical parallax: follows camera Y at 0.2× rate, clamped ≤ 0
    let fg_y_offset = if sprite.layer == bg_data::BgLayer::Foreground {
        (camera.center.y * 0.2).min(0.0)
    } else {
        0.0
    };
    let world_y = sprite.world_y + y_offset + fg_y_offset;

    // Compute display size in world units
    let half_ext_x = sprite.sprite_w * WORLD_SCALE;
    let half_ext_y = sprite.sprite_h * WORLD_SCALE;
    let orig_display_w = half_ext_x * 2.0 * sprite.scale_x.abs();
    let display_h = half_ext_y * 2.0 * sprite.scale_y.abs();

    // Dynamically compute the exact width needed to cover the entire viewport
    // from this sprite's center, regardless of camera position.
    // No magic constant — always exactly wide enough + 1 world-unit margin.
    let display_w = if extend_fill_like {
        let viewport_half_w = rect.width() / (2.0 * camera.zoom);
        let offset_from_cam = (world_x - camera.center.x).abs();
        let needed_half_w = offset_from_cam + viewport_half_w + 1.0;
        orig_display_w.max(needed_half_w * 2.0)
    } else {
        orig_display_w
    };

    // content_ratio_x: fraction of the extended quad that contains actual texture.
    // The GPU shader clamps UV at the original content edges so extended portions
    // repeat edge pixels instead of stretching the atlas texture.
    // For sky textures, stretching IS intended, so ratio stays 1.0.
    let content_ratio_x = if sprite.sky_texture.is_some() {
        1.0
    } else {
        orig_display_w / display_w
    };

    let center = camera.world_to_screen(
        Vec2 {
            x: world_x,
            y: world_y,
        },
        canvas_center,
    );

    // Quick frustum cull
    let hw_screen = display_w * 0.5 * camera.zoom;
    let hh_screen = display_h * 0.5 * camera.zoom;
    if center.x + hw_screen < rect.left() - 50.0
        || center.x - hw_screen > rect.right() + 50.0
        || center.y + hh_screen < rect.top() - 50.0
        || center.y - hh_screen > rect.bottom() + 50.0
    {
        return;
    }

    let screen_rect =
        egui::Rect::from_center_size(center, egui::vec2(hw_screen * 2.0, hh_screen * 2.0));

    // Fill color sprites (solid rectangles) — no shader needed.
    // In Unity these use _Custom/Unlit_Color_Geometry (Queue=Transparent,
    // ZWrite Off, no blend = opaque).  They render at their natural size
    // and position, painting a solid fill color ON TOP of the farther hill
    // sprites (fill Z is slightly lower = closer to camera).
    if let Some(rgb) = sprite.fill_color {
        let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        painter.rect_filled(screen_rect, 0.0, color);
        return;
    }

    // Cutoff: controls shader blend mode.
    //   -1.0 → opaque (Unity _Custom/Unlit_Color_Geometry — solid fill layers)
    //    0.5 → alpha cutout (Unity Unlit/Transparent Cutout, _Cutoff=0.5)
    //    0.004 → alpha blend (nearly transparent sprites like clouds)
    // Solid fills (fillColor) already returned above; atlas-based sprites use cutout.
    let cutoff = if sprite.alpha_blend { 0.004 } else { 0.5 };

    // ── GPU path: use WGSL background shader ──
    if let Some(g) = gpu
        && *g.slot_counter < bg_shader::max_draw_slots()
    {
        // Sky texture
        if let Some(ref sky_name) = sprite.sky_texture
            && let Some(atlas) =
                g.atlas_cache
                    .get_or_load(g.device, g.queue, &g.resources, sky_name)
        {
            let uniforms = bg_shader::BgUniforms {
                screen_size: [rect.width(), rect.height()],
                camera_center: [camera.center.x, camera.center.y],
                zoom: camera.zoom,
                cutoff: 0.004,
                world_center: [world_x, world_y],
                world_size: [display_w, display_h],
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
                content_ratio_x,
                _pad: 0.0,
                tint_color: sprite.tint,
            };
            let slot = *g.slot_counter;
            *g.slot_counter += 1;
            painter.add(bg_shader::make_bg_callback(
                rect,
                g.resources.clone(),
                atlas,
                slot,
                uniforms,
            ));
            return;
        }

        // Atlas sprite
        if let Some(ref atlas_name) = sprite.atlas
            && let Some(atlas) =
                g.atlas_cache
                    .get_or_load(g.device, g.queue, &g.resources, atlas_name)
        {
            let cell = 1.0 / sprite.subdiv;
            // When border > 0, border_off alone maps to the exact content
            // boundary; the border pixels duplicate edge content for safe
            // bilinear filtering.  Extra padding would skip actual content
            // and create visible seams between adjacent tiled sprites.
            let padding = if sprite.border > 0.0 {
                0.0
            } else {
                1.0 / 2048.0
            };
            let border_off = sprite.border / 1024.0;
            let u0 = sprite.uv_x * cell + padding + border_off;
            let u1 = (sprite.uv_x + sprite.grid_w) * cell - padding - border_off;
            let v0_unity = sprite.uv_y * cell + padding + border_off;
            let v1_unity = (sprite.uv_y + sprite.grid_h) * cell - padding - border_off;
            // UV Y flip: Unity V=0 at bottom, wgpu V=0 at top
            let v0 = 1.0 - v1_unity;
            let v1 = 1.0 - v0_unity;

            // Handle flipping via UV swap
            let (u0, u1) = if sprite.scale_x < 0.0 {
                (u1, u0)
            } else {
                (u0, u1)
            };
            let (v0, v1) = if sprite.scale_y < 0.0 {
                (v1, v0)
            } else {
                (v0, v1)
            };

            let uniforms = bg_shader::BgUniforms {
                screen_size: [rect.width(), rect.height()],
                camera_center: [camera.center.x, camera.center.y],
                zoom: camera.zoom,
                cutoff,
                world_center: [world_x, world_y],
                world_size: [display_w, display_h],
                uv_min: [u0, v0],
                uv_max: [u1, v1],
                content_ratio_x,
                _pad: 0.0,
                tint_color: sprite.tint,
            };
            let slot = *g.slot_counter;
            *g.slot_counter += 1;
            painter.add(bg_shader::make_bg_callback(
                rect,
                g.resources.clone(),
                atlas,
                slot,
                uniforms,
            ));
            return;
        }
    }

    // ── CPU fallback: egui mesh ──

    // Sky texture
    if let Some(ref sky_name) = sprite.sky_texture {
        if let Some(tex_id) = tex_cache.get(sky_name) {
            let tint = egui::Color32::from_rgba_unmultiplied(
                (sprite.tint[0] * 255.0) as u8,
                (sprite.tint[1] * 255.0) as u8,
                (sprite.tint[2] * 255.0) as u8,
                (sprite.tint[3] * 255.0) as u8,
            );
            let mut mesh = egui::Mesh::with_texture(tex_id);
            mesh.add_rect_with_uv(
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
            painter.add(egui::Shape::mesh(mesh));
        }
        return;
    }

    // Atlas sprite with UV mapping
    if let Some(ref atlas_name) = sprite.atlas {
        let tex_id = match tex_cache.get(atlas_name) {
            Some(id) => id,
            None => return,
        };

        let cell = 1.0 / sprite.subdiv;
        let padding = 1.0 / 2048.0;
        let border_off = sprite.border / 1024.0;
        let u0 = sprite.uv_x * cell + padding + border_off;
        let u1 = (sprite.uv_x + sprite.grid_w) * cell - padding - border_off;
        let v0_unity = sprite.uv_y * cell + padding + border_off;
        let v1_unity = (sprite.uv_y + sprite.grid_h) * cell - padding - border_off;
        // UV Y flip: Unity V=0 at bottom, egui V=0 at top
        let v0 = 1.0 - v1_unity;
        let v1 = 1.0 - v0_unity;

        // Handle flipping via UV swap
        let (u0, u1) = if sprite.scale_x < 0.0 {
            (u1, u0)
        } else {
            (u0, u1)
        };
        let (v0, v1) = if sprite.scale_y < 0.0 {
            (v1, v0)
        } else {
            (v0, v1)
        };

        let uv_rect = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
        // CPU fallback: use original (non-extended) width to avoid UV stretching.
        // GPU path handles extension via content_ratio_x UV clamping in shader.
        let cpu_hw = orig_display_w * 0.5 * camera.zoom;
        let cpu_rect =
            egui::Rect::from_center_size(center, egui::vec2(cpu_hw * 2.0, hh_screen * 2.0));
        let mut mesh = egui::Mesh::with_texture(tex_id);
        mesh.add_rect_with_uv(cpu_rect, uv_rect, egui::Color32::WHITE);
        painter.add(egui::Shape::mesh(mesh));
    }
}
