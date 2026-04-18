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

use super::{Camera, DrawCtx, LevelRenderer, bg_shader, clouds};

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

fn sprite_display_width(sprite: &bg_data::BgSprite) -> f32 {
    sprite.sprite_w * WORLD_SCALE * 2.0 * sprite.scale_x.abs()
}

fn tile_block_width(sorted: &[usize], sprites: &[bg_data::BgSprite]) -> Option<f32> {
    // Use edge-to-edge bounding for the wrap gap: the gap between the last
    // sprite's right edge and the first sprite's left edge in the next copy
    // should equal the median edge gap between adjacent sprites.  This is
    // correct regardless of whether sprites have uniform or varying display
    // widths (e.g. BGLayerNear's first sprite has a smaller scale than the
    // rest; a centre-to-centre formula would produce a 1-world-unit gap at
    // the seam, while edge-based matches the ~0-pixel internal overlap).
    let mut edge_gaps: Vec<f32> = sorted
        .windows(2)
        .map(|pair| {
            let a = &sprites[pair[0]];
            let b = &sprites[pair[1]];
            let a_right = a.world_x + sprite_display_width(a) * 0.5;
            let b_left = b.world_x - sprite_display_width(b) * 0.5;
            b_left - a_right
        })
        .collect();
    if edge_gaps.is_empty() {
        return None;
    }
    edge_gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_edge_gap = edge_gaps[edge_gaps.len() / 2];
    let first = &sprites[sorted[0]];
    let last = &sprites[*sorted.last().unwrap()];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    Some(max_right - min_left + median_edge_gap)
}

fn tile_group_key(
    sprite: &bg_data::BgSprite,
    name_lower: &str,
    name_count: usize,
) -> Option<String> {
    if sprite.fill_color.is_some() || sprite.sky_texture.is_some() || name_lower.contains("fill") {
        return None;
    }

    let layer_key = sprite.layer.order();
    if !sprite.parent_group.is_empty() {
        if name_count <= 1 {
            Some(format!("g:{}:{}", sprite.parent_group, layer_key))
        } else {
            // Use 0.5-unit Z granularity: multiply by 2 before rounding.
            // Plain z.round() collapses Z=5.5 and Z=6.0 to the same key (both →
            // 6), merging e.g. Lamp_01 and Background_Plateau_02 in Halloween
            // BGLayerNear and producing the wrong block_width.  Sprites that
            // belong to a single tiling strip (like Morning BGLayerForeground's
            // 14 uniquely-named trees, all at Z=−8.94) still share one key.
            let z_key = (sprite.world_z * 2.0).round() as i32;
            Some(format!("g:{}:{}:{}", sprite.parent_group, layer_key, z_key))
        }
    } else {
        let atlas_key = sprite.atlas.as_deref().unwrap_or("");
        let y_key = sprite.world_y.round() as i32;
        Some(format!("y:{}:{}:{}", y_key, atlas_key, layer_key))
    }
}

fn bg_sprite_x_animation_offset(_name_lower: &str, _time: f64, _layer: &bg_data::BgLayer) -> f32 {
    // Unity's background prefab cloud strips are static parallax sprites.
    // Only CloudSet instances animate horizontally at runtime.
    0.0
}

/// Build the background layer cache. Call once at level load time.
pub fn build_bg_layer_cache(
    theme_name: &str,
    bg_override_text: Option<&str>,
) -> Option<BgLayerCache> {
    let theme = bg_data::get_theme(theme_name)?;

    let effective_sprites = if let Some(raw) = bg_override_text {
        // Try Transform-based overrides first (EP1-5 style)
        let overrides = bg_data::parse_bg_overrides(raw);
        if !overrides.groups.is_empty() || !overrides.sprites.is_empty() {
            Some(bg_data::apply_bg_overrides(theme, &overrides))
        } else if !theme.child_order.is_empty() {
            // Try PositionSerializer-based overrides (EP6 style)
            let overrides = bg_data::parse_position_serializer_overrides(raw, &theme.child_order);
            if !overrides.groups.is_empty() {
                Some(bg_data::apply_bg_overrides(theme, &overrides))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let sprites = effective_sprites.as_deref().unwrap_or(&theme.sprites);

    // Build tile bands: group non-fill, non-sky sprites by parent_group (or
    // fallback to atlas+round(y)+layer when no parent_group is set).
    // When parent_group is set, we group by parent_group+layer only — NO Y or
    // atlas in the key — so that all sprites in the same Unity repeating group
    // share one block_width and tile as a coherent unit (e.g. MayaTemple
    // vertical block columns must tile at the same period as the base strips).
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut parent_group_names: HashMap<(String, i32), HashSet<String>> = HashMap::new();
    let mut name_lower: Vec<String> = Vec::with_capacity(sprites.len());
    for (i, sprite) in sprites.iter().enumerate() {
        let nl = sprite.name.to_ascii_lowercase();
        name_lower.push(nl);
        if !sprite.parent_group.is_empty()
            && sprite.fill_color.is_none()
            && sprite.sky_texture.is_none()
        {
            parent_group_names
                .entry((sprite.parent_group.clone(), sprite.layer.order()))
                .or_default()
                .insert(name_lower[i].clone());
        }
    }

    for (i, sprite) in sprites.iter().enumerate() {
        if sprite.fill_color.is_some() || sprite.sky_texture.is_some() {
            continue;
        }
        if name_lower[i].contains("fill") {
            continue;
        }
        let name_count = if sprite.parent_group.is_empty() {
            0
        } else {
            parent_group_names
                .get(&(sprite.parent_group.clone(), sprite.layer.order()))
                .map(HashSet::len)
                .unwrap_or(0)
        };
        let Some(group_key) = tile_group_key(sprite, &name_lower[i], name_count) else {
            continue;
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
        let Some(block_width) = tile_block_width(&sorted, sprites) else {
            continue;
        };
        let speed = sprites[sorted[0]].layer.parallax_speed();
        for &idx in &sorted {
            tile_info.insert(idx, (block_width, speed));
        }
    }

    // Sort sprites by worldZ descending (farthest first = back-to-front),
    // matching Unity's Transparent queue rendering order.
    // Fill sprites (e.g. z=9.9) have slightly lower Z than their companion
    // hills (e.g. z=10.0), so they naturally render AFTER (on top of) the
    // hills — covering the lower portion while hilltops remain visible.
    let s = effective_sprites.as_deref().unwrap_or(&theme.sprites);
    let mut idx: Vec<usize> = (0..s.len()).collect();
    idx.sort_by(|a, b| {
        s[*b]
            .world_z
            .partial_cmp(&s[*a].world_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Some(BgLayerCache {
        tile_info,
        singleton_set,
        name_lower,
        sorted_indices: idx,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn median(values: &mut [f32]) -> f32 {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values[values.len() / 2]
    }

    #[test]
    fn jungle_far_tiles_share_one_period_across_z() {
        let cache = build_bg_layer_cache("Jungle", None).expect("jungle cache");
        let theme = bg_data::get_theme("Jungle").expect("jungle theme");
        let sprites = cache.sprites(theme);

        let far_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, sprite)| {
                sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02"
            })
            .map(|(idx, _)| idx)
            .collect();

        assert!(
            far_indices.len() > 4,
            "expected far hill sprites in jungle theme"
        );

        let first_width = cache
            .tile_info
            .get(&far_indices[0])
            .map(|(width, _)| *width)
            .expect("first far hill should tile");

        for idx in far_indices {
            let width = cache
                .tile_info
                .get(&idx)
                .map(|(block_width, _)| *block_width)
                .expect("every far hill should tile");
            assert!(
                (width - first_width).abs() < 0.001,
                "expected shared block width, got {width} vs {first_width}"
            );
        }
    }

    #[test]
    fn jungle_far_wrap_gap_matches_internal_spacing() {
        let cache = build_bg_layer_cache("Jungle", None).expect("jungle cache");
        let theme = bg_data::get_theme("Jungle").expect("jungle theme");
        let sprites = cache.sprites(theme);

        let mut far_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, sprite)| {
                sprite.parent_group == "BGLayerFar" && sprite.name == "Background_Jungle_02"
            })
            .map(|(idx, _)| idx)
            .collect();
        far_indices.sort_by(|a, b| {
            sprites[*a]
                .world_x
                .partial_cmp(&sprites[*b].world_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let min_x = sprites[*far_indices.first().expect("first")].world_x;
        let max_x = sprites[*far_indices.last().expect("last")].world_x;
        let mut diffs: Vec<f32> = far_indices
            .windows(2)
            .map(|pair| sprites[pair[1]].world_x - sprites[pair[0]].world_x)
            .collect();
        let expected_wrap_gap = median(&mut diffs);

        let block_width = cache
            .tile_info
            .get(&far_indices[0])
            .map(|(width, _)| *width)
            .expect("far hills should tile");
        let actual_wrap_gap = block_width - (max_x - min_x);

        assert!(
            (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
            "expected wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
        );
    }

    #[test]
    fn ocean_parent_group_splits_by_name_when_names_differ() {
        let theme = bg_data::get_theme("Jungle").expect("jungle theme");
        let sprites = &theme.sprites;

        let ocean_name_count = sprites
            .iter()
            .filter(|sprite| sprite.parent_group == "Ocean")
            .map(|sprite| sprite.name.to_ascii_lowercase())
            .collect::<HashSet<_>>()
            .len();

        assert!(
            ocean_name_count >= 2,
            "expected multiple Ocean sprite names"
        );

        let wave = sprites
            .iter()
            .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Waves")
            .expect("wave sprite");
        let foam = sprites
            .iter()
            .find(|sprite| sprite.parent_group == "Ocean" && sprite.name == "Foam")
            .expect("foam sprite");

        let wave_key = tile_group_key(wave, "waves", ocean_name_count).expect("wave key");
        let foam_key = tile_group_key(foam, "foam", ocean_name_count).expect("foam key");

        assert!(
            wave_key != foam_key,
            "expected Ocean sub-bands to keep separate repeat groups"
        );
    }

    #[test]
    fn background_cloud_sprites_do_not_drift_over_time() {
        let offset_start = bg_sprite_x_animation_offset(
            "background_clouds _forest_01",
            0.0,
            &bg_data::BgLayer::Sky,
        );
        let offset_later = bg_sprite_x_animation_offset(
            "background_clouds _forest_01",
            123.45,
            &bg_data::BgLayer::Sky,
        );

        assert_eq!(offset_start, 0.0);
        assert_eq!(offset_later, 0.0);
    }

    #[test]
    fn morning_cloud_wrap_gap_matches_internal_edge_gap() {
        let cache = build_bg_layer_cache("Morning", None).expect("morning cache");
        let theme = bg_data::get_theme("Morning").expect("morning theme");
        let sprites = cache.sprites(theme);

        let mut cloud_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, sprite)| {
                sprite.parent_group == "BGLayerClouds"
                    && sprite.name == "Background_Clouds _Forest_01"
            })
            .map(|(idx, _)| idx)
            .collect();
        cloud_indices.sort_by(|a, b| {
            sprites[*a]
                .world_x
                .partial_cmp(&sprites[*b].world_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let first = &sprites[*cloud_indices.first().expect("first cloud")];
        let last = &sprites[*cloud_indices.last().expect("last cloud")];
        let min_left = first.world_x - sprite_display_width(first) * 0.5;
        let max_right = last.world_x + sprite_display_width(last) * 0.5;
        let mut edge_gaps: Vec<f32> = cloud_indices
            .windows(2)
            .map(|pair| {
                let a = &sprites[pair[0]];
                let b = &sprites[pair[1]];
                let a_right = a.world_x + sprite_display_width(a) * 0.5;
                let b_left = b.world_x - sprite_display_width(b) * 0.5;
                b_left - a_right
            })
            .collect();
        let expected_wrap_gap = median(&mut edge_gaps);

        let block_width = cache
            .tile_info
            .get(&cloud_indices[0])
            .map(|(width, _)| *width)
            .expect("cloud strip should tile");
        let actual_wrap_gap = block_width - (max_right - min_left);

        assert!(
            (actual_wrap_gap - expected_wrap_gap).abs() < 0.001,
            "expected wrap gap {expected_wrap_gap}, got {actual_wrap_gap}"
        );
    }

    /// Halloween BGLayerNear has 4 sprite names at 3 different Z values.
    /// Background_Plateau_02 (Z=6) and Lamp_01 (Z=5.5) both round to z_key=6
    /// under the old scheme, merging their interleaved X positions and producing
    /// a completely wrong block_width (~173 instead of ~185).
    /// With name-based splitting each type gets its own clean tile group.
    #[test]
    fn halloween_near_plateau_tiles_at_correct_period() {
        let cache = build_bg_layer_cache("Halloween", None).expect("halloween cache");
        let theme = bg_data::get_theme("Halloween").expect("halloween theme");
        let sprites = cache.sprites(theme);

        let mut plateau_indices: Vec<usize> = sprites
            .iter()
            .enumerate()
            .filter(|(_, s)| s.parent_group == "BGLayerNear" && s.name == "Background_Plateau_02")
            .map(|(idx, _)| idx)
            .collect();
        assert!(
            plateau_indices.len() >= 4,
            "expected BGLayerNear plateau sprites"
        );

        plateau_indices.sort_by(|a, b| {
            sprites[*a]
                .world_x
                .partial_cmp(&sprites[*b].world_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let block_width = cache
            .tile_info
            .get(&plateau_indices[0])
            .map(|(w, _)| *w)
            .expect("plateau should tile");

        // Correct period ≈ 7 * 26.4 ≈ 184–185.  Old (broken) period was ~173.
        assert!(
            block_width > 180.0,
            "block_width {block_width:.2} too small — Z-rounding collision bug"
        );
        assert!(
            block_width < 195.0,
            "block_width {block_width:.2} too large"
        );
    }
}

// ── BG Z-range draw method (extracted from show()) ──

impl LevelRenderer {
    /// Draw background layers for a Z range, constructing BgGpuState if available.
    pub(super) fn draw_bg_z_range(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        z_range: (f32, f32),
    ) {
        let Some(theme_name) = self.bg_theme else {
            return;
        };
        let Some(ref cache) = self.bg_layer_cache else {
            return;
        };
        let mut gpu = match (&self.bg_resources, &self.wgpu_device, &self.wgpu_queue) {
            (Some(r), Some(d), Some(q)) => Some(BgGpuState {
                resources: r.clone(),
                atlas_cache: &mut self.bg_atlas_cache,
                device: d,
                queue: q,
                slot_counter: &mut self.bg_slot_counter,
            }),
            _ => None,
        };
        draw_bg_layers(
            &DrawCtx {
                painter,
                camera: &self.camera,
                canvas_center,
                canvas_rect: rect,
                tex_cache: &self.tex_cache,
            },
            theme_name,
            self.time,
            z_range,
            cache,
            gpu.as_mut(),
        );
    }

    /// Draw sky background, parallax layers interleaved with clouds, and ground bg.
    pub(super) fn draw_background_all(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        dt: f32,
    ) {
        draw_background(painter, rect, &self.camera, canvas_center, self.bg_theme);

        // Reset per-frame shader slot counters
        self.bg_slot_counter = 0;
        self.sprite_slot_counter = 0;
        self.fill_slot_counter = 0;

        // Parallax background layers: sky (worldZ >= 17.5, before cloud instances)
        self.draw_bg_z_range(painter, canvas_center, rect, (17.5, f32::INFINITY));

        // Parallax background layers + clouds: (5 <= worldZ < 17.5)
        {
            let cloud_z_min = self
                .cloud_instances
                .iter()
                .map(|c| c.z)
                .fold(f32::INFINITY, f32::min);
            let cloud_z_max = self
                .cloud_instances
                .iter()
                .map(|c| c.z)
                .fold(f32::NEG_INFINITY, f32::max);

            if !self.cloud_instances.is_empty() {
                self.draw_bg_z_range(painter, canvas_center, rect, (cloud_z_max, 17.5));
                clouds::update_and_draw_clouds(
                    &mut self.cloud_instances,
                    dt,
                    &self.camera,
                    painter,
                    canvas_center,
                    rect,
                    &self.tex_cache,
                );
                self.draw_bg_z_range(painter, canvas_center, rect, (5.0, cloud_z_min));
            } else {
                self.draw_bg_z_range(painter, canvas_center, rect, (5.0, 17.5));
                clouds::update_cloud_positions(&mut self.cloud_instances, dt);
            }
        }
    }
}
