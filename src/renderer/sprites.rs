//! Sprite rendering — draw prefab objects as colored squares with correct sizes.
//!
//! Uses sprite database for accurate sizing. Falls back to colored rectangles
//! when atlas textures aren't loaded. Supports textured rendering via egui when available.

use std::collections::BTreeSet;

use eframe::egui;

use crate::assets;
use crate::sprite_db;
use crate::types::*;

use super::compounds;
use super::{background, opaque_shader, sprite_shader};
use super::{Camera, CompoundTransform, DrawCtx, LevelRenderer};

// ── BirdSleep2.anim hermite keyframes (t, value, inSlope, outSlope) ──
pub const BIRD_SLEEP_DURATION: f32 = 4.0;
pub const BIRD_SLEEP_POS_Y: &[(f32, f32, f32, f32)] = &[
    (0.0, 0.0, -0.03356487, -0.03356487),
    (1.833333, -0.061, -0.00255944, -0.00255944),
    (4.0, 0.0, 0.02840104, 0.02840104),
];
pub const BIRD_SLEEP_SCALE_X: &[(f32, f32, f32, f32)] = &[
    (0.0, 1.0, 0.05454547, 0.05454547),
    (1.833333, 1.1, 0.004195808, 0.004195808),
    (4.0, 1.0, -0.04615385, -0.04615385),
];
pub const BIRD_SLEEP_SCALE_Y: &[(f32, f32, f32, f32)] = &[
    (0.0, 1.0, -0.05454547, -0.05454547),
    (1.833333, 0.9, -0.004195808, -0.004195808),
    (4.0, 1.0, 0.04615385, 0.04615385),
];

/// A prepared sprite for drawing.
pub struct SpriteDrawData {
    /// World position.
    pub world_pos: Vec3,
    /// Display color.
    pub color: egui::Color32,
    /// Half-width and half-height in world units.
    pub half_size: (f32, f32),
    /// Instance scale (signed, for flip detection).
    pub scale: (f32, f32),
    /// Object name for labeling.
    pub name: String,
    /// Object index in the level arena.
    pub index: ObjectIndex,
    /// Whether this is a terrain object (skip — terrain renders separately).
    pub is_terrain: bool,
    /// Atlas filename if sprite was found in database.
    pub atlas: Option<String>,
    /// UV rect if sprite was found in database.
    pub uv: Option<sprite_db::UvRect>,
    /// Whether this sprite is hidden (not rendered and not hit-testable unless selected).
    pub is_hidden: bool,
    /// Parent container index (for showing siblings when parent is selected).
    pub parent: Option<ObjectIndex>,
    /// Raw override text for compound rendering (Bridge/Fan parsing).
    pub override_text: Option<String>,
    /// Z-axis rotation in radians.
    pub rotation: f32,
    /// Bird sleep animation phase offset (random per bird, 0..BIRD_SLEEP_DURATION).
    pub bird_phase: f32,
    /// Pre-computed lowercase name (avoids per-frame String allocation).
    pub name_lower: String,
}

/// Build sprite draw data for a prefab instance.
/// `name_override` is the resolved name from level-refs (if different from `prefab.name`).
pub fn build_sprite(
    prefab: &PrefabInstance,
    world_pos: Vec3,
    index: ObjectIndex,
    name_override: Option<&str>,
) -> SpriteDrawData {
    let is_terrain = prefab.terrain_data.is_some();
    let sprite_name = name_override.unwrap_or(&prefab.name);

    // Dessert picker: resolve DessertPlace to a specific dessert variant
    let dessert_resolved;
    let is_dessert_place = sprite_name.contains("DessertPlace") || sprite_name.contains("Desserts");
    let sprite_name = if is_dessert_place {
        let hash = (index as u32).wrapping_mul(2654435761);
        let is_golden = hash % 100 == 50;
        dessert_resolved = if is_golden {
            "GoldenCake".to_string()
        } else {
            const REGULAR: &[&str] = &[
                "StrawberryCake",
                "Cupcake",
                "VanillaCakeSlice",
                "CreamyBun",
                "IcecreamBalls",
            ];
            REGULAR[(hash as usize) % REGULAR.len()].to_string()
        };
        dessert_resolved.as_str()
    } else {
        sprite_name
    };

    // DessertPlace Y offset: shift along local up (accounting for rotation)
    let world_pos = if is_dessert_place {
        let y_off = dessert_y_offset(sprite_name);
        let rot_z = prefab.rotation.z.to_radians();
        Vec3 {
            x: world_pos.x - rot_z.sin() * y_off,
            y: world_pos.y + rot_z.cos() * y_off,
            z: world_pos.z,
        }
    } else {
        world_pos
    };

    let color = assets::get_object_color(sprite_name, prefab.prefab_index);

    let sx = prefab.scale.x.abs().max(0.01);
    let sy = prefab.scale.y.abs().max(0.01);

    // Try to get real sprite size from database
    let sprite_info = sprite_db::get_sprite_info(sprite_name);
    let (half_w, half_h, atlas, uv) = if let Some(info) = sprite_info {
        // world_w/world_h are half-extents; scale by instance scale
        (
            info.world_w * sx,
            info.world_h * sy,
            Some(info.atlas.clone()),
            Some(info.uv),
        )
    } else {
        // Fallback: 0.3 world units half-extent
        (0.3 * sx, 0.3 * sy, None, None)
    };

    let has_atlas = atlas.is_some();

    SpriteDrawData {
        world_pos,
        color,
        half_size: (half_w, half_h),
        scale: (prefab.scale.x, prefab.scale.y),
        name: sprite_name.to_string(),
        index,
        is_terrain,
        atlas,
        uv,
        is_hidden: is_dessert_place
            || sprite_name.starts_with("WindArea")
            || (!has_atlas && assets::should_skip_render(sprite_name)),
        parent: prefab.parent,
        override_text: prefab.override_data.as_ref().map(|od| od.raw_text.clone()),
        rotation: prefab.rotation.z.to_radians(),
        bird_phase: if sprite_name.starts_with("Bird_") && !sprite_name.starts_with("BirdCompass") {
            // Deterministic random phase per bird based on position
            let seed = (world_pos.x * 1000.0) as u32 ^ (world_pos.y * 1000.0) as u32;
            (seed % (BIRD_SLEEP_DURATION as u32 * 1000)) as f32 / 1000.0
        } else {
            0.0
        },
        name_lower: sprite_name.to_ascii_lowercase(),
    }
}

/// Options for `draw_sprite`.
pub struct SpriteDrawOpts {
    pub is_selected: bool,
    pub time: f64,
    pub tex_id: Option<egui::TextureId>,
    pub atlas_size: Option<[usize; 2]>,
    pub fan_angle: Option<f32>,
    pub opaque_rendered: bool,
}

/// Draw a sprite as a colored rectangle on the canvas.
/// If `tex_id` is available, draws a textured quad using the sprite's UV rect.
/// `fan_angle` is the propeller rotation angle from the state machine (for Fan only).
/// `opaque_rendered` — if true, skip texture/rect rendering (already drawn by GPU shader);
/// still draws selection highlight and label.
pub fn draw_sprite(ctx: &DrawCtx<'_>, sprite: &SpriteDrawData, opts: SpriteDrawOpts) {
    let painter = ctx.painter;
    let camera = ctx.camera;
    let canvas_center = ctx.canvas_center;
    let canvas_rect = ctx.canvas_rect;
    let SpriteDrawOpts {
        is_selected,
        time,
        tex_id,
        atlas_size,
        fan_angle,
        opaque_rendered,
    } = opts;
    if sprite.is_terrain {
        return; // terrain renders via terrain module
    }

    // Hidden objects (DessertPlace, skip-render containers) are not drawn unless
    // selected or when their parent is selected
    if sprite.is_hidden && !is_selected {
        return;
    }

    // Goal bobbing animation: sine wave on Y
    let name_lower = &sprite.name_lower;
    let y_offset = if name_lower.contains("goal") || name_lower.contains("dessert") {
        (time * 3.0).sin() as f32 * 0.25
    } else if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
        // BirdSleep2.anim: vertical bob via hermite pos.y curve
        let t = ((time as f32 + sprite.bird_phase) % BIRD_SLEEP_DURATION).max(0.0);
        super::background::hermite(BIRD_SLEEP_POS_Y, t)
    } else {
        0.0
    };

    let center = camera.world_to_screen(
        Vec2 {
            x: sprite.world_pos.x,
            y: sprite.world_pos.y + y_offset,
        },
        canvas_center,
    );

    // Quick frustum cull
    let margin = sprite.half_size.0.max(sprite.half_size.1) * camera.zoom + 20.0;
    if center.x + margin < canvas_rect.left()
        || center.x - margin > canvas_rect.right()
        || center.y + margin < canvas_rect.top()
        || center.y - margin > canvas_rect.bottom()
    {
        return;
    }

    let hw = sprite.half_size.0 * camera.zoom;
    let hh = sprite.half_size.1 * camera.zoom;

    // Fan propeller rotation: foreshorten X via cos(angle) from state machine
    let (hw, hh) = if sprite.name == "Fan" {
        let angle = fan_angle.unwrap_or((time * 10.472) as f32);
        let foreshorten = angle.cos().abs().max(0.05);
        (hw * foreshorten, hh)
    } else if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
        // BirdSleep2.anim: 4-second hermite spline cycle
        let t = ((time as f32 + sprite.bird_phase) % 4.0).max(0.0);
        let sx = super::background::hermite(BIRD_SLEEP_SCALE_X, t);
        let sy = super::background::hermite(BIRD_SLEEP_SCALE_Y, t);
        (hw * sx, hh * sy)
    } else {
        (hw, hh)
    };

    // Ensure minimum visible size
    let hw = hw.max(2.0);
    let hh = hh.max(2.0);

    let rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));

    // Glow is drawn in a separate pass before terrain (see draw_glow).

    // Draw textured quad if atlas texture is available, otherwise colored rectangle.
    // Skip if already rendered via GPU opaque shader.
    if opaque_rendered {
        // Sprite body already drawn by wgpu opaque pipeline — only selection/label below.
    } else if let (Some(tid), Some(uv)) = (tex_id, &sprite.uv) {
        // UV Y flip: Unity V=0 at bottom, egui V=0 at top
        let uv_min = egui::pos2(uv.x, 1.0 - uv.y - uv.h);
        let uv_max = egui::pos2(uv.x + uv.w, 1.0 - uv.y);

        // Half-texel UV inset: prevents bilinear filtering from sampling beyond
        // the sprite boundary in the atlas, eliminating color fringing at edges.
        let tex_w = atlas_size.map_or(2048.0, |s| s[0] as f32);
        let tex_h = atlas_size.map_or(2048.0, |s| s[1] as f32);
        let half_texel_u = 0.5 / tex_w;
        let half_texel_v = 0.5 / tex_h;
        let uv_min = egui::pos2(uv_min.x + half_texel_u, uv_min.y + half_texel_v);
        let uv_max = egui::pos2(uv_max.x - half_texel_u, uv_max.y - half_texel_v);

        // Handle horizontal/vertical flip via UV swap
        let (u0, u1) = if sprite.scale.0 < 0.0 {
            (uv_max.x, uv_min.x)
        } else {
            (uv_min.x, uv_max.x)
        };
        let (v0, v1) = if sprite.scale.1 < 0.0 {
            (uv_max.y, uv_min.y)
        } else {
            (uv_min.y, uv_max.y)
        };

        let mut mesh = egui::Mesh::with_texture(tid);
        if sprite.rotation.abs() > 0.001 {
            // Build rotated quad manually
            let cos_r = sprite.rotation.cos();
            let sin_r = sprite.rotation.sin();
            // Note: screen Y is inverted (down = positive), so negate sin for rotation
            let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                egui::pos2(
                    center.x + dx * cos_r + dy * sin_r,
                    center.y - dx * sin_r + dy * cos_r,
                )
            };
            let tl = rot(-hw, -hh);
            let tr = rot(hw, -hh);
            let br = rot(hw, hh);
            let bl = rot(-hw, hh);
            let white = egui::Color32::WHITE;
            let i = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tl,
                uv: egui::pos2(u0, v0),
                color: white,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tr,
                uv: egui::pos2(u1, v0),
                color: white,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: br,
                uv: egui::pos2(u1, v1),
                color: white,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: bl,
                uv: egui::pos2(u0, v1),
                color: white,
            });
            mesh.indices
                .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
        } else {
            let uv_rect = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
            mesh.add_rect_with_uv(rect, uv_rect, egui::Color32::WHITE);
        }
        painter.add(egui::Shape::mesh(mesh));
    } else if sprite.rotation.abs() > 0.001 {
        // Rotated colored quad
        let cos_r = sprite.rotation.cos();
        let sin_r = sprite.rotation.sin();
        let rot = |dx: f32, dy: f32| -> egui::Pos2 {
            egui::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };
        let points = vec![rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh)];
        painter.add(egui::Shape::convex_polygon(
            points,
            sprite.color,
            egui::Stroke::NONE,
        ));
    } else {
        painter.rect_filled(rect, 1.0, sprite.color);
    }

    // Selection highlight
    if is_selected {
        if sprite.rotation.abs() > 0.001 {
            let cos_r = sprite.rotation.cos();
            let sin_r = sprite.rotation.sin();
            let ehw = hw + 2.0;
            let ehh = hh + 2.0;
            let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                egui::pos2(
                    center.x + dx * cos_r + dy * sin_r,
                    center.y - dx * sin_r + dy * cos_r,
                )
            };
            let points = vec![
                rot(-ehw, -ehh),
                rot(ehw, -ehh),
                rot(ehw, ehh),
                rot(-ehw, ehh),
                rot(-ehw, -ehh),
            ];
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(2.0, egui::Color32::YELLOW),
            ));
        } else {
            painter.rect_stroke(
                rect.expand(2.0),
                2.0,
                egui::Stroke::new(2.0, egui::Color32::YELLOW),
                egui::StrokeKind::Outside,
            );
        }
    }

    // Label at reasonable zoom levels
    if camera.zoom > 15.0 {
        let font = egui::FontId::proportional(9.0);
        // Truncate long names
        let label = if sprite.name.len() > 20 {
            format!("{}…", &sprite.name[..19])
        } else {
            sprite.name.clone()
        };
        painter.text(
            egui::pos2(center.x, rect.bottom() + 2.0),
            egui::Align2::CENTER_TOP,
            label,
            font,
            egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180),
        );
    }

    // Particle/animation stubs
    // WindArea: semi-transparent zone + direction arrows
    if name_lower.starts_with("windarea") {
        let zone_hw = 20.0 * sprite.scale.0.abs() * camera.zoom;
        let zone_hh = 7.5 * sprite.scale.1.abs() * camera.zoom;
        let zone_rect =
            egui::Rect::from_center_size(center, egui::vec2(zone_hw * 2.0, zone_hh * 2.0));
        painter.rect_filled(
            zone_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(100, 200, 255, 20),
        );
        painter.rect_stroke(
            zone_rect,
            0.0,
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(100, 200, 255, 60),
            ),
            egui::StrokeKind::Outside,
        );
        // Wind direction arrow (pointing right, wind blows in +X direction)
        let arrow_y = center.y;
        let arrow_len = zone_hw.min(40.0);
        let arrow_start = egui::pos2(center.x - arrow_len * 0.5, arrow_y);
        let arrow_end = egui::pos2(center.x + arrow_len * 0.5, arrow_y);
        let arrow_color = egui::Color32::from_rgba_unmultiplied(100, 200, 255, 100);
        painter.line_segment(
            [arrow_start, arrow_end],
            egui::Stroke::new(2.0, arrow_color),
        );
        // Arrowhead
        painter.line_segment(
            [arrow_end, egui::pos2(arrow_end.x - 6.0, arrow_y - 4.0)],
            egui::Stroke::new(2.0, arrow_color),
        );
        painter.line_segment(
            [arrow_end, egui::pos2(arrow_end.x - 6.0, arrow_y + 4.0)],
            egui::Stroke::new(2.0, arrow_color),
        );
    }
}

/// Y offset per dessert variant (from prefab BoxCollider center.y).
fn dessert_y_offset(name: &str) -> f32 {
    match name {
        "Cupcake" => 0.4167,
        "StrawberryCake" => 0.7813,
        "VanillaCakeSlice" => 0.4688,
        "GoldenCake" => 0.2604,
        "CreamyBun" => 0.3125,
        "IcecreamBalls" => 0.6771,
        _ => 0.0,
    }
}

/// Returns true if this sprite should have a glow starburst behind it.
pub fn has_glow(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.starts_with("goalarea") && n != "goalarea_night")
        || n.starts_with("boxchallenge")
        || n.starts_with("dynamicboxchallenge")
        || n.contains("starbox")
}

/// Draw glow starburst effect. Called in a separate pass before terrain to match
/// Unity/TS render order (glow renderOrder = terrainFill - 1 = behind terrain).
/// GoalArea glows bob vertically; BoxChallenge/StarBox glows are stationary.
pub fn draw_glow(
    painter: &egui::Painter,
    sprite: &SpriteDrawData,
    camera: &Camera,
    canvas_center: egui::Vec2,
    canvas_rect: egui::Rect,
    time: f64,
    glow_tex: egui::TextureId,
) {
    let name_lower = &sprite.name_lower;
    let bobs = name_lower.starts_with("goalarea");
    let y_offset = if bobs {
        (time * 3.0).sin() as f32 * 0.25
    } else {
        0.0
    };

    let center = camera.world_to_screen(
        Vec2 {
            x: sprite.world_pos.x,
            y: sprite.world_pos.y + y_offset,
        },
        canvas_center,
    );

    // Glow sprite from Particles_Sheet_01.png: 3×3 cells at grid (0,5) in 16×16 atlas
    // TS: glowFullSize = 114 * 20 / 768 ≈ 2.96875 world units
    let glow_world_size = 114.0 * 20.0 / 768.0;
    let glow_hw = glow_world_size * 0.5 * camera.zoom;
    let glow_hh = glow_world_size * 0.5 * camera.zoom;

    // Quick frustum cull
    let margin = glow_hw.max(glow_hh) + 20.0;
    if center.x + margin < canvas_rect.left()
        || center.x - margin > canvas_rect.right()
        || center.y + margin < canvas_rect.top()
        || center.y - margin > canvas_rect.bottom()
    {
        return;
    }

    let angle = (time * std::f64::consts::PI / 10.0) as f32;

    // UV rect: (0/16, 5/16) .. (3/16, 8/16) — flip Y for egui (V=0=top)
    let u0 = 0.0_f32;
    let u1 = 3.0 / 16.0;
    let v0 = 1.0 - 8.0 / 16.0; // = 0.5
    let v1 = 1.0 - 5.0 / 16.0; // = 0.6875

    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let rot = |dx: f32, dy: f32| -> egui::Pos2 {
        egui::pos2(
            center.x + dx * cos_a + dy * sin_a,
            center.y - dx * sin_a + dy * cos_a,
        )
    };

    let mut mesh = egui::Mesh::with_texture(glow_tex);
    let tl = rot(-glow_hw, -glow_hh);
    let tr = rot(glow_hw, -glow_hh);
    let br = rot(glow_hw, glow_hh);
    let bl = rot(-glow_hw, glow_hh);
    let white = egui::Color32::WHITE;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: tl,
        uv: egui::pos2(u0, v0),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: tr,
        uv: egui::pos2(u1, v0),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: br,
        uv: egui::pos2(u1, v1),
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: bl,
        uv: egui::pos2(u0, v1),
        color: white,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

// ── Sprite rendering loop (extracted from show()) ──

impl LevelRenderer {
    /// Draw all sprites with GPU batching, compound sub-sprites, and bird face deferral.
    pub(super) fn draw_sprites(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        selected: &BTreeSet<ObjectIndex>,
    ) {
        let t = self.time;

        // Pre-compute world-space visible rect for frustum culling
        let world_half_w = rect.width() * 0.5 / self.camera.zoom;
        let world_half_h = rect.height() * 0.5 / self.camera.zoom;
        let visible_min_x = self.camera.center.x - world_half_w;
        let visible_max_x = self.camera.center.x + world_half_w;
        let visible_min_y = self.camera.center.y - world_half_h;
        let visible_max_y = self.camera.center.y + world_half_h;

        // Build fan angle lookup (avoids O(sprites × fans) per-frame scan)
        let mut fan_angle_map: Vec<Option<f32>> = vec![None; self.sprite_data.len()];
        for e in &self.fan_emitters {
            if e.sprite_index < fan_angle_map.len() {
                fan_angle_map[e.sprite_index] = Some(e.angle);
            }
        }

        // Collect GPU sprite draws into a single Z-ordered Vec.
        // Consecutive same-type draws are batched into one PaintCallback when emitted.
        enum GpuDraw {
            Opaque(opaque_shader::OpaqueBatchDraw),
            Transparent(sprite_shader::SpriteBatchDraw),
        }
        let mut gpu_draws: Vec<GpuDraw> = Vec::new();

        // Deferred bird face draws: must render AFTER the GPU batch callbacks so
        // faces appear on top of GPU-rendered bird bodies.
        struct DeferredBird {
            name: String,
            wx: f32,
            wy: f32,
            sx: f32,
            sy: f32,
            rot: f32,
            bsx: f32,
            bsy: f32,
        }
        let mut deferred_birds: Vec<DeferredBird> = Vec::new();

        for (si, sprite) in self.sprite_data.iter().enumerate() {
            let is_sel = selected.contains(&sprite.index)
                || (sprite.is_hidden
                    && sprite.parent.is_some()
                    && sprite.parent.map_or(false, |p| selected.contains(&p)));

            // Early world-space frustum cull
            if !is_sel {
                let margin = sprite.half_size.0.max(sprite.half_size.1) + 2.0;
                let sx = sprite.world_pos.x;
                let sy = sprite.world_pos.y;
                if sx + margin < visible_min_x
                    || sx - margin > visible_max_x
                    || sy + margin < visible_min_y
                    || sy - margin > visible_max_y
                {
                    continue;
                }
            }

            let fan_angle = fan_angle_map[si];
            let skip_root = compounds::draw_compound(
                &DrawCtx {
                    painter,
                    camera: &self.camera,
                    canvas_center,
                    canvas_rect: rect,
                    tex_cache: &self.tex_cache,
                },
                &sprite.name,
                CompoundTransform {
                    world_x: sprite.world_pos.x,
                    world_y: sprite.world_pos.y,
                    scale_x: sprite.scale.0,
                    scale_y: sprite.scale.1,
                    rotation_z: sprite.rotation,
                },
                t,
                sprite.override_text.as_deref(),
            );

            let mut is_gpu_rendered = false;

            if !skip_root {
                let opaque_idx = self.opaque_sprite_map.get(si).copied().flatten();
                // Props sprites: render via GPU opaque shader (exact Unity shader port)
                if let Some(oidx) = opaque_idx
                    && let (Some(_resources), Some(_batch)) =
                        (&self.opaque_resources, &self.opaque_batch)
                {
                    // Compute per-sprite y_offset (goal/dessert bobbing, bird sleep bob)
                    let y_off = if sprite.name_lower.contains("goal")
                        || sprite.name_lower.contains("dessert")
                    {
                        (t * 3.0).sin() as f32 * 0.25
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % BIRD_SLEEP_DURATION).max(0.0);
                        background::hermite(BIRD_SLEEP_POS_Y, bt)
                    } else {
                        0.0
                    };
                    let (cam_x, cam_y) = if let Some(ref drag) = self.dragging {
                        if drag.index == sprite.index {
                            let dx = sprite.world_pos.x - drag.original_pos.x;
                            let dy = sprite.world_pos.y - drag.original_pos.y;
                            (self.camera.center.x - dx, self.camera.center.y - dy)
                        } else {
                            (self.camera.center.x, self.camera.center.y)
                        }
                    } else if let Some((idx, dx, dy)) = self.pending_drag_offset {
                        if idx == sprite.index {
                            (self.camera.center.x - dx, self.camera.center.y - dy)
                        } else {
                            (self.camera.center.x, self.camera.center.y)
                        }
                    } else {
                        (self.camera.center.x, self.camera.center.y)
                    };
                    gpu_draws.push(GpuDraw::Opaque(opaque_shader::OpaqueBatchDraw {
                        sprite_index: oidx,
                        cam_x,
                        cam_y,
                        y_offset: y_off,
                    }));
                    is_gpu_rendered = true;
                }
                // Non-Props sprites: render via GPU transparent sprite shader
                let mut _sprite_gpu_rendered = false;
                if (is_sel || !sprite.is_hidden)
                    && opaque_idx.is_none()
                    && let (Some(atlas_name), Some(uv)) = (&sprite.atlas, &sprite.uv)
                    && let (Some(resources), Some(device), Some(queue)) =
                        (&self.sprite_resources, &self.wgpu_device, &self.wgpu_queue)
                    && let Some(atlas) = self
                        .sprite_atlas_cache
                        .get_or_load(device, queue, resources, atlas_name)
                    && self.sprite_slot_counter < sprite_shader::max_draw_slots()
                {
                    let slot = self.sprite_slot_counter;
                    self.sprite_slot_counter += 1;

                    // Compute animation offsets
                    let y_off = if sprite.name_lower.contains("goal")
                        || sprite.name_lower.contains("dessert")
                    {
                        (t * 3.0).sin() as f32 * 0.25
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % BIRD_SLEEP_DURATION).max(0.0);
                        background::hermite(BIRD_SLEEP_POS_Y, bt)
                    } else {
                        0.0
                    };

                    // Animated half-size (Fan foreshorten, Bird scale)
                    let (hw, hh) = if sprite.name == "Fan" {
                        let angle = fan_angle.unwrap_or((t * 10.472) as f32);
                        let foreshorten = angle.cos().abs().max(0.05);
                        (sprite.half_size.0 * foreshorten, sprite.half_size.1)
                    } else if sprite.name.starts_with("Bird_")
                        && !sprite.name.starts_with("BirdCompass")
                    {
                        let bt = ((t as f32 + sprite.bird_phase) % 4.0).max(0.0);
                        let sx = background::hermite(BIRD_SLEEP_SCALE_X, bt);
                        let sy = background::hermite(BIRD_SLEEP_SCALE_Y, bt);
                        (sprite.half_size.0 * sx, sprite.half_size.1 * sy)
                    } else {
                        sprite.half_size
                    };

                    let (uv_min, uv_max) = sprite_shader::compute_uvs(
                        uv,
                        atlas.width as f32,
                        atlas.height as f32,
                        sprite.scale.0 < 0.0,
                        sprite.scale.1 < 0.0,
                    );

                    let uniforms = sprite_shader::SpriteUniforms {
                        screen_size: [rect.width(), rect.height()],
                        camera_center: [self.camera.center.x, self.camera.center.y],
                        zoom: self.camera.zoom,
                        rotation: sprite.rotation,
                        world_center: [sprite.world_pos.x, sprite.world_pos.y + y_off],
                        half_size: [hw, hh],
                        uv_min,
                        uv_max,
                        mode: 0.0,
                        shine_center: 0.0,
                        tint_color: [1.0, 1.0, 1.0, 1.0],
                    };

                    gpu_draws.push(GpuDraw::Transparent(sprite_shader::SpriteBatchDraw {
                        atlas,
                        slot,
                        uniforms,
                    }));
                    _sprite_gpu_rendered = true;
                    is_gpu_rendered = true;
                }
                let gpu_rendered = is_gpu_rendered;
                let tex_id = if gpu_rendered {
                    None
                } else {
                    sprite.atlas.as_ref().and_then(|a| self.tex_cache.get(a))
                };
                let atlas_size = if gpu_rendered {
                    None
                } else {
                    sprite
                        .atlas
                        .as_ref()
                        .and_then(|a| self.tex_cache.texture_size(a))
                };
                draw_sprite(
                    &DrawCtx {
                        painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    sprite,
                    SpriteDrawOpts {
                        is_selected: is_sel,
                        time: t,
                        tex_id,
                        atlas_size,
                        fan_angle,
                        opaque_rendered: gpu_rendered,
                    },
                );
            }

            // Bird face: defer if GPU-rendered so faces draw after batch callback
            if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
                let bt =
                    ((t as f32 + sprite.bird_phase) % BIRD_SLEEP_DURATION).max(0.0);
                let breath_y = background::hermite(BIRD_SLEEP_POS_Y, bt);
                let breath_sx = background::hermite(BIRD_SLEEP_SCALE_X, bt);
                let breath_sy = background::hermite(BIRD_SLEEP_SCALE_Y, bt);
                if is_gpu_rendered {
                    deferred_birds.push(DeferredBird {
                        name: sprite.name.clone(),
                        wx: sprite.world_pos.x,
                        wy: sprite.world_pos.y + breath_y,
                        sx: sprite.scale.0,
                        sy: sprite.scale.1,
                        rot: sprite.rotation,
                        bsx: breath_sx,
                        bsy: breath_sy,
                    });
                } else {
                    compounds::draw_bird_face(
                        &DrawCtx {
                            painter,
                            camera: &self.camera,
                            canvas_center,
                            canvas_rect: rect,
                            tex_cache: &self.tex_cache,
                        },
                        &sprite.name,
                        CompoundTransform {
                            world_x: sprite.world_pos.x,
                            world_y: sprite.world_pos.y + breath_y,
                            scale_x: sprite.scale.0,
                            scale_y: sprite.scale.1,
                            rotation_z: sprite.rotation,
                        },
                        breath_sx,
                        breath_sy,
                    );
                }
            }

            if skip_root && is_sel {
                let center = self.camera.world_to_screen(
                    Vec2 {
                        x: sprite.world_pos.x,
                        y: sprite.world_pos.y,
                    },
                    canvas_center,
                );
                let hw = sprite.half_size.0 * self.camera.zoom;
                let hh = sprite.half_size.1 * self.camera.zoom;
                let sel_rect = egui::Rect::from_center_size(
                    center,
                    egui::vec2(hw.max(4.0) * 2.0, hh.max(4.0) * 2.0),
                );
                painter.rect_stroke(
                    sel_rect.expand(2.0),
                    2.0,
                    egui::Stroke::new(2.0, egui::Color32::YELLOW),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Emit GPU sprite callbacks in Z order, batching consecutive same-type
        // draws into one callback to minimise render state resets.
        {
            let mut pending_opaque: Vec<opaque_shader::OpaqueBatchDraw> = Vec::new();
            let mut pending_transparent: Vec<sprite_shader::SpriteBatchDraw> = Vec::new();
            let props_tint = assets::props_tint_color(self.bg_theme);

            for draw in gpu_draws {
                match draw {
                    GpuDraw::Opaque(d) => {
                        if !pending_transparent.is_empty()
                            && let Some(resources) = &self.sprite_resources
                        {
                            painter.add(sprite_shader::make_sprite_batch_callback(
                                rect,
                                resources.clone(),
                                std::mem::take(&mut pending_transparent),
                            ));
                        }
                        pending_opaque.push(d);
                    }
                    GpuDraw::Transparent(d) => {
                        if !pending_opaque.is_empty()
                            && let (Some(resources), Some(batch)) =
                                (&self.opaque_resources, &self.opaque_batch)
                        {
                            painter.add(opaque_shader::make_opaque_batch_callback(
                                rect,
                                resources.clone(),
                                batch.clone(),
                                opaque_shader::OpaqueBatchParams {
                                    screen_w: rect.width(),
                                    screen_h: rect.height(),
                                    zoom: self.camera.zoom,
                                    tint_color: props_tint,
                                },
                                std::mem::take(&mut pending_opaque),
                            ));
                        }
                        pending_transparent.push(d);
                    }
                }
            }

            // Flush remaining draws
            if !pending_opaque.is_empty()
                && let (Some(resources), Some(batch)) =
                    (&self.opaque_resources, &self.opaque_batch)
            {
                painter.add(opaque_shader::make_opaque_batch_callback(
                    rect,
                    resources.clone(),
                    batch.clone(),
                    opaque_shader::OpaqueBatchParams {
                        screen_w: rect.width(),
                        screen_h: rect.height(),
                        zoom: self.camera.zoom,
                        tint_color: props_tint,
                    },
                    pending_opaque,
                ));
            }
            if !pending_transparent.is_empty()
                && let Some(resources) = &self.sprite_resources
            {
                painter.add(sprite_shader::make_sprite_batch_callback(
                    rect,
                    resources.clone(),
                    pending_transparent,
                ));
            }
        }

        // Deferred bird faces: draw after GPU batch so faces appear on top of bodies
        for bird in &deferred_birds {
            compounds::draw_bird_face(
                &DrawCtx {
                    painter,
                    camera: &self.camera,
                    canvas_center,
                    canvas_rect: rect,
                    tex_cache: &self.tex_cache,
                },
                &bird.name,
                CompoundTransform {
                    world_x: bird.wx,
                    world_y: bird.wy,
                    scale_x: bird.sx,
                    scale_y: bird.sy,
                    rotation_z: bird.rot,
                },
                bird.bsx,
                bird.bsy,
            );
        }
    }

    /// Draw glow starbursts and goal flags before collider terrain.
    pub(super) fn draw_pre_terrain_effects(
        &self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let world_half_w = rect.width() * 0.5 / self.camera.zoom;
        let world_half_h = rect.height() * 0.5 / self.camera.zoom;
        let visible_min_x = self.camera.center.x - world_half_w;
        let visible_max_x = self.camera.center.x + world_half_w;
        let visible_min_y = self.camera.center.y - world_half_h;
        let visible_max_y = self.camera.center.y + world_half_h;

        // Glow starbursts behind collider terrain
        if let Some(glow_id) = self.tex_cache.get(super::GLOW_ATLAS) {
            for sprite in &self.sprite_data {
                if !has_glow(&sprite.name) {
                    continue;
                }
                let glow_margin = 2.0;
                if sprite.world_pos.x + glow_margin < visible_min_x
                    || sprite.world_pos.x - glow_margin > visible_max_x
                    || sprite.world_pos.y + glow_margin < visible_min_y
                    || sprite.world_pos.y - glow_margin > visible_max_y
                {
                    continue;
                }
                draw_glow(
                    painter,
                    sprite,
                    &self.camera,
                    canvas_center,
                    rect,
                    self.time,
                    glow_id,
                );
            }
        }

        // Goal flag meshes: draw BEFORE collider terrain so terrain edge
        // occludes the flag bottom.
        if let Some(flag_tex) = self.tex_cache.get(super::GOAL_FLAG_TEXTURE) {
            for sprite in &self.sprite_data {
                if !sprite.name_lower.starts_with("goalarea") {
                    continue;
                }
                let margin = 1.5;
                if sprite.world_pos.x + margin < visible_min_x
                    || sprite.world_pos.x - margin > visible_max_x
                    || sprite.world_pos.y + margin < visible_min_y
                    || sprite.world_pos.y - margin > visible_max_y
                {
                    continue;
                }
                super::goal_flag::draw_goal_flag(
                    painter,
                    sprite,
                    &self.camera,
                    canvas_center,
                    rect,
                    self.time,
                    flag_tex,
                );
            }
        }
    }
}
