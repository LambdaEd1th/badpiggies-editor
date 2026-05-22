//! Drawing functions for background sprites and layers.

use eframe::egui;

use crate::data::bg_data::{self, BgSprite};
use crate::data::{assets, unity_anim};
use crate::domain::types::Vec2;

use super::super::{Camera, DrawCtx, bg_shader};
use super::cache::{BgGpuState, BgLayerCache, WORLD_SCALE, bg_sprite_x_animation_offset};

const DARK_OVERLAY_MAX_RENDER_QUEUE: i32 = 3005;

pub fn draw_background(
    painter: &egui::Painter,
    rect: egui::Rect,
    camera: &Camera,
    canvas_center: egui::Vec2,
    theme: Option<&str>,
) {
    let sky_color = background_base_color(theme);
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

pub(super) fn background_base_color(theme: Option<&str>) -> egui::Color32 {
    match theme {
        Some("Cave") => assets::ground_color("Cave"),
        Some(theme_name) => assets::sky_top_color(theme_name),
        None => egui::Color32::from_rgb(0x28, 0x2c, 0x34),
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
    draw_after_dark_overlay: Option<bool>,
) {
    let theme = match bg_data::get_theme(theme_name) {
        Some(t) => t,
        None => return,
    };

    let sprites = cache.sprites(theme);

    // Draw sprites in authored layer order, then back-to-front within each
    // layer. Z-range filtering still uses the authored world Z so fill sort
    // overrides cannot move sprites into a different background pass.
    for &i in &cache.sorted_indices {
        let z = sprites[i].world_z;
        if z < z_range.0 || z >= z_range.1 {
            continue;
        }
        if let Some(after_dark_overlay) = draw_after_dark_overlay {
            let sprite_after_dark_overlay = sprites[i]
                .custom_render_queue
                .is_some_and(|queue| queue > DARK_OVERLAY_MAX_RENDER_QUEUE);
            if sprite_after_dark_overlay != after_dark_overlay {
                continue;
            }
        }

        draw_bg_sprite(ctx, time, sprites, i, cache, &mut gpu);
    }
}

pub(in crate::renderer) fn should_extend_fill_like(
    sprite: &BgSprite,
    name_lower: &str,
    cache: &BgLayerCache,
    sprite_index: usize,
) -> bool {
    sprite.fill_color.is_some()
        || sprite.sky_texture.is_some()
        || (sprite.fill_color.is_none() && name_lower.contains("fill"))
        || cache.singleton_set.contains(&sprite_index)
}

pub(in crate::renderer) fn content_ratio_x_for_bg_sprite(
    sprite: &BgSprite,
    extend_fill_like: bool,
    orig_display_w: f32,
    display_w: f32,
) -> f32 {
    if sprite.sky_texture.is_some() || extend_fill_like {
        1.0
    } else {
        orig_display_w / display_w
    }
}

pub(in crate::renderer) fn draw_bg_sprite(
    ctx: &DrawCtx<'_>,
    time: f64,
    sprites: &[BgSprite],
    sprite_index: usize,
    cache: &BgLayerCache,
    gpu: &mut Option<&mut BgGpuState<'_>>,
) {
    let Some(sprite) = sprites.get(sprite_index) else {
        return;
    };

    let rect = ctx.canvas_rect;
    let camera = ctx.camera;
    let wave_y_offset = wave_offset(time);
    let foam_y_offset = foam_offset(time);

    let name_lower = &cache.name_lower[sprite_index];
    let anim_x = bg_sprite_x_animation_offset(name_lower, time, &sprite.layer);
    // Wave/foam Y offset (Dummy is in OceanAnimRoot, same as Waves)
    let anim_y = if name_lower == "waves" || name_lower == "dummy" {
        wave_y_offset
    } else if name_lower == "foam" {
        foam_y_offset
    } else {
        0.0
    };

    if let Some(&(block_width, speed)) = cache.tile_info.get(&sprite_index) {
        let phase = cache.tile_phase.get(&sprite_index).copied().unwrap_or(0.0);
        let apparent_x = camera.center.x * (1.0 - speed);
        let shift = ((apparent_x - phase) / block_width).round() * block_width + phase;
        // Dynamic copy count: enough to cover the viewport at any zoom
        let viewport_w = rect.width() / camera.zoom;
        let n = (viewport_w / block_width).ceil() as i32 + 1;
        for copy in -n..=n {
            let x_offset = copy as f32 * block_width + shift + anim_x;
            draw_bg_sprite_offset(ctx, sprite, x_offset, anim_y, false, None, gpu);
        }
    } else {
        draw_bg_sprite_offset(
            ctx,
            sprite,
            anim_x,
            anim_y,
            should_extend_fill_like(sprite, name_lower, cache, sprite_index),
            cache.fill_top_world_y.get(&sprite_index).copied(),
            gpu,
        );
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
    sample_required_root_position_y_curve(
        time,
        unity_anim::ocean_animation_clip(),
        "OceanAnimation.anim",
    )
}

/// Foam Y offset (6-second hermite loop).
///
/// The animation curve stores absolute `localPosition.y` values for
/// `FoamAnimRoot`.  At rest the animation sets Y = 0.774923, but the
/// prefab default that bg-data.toml baked into `worldY` used the
/// *prefab* value 1.146046.  We subtract the prefab Y so the returned
/// delta is relative to the baked position.
fn foam_offset(time: f64) -> f32 {
    // FoamAnimRoot prefab localY (baked into bg-data.toml worldY).
    const PREFAB_Y: f32 = 1.146046;
    sample_required_root_position_y_curve(
        time,
        unity_anim::ocean_foam_animation_clip(),
        "OceanFoamAnimation.anim",
    ) - PREFAB_Y
}

fn sample_required_root_position_y_curve(
    time: f64,
    clip: Option<&unity_anim::UnityAnimationClip>,
    asset_name: &str,
) -> f32 {
    let clip = clip.unwrap_or_else(|| panic!("{} should load from embedded assets", asset_name));
    let curve = clip
        .root_position()
        .unwrap_or_else(|| panic!("{} must include root position curves", asset_name))
        .y
        .as_slice();
    if curve.is_empty() {
        panic!("{} root position Y curve must not be empty", asset_name);
    }
    hermite(curve, clip.sample_time(time, 0.0))
}

fn atlas_uv_padding(sprite: &BgSprite) -> (f32, f32, f32) {
    let default_padding = if sprite.border > 0.0 {
        0.0
    } else {
        1.0 / 2048.0
    };

    let extra_right_padding: f32 = if sprite.parent_group == "BGLayerFurther"
        && sprite.name == "Background_Maya_High_Further_02"
    {
        24.0 / 2048.0
    } else {
        default_padding
    };

    (
        default_padding,
        extra_right_padding.max(default_padding),
        default_padding,
    )
}

fn draw_bg_sprite_offset(
    ctx: &DrawCtx,
    sprite: &BgSprite,
    x_offset: f32,
    y_offset: f32,
    extend_fill_like: bool,
    fill_top_world_y: Option<f32>,
    gpu: &mut Option<&mut BgGpuState<'_>>,
) {
    let painter = ctx.painter;
    let rect = ctx.canvas_rect;
    let camera = ctx.camera;
    let canvas_center = ctx.canvas_center;
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

    // Old TS/Pixi behavior stretches atlas-based fill-like sprites and large
    // singleton backdrops when they are widened to cover the viewport.
    let content_ratio_x =
        content_ratio_x_for_bg_sprite(sprite, extend_fill_like, orig_display_w, display_w);

    let center = camera.world_to_screen(
        Vec2 {
            x: world_x,
            y: world_y,
        },
        canvas_center,
    );

    let (draw_world_y, draw_display_h, center, hh_screen) = if sprite.fill_color.is_some() {
        let bottom_world_y = world_y - display_h * 0.5;
        let top_world_y = fill_top_world_y
            .unwrap_or(world_y + display_h * 0.5)
            .max(bottom_world_y);
        let fill_center = camera.world_to_screen(
            Vec2 {
                x: world_x,
                y: (bottom_world_y + top_world_y) * 0.5,
            },
            canvas_center,
        );
        let hh_screen = (top_world_y - bottom_world_y) * 0.5 * camera.zoom;
        (
            (bottom_world_y + top_world_y) * 0.5,
            top_world_y - bottom_world_y,
            fill_center,
            hh_screen,
        )
    } else {
        (world_y, display_h, center, display_h * 0.5 * camera.zoom)
    };

    let hw_screen = display_w * 0.5 * camera.zoom;
    if center.x + hw_screen < rect.left() - 50.0
        || center.x - hw_screen > rect.right() + 50.0
        || center.y + hh_screen < rect.top() - 50.0
        || center.y - hh_screen > rect.bottom() + 50.0
    {
        return;
    }

    // ── GPU path: use WGSL background shader ──
    if let Some(g) = gpu
        && *g.slot_counter < bg_shader::max_draw_slots()
        && let Some(atlas_key) = sprite
            .fill_color
            .map(|_| bg_shader::WHITE_TEXTURE_KEY)
            .or(sprite.sky_texture.as_deref())
            .or(sprite.atlas.as_deref())
        && let Some(atlas) = g
            .atlas_cache
            .get_or_load(g.device, g.queue, &g.resources, atlas_key)
    {
        let (uv_min, uv_max) = if sprite.fill_color.is_some() || sprite.sky_texture.is_some() {
            ([0.0, 0.0], [1.0, 1.0])
        } else {
            let cell = 1.0 / sprite.subdiv;
            // When border > 0, border_off alone maps to the exact content
            // boundary; the border pixels duplicate edge content for safe
            // bilinear filtering.  MayaHigh's Further_02 art still leaves a
            // visible vertical column on its right edge, so trim that side
            // more aggressively than the left.
            let (padding_left_x, padding_right_x, padding_y) = atlas_uv_padding(sprite);
            let border_off = sprite.border / 1024.0;
            let u0 = sprite.uv_x * cell + padding_left_x + border_off;
            let u1 = (sprite.uv_x + sprite.grid_w) * cell - padding_right_x - border_off;
            let v0_unity = sprite.uv_y * cell + padding_y + border_off;
            let v1_unity = (sprite.uv_y + sprite.grid_h) * cell - padding_y - border_off;
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

            ([u0, v0], [u1, v1])
        };

        let uniforms = bg_shader::BgUniforms {
            screen_size: [rect.width(), rect.height()],
            camera_center: [camera.center.x, camera.center.y],
            zoom: camera.zoom,
            cutoff: sprite.cutoff,
            world_center: [world_x, draw_world_y],
            world_size: [display_w, draw_display_h],
            uv_min,
            uv_max,
            content_ratio_x,
            _pad0: 0.0,
            main_tex_st: sprite.main_tex_st,
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
            sprite.shader_kind,
        ));
    }
}
