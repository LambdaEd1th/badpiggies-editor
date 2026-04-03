//! Background rendering — sky color, ground fill, parallax sprite layers.
//!
//! Draws the scene backdrop with up to 7 parallax layers per theme. Each layer
//! has a speed factor controlling how much it shifts relative to the camera.
//! Fill sprites are solid-color rectangles, atlas sprites use UV-mapped textures.

use std::collections::{HashMap, HashSet};

use eframe::egui;

use std::sync::Arc;

use crate::assets;
use crate::bg_data::{self, BgSprite};
use crate::types::Vec2;

use super::{Camera, DrawCtx, bg_shader};

/// Pre-computed tile/singleton data for background sprites. Built once per level
/// load to avoid recomputing 12 HashMaps + 4 sorts every frame.
pub struct BgLayerCache {
    /// Sprite index → (block_width, parallax_speed) for tiled sprite groups.
    tile_info: HashMap<usize, (f32, f32)>,
    /// Set of sprite indices that are fill-extended singletons.
    singleton_set: HashSet<usize>,
    /// Pre-lowercased sprite names (avoids per-frame String allocation).
    name_lower: Vec<String>,
    /// Effective sprites (with overrides applied), or None if using theme defaults.
    effective_sprites: Option<Vec<bg_data::BgSprite>>,
    /// Sprite indices sorted by worldZ descending (farthest first = back-to-front).
    sorted_indices: Vec<usize>,
}

impl BgLayerCache {
    /// Get the effective sprite slice (overrides or theme defaults).
    pub fn sprites<'a>(&'a self, theme: &'a bg_data::BgTheme) -> &'a [bg_data::BgSprite] {
        self.effective_sprites.as_deref().unwrap_or(&theme.sprites)
    }
}

/// Build the background layer cache. Call once at level load time.
pub fn build_bg_layer_cache(
    theme_name: &str,
    bg_override_text: Option<&str>,
) -> Option<BgLayerCache> {
    let theme = bg_data::get_theme(theme_name)?;

    let effective_sprites = if let Some(raw) = bg_override_text {
        let overrides = bg_data::parse_bg_overrides(raw);
        if !overrides.groups.is_empty() || !overrides.sprites.is_empty() {
            Some(bg_data::apply_bg_overrides(theme, &overrides))
        } else {
            None
        }
    } else {
        None
    };
    let sprites = effective_sprites.as_deref().unwrap_or(&theme.sprites);

    // Build tile bands: group non-fill, non-sky sprites by parent_group (or
    // fallback to atlas+round(y)+layer when no parent_group is set).
    // Using parent_group avoids splitting sprites that belong to the same
    // Unity repeating group but sit at slightly different Y positions.
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut name_lower: Vec<String> = Vec::with_capacity(sprites.len());
    for (i, sprite) in sprites.iter().enumerate() {
        let nl = sprite.name.to_ascii_lowercase();
        name_lower.push(nl);
        if sprite.fill_color.is_some() || sprite.sky_texture.is_some() {
            continue;
        }
        if name_lower[i].contains("fill") {
            continue;
        }
        let atlas_key = sprite.atlas.as_deref().unwrap_or("");
        let layer_key = sprite.layer.order();
        let y_key = sprite.world_y.round() as i32;
        let group_key = if !sprite.parent_group.is_empty() {
            format!(
                "g:{}:{}:{}:{}",
                sprite.parent_group, y_key, atlas_key, layer_key
            )
        } else {
            format!("y:{}:{}:{}", y_key, atlas_key, layer_key)
        };
        groups.entry(group_key).or_default().push(i);
    }

    let mut singleton_set: HashSet<usize> = HashSet::new();
    for indices in groups.values() {
        if indices.len() == 1 {
            let s = &sprites[indices[0]];
            let dw = s.sprite_w * WORLD_SCALE * 2.0 * s.scale_x.abs();
            if dw > 100.0 {
                singleton_set.insert(indices[0]);
            }
        }
    }

    let mut tile_info: HashMap<usize, (f32, f32)> = HashMap::new();
    for indices in groups.values() {
        if indices.len() < 2 {
            continue;
        }
        let mut sorted: Vec<usize> = indices.clone();
        sorted.sort_by(|a, b| {
            sprites[*a]
                .world_x
                .partial_cmp(&sprites[*b].world_x)
                .unwrap()
        });
        let min_x = sprites[sorted[0]].world_x;
        let max_x = sprites[*sorted.last().unwrap()].world_x;
        let avg_spacing = (max_x - min_x) / (sorted.len() as f32 - 1.0);
        if avg_spacing <= 0.1 {
            continue;
        }
        // Compute block_width from edge sprite display widths instead of
        // avg_spacing so tile copies butt up seamlessly at copy boundaries.
        let first = &sprites[sorted[0]];
        let last = &sprites[*sorted.last().unwrap()];
        let first_w = first.sprite_w * WORLD_SCALE * 2.0 * first.scale_x.abs();
        let last_w = last.sprite_w * WORLD_SCALE * 2.0 * last.scale_x.abs();
        let block_width = max_x - min_x + (first_w + last_w) / 2.0;
        let speed = sprites[sorted[0]].layer.parallax_speed();
        for &idx in &sorted {
            tile_info.insert(idx, (block_width, speed));
        }
    }

    Some(BgLayerCache {
        tile_info,
        singleton_set,
        name_lower,
        sorted_indices: {
            let s = effective_sprites.as_deref().unwrap_or(&theme.sprites);
            let mut idx: Vec<usize> = (0..s.len()).collect();
            idx.sort_by(|a, b| {
                s[*b]
                    .world_z
                    .partial_cmp(&s[*a].world_z)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            idx
        },
        effective_sprites,
    })
}

/// GPU state passed by the renderer for background sprites.
pub struct BgGpuState<'a> {
    pub resources: Arc<bg_shader::BgResources>,
    pub atlas_cache: &'a mut bg_shader::BgAtlasCache,
    pub device: &'a eframe::wgpu::Device,
    pub queue: &'a eframe::wgpu::Queue,
    pub slot_counter: &'a mut u32,
}

/// World-size formula: pixelSize * 10 / 768
const WORLD_SCALE: f32 = 10.0 / 768.0;

/// Draw the scene background: sky color fill + ground fill band.
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

    // Draw sprites in Z-sorted order (farthest first = back-to-front)
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

        // Cloud drift: horizontal offset based on time
        let name_lower = &cache.name_lower[i];
        let cloud_x = if name_lower.contains("cloud") {
            cloud_drift_offset(time, &sprite.layer)
        } else {
            0.0
        };
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
                let x_offset = copy as f32 * block_width + shift + cloud_x;
                draw_bg_sprite_offset(ctx, sprite, x_offset, anim_y, false, &mut gpu);
            }
        } else {
            let extend_fill_like = sprite.fill_color.is_some()
                || sprite.sky_texture.is_some()
                || name_lower.contains("fill")
                || cache.singleton_set.contains(&i);
            draw_bg_sprite_offset(ctx, sprite, cloud_x, anim_y, extend_fill_like, &mut gpu);
        }
    }
}

// ── Animation helpers ────────────────────────────────────────────────────

/// Hermite spline evaluation (Unity AnimationCurve equivalent).
pub(super) fn hermite(keys: &[(f32, f32, f32, f32)], time: f32) -> f32 {
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

/// Foam Y offset (6-second hermite loop, rest at 0.774923).
fn foam_offset(time: f64) -> f32 {
    const KEYS: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.774923, 0.0, 0.0),
        (0.016667, 0.774923, 0.0, 0.0),
        (2.466667, 1.796472, 0.332467, 0.332467),
        (3.0, 1.893086, 0.006075, 0.002644),
        (3.7, 1.739951, -0.399446, -0.399446),
        (6.0, 0.774923, 0.0, 0.0),
    ];
    const REST: f32 = 0.774923;
    let t = (time % 6.0) as f32;
    hermite(KEYS, t) - REST
}

/// Cloud horizontal drift offset based on layer speed.
fn cloud_drift_offset(time: f64, layer: &bg_data::BgLayer) -> f32 {
    let velocity = match layer {
        bg_data::BgLayer::Sky => 0.1,
        bg_data::BgLayer::Far | bg_data::BgLayer::Further => 0.2,
        bg_data::BgLayer::Near => 0.3,
        _ => 0.15,
    };
    (time * velocity) as f32
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
    // Extend downward to screen bottom so fills cover the full depth,
    // matching Unity's coverage.
    if let Some(rgb) = sprite.fill_color {
        let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        let fill_rect = egui::Rect::from_min_max(
            screen_rect.left_top(),
            egui::pos2(screen_rect.right(), rect.bottom()),
        );
        painter.rect_filled(fill_rect, 0.0, color);
        return;
    }

    // Cutoff: controls shader blend mode.
    //   -1.0 → opaque (Unity _Custom/Unlit_Color_Geometry — fill layers)
    //    0.5 → alpha cutout (Unity Unlit/Transparent Cutout, _Cutoff=0.5)
    let name_lower_ref = sprite.name.to_ascii_lowercase();
    let cutoff = if name_lower_ref.contains("fill") {
        -1.0
    } else {
        0.5
    };

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
                tint_color: [1.0, 1.0, 1.0, 1.0],
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
                tint_color: [1.0, 1.0, 1.0, 1.0],
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
            let mut mesh = egui::Mesh::with_texture(tex_id);
            mesh.add_rect_with_uv(
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
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
