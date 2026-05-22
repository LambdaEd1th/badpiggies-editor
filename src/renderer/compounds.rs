//! Compound prefab rendering — multi-sprite objects (slingshot, fan, door, button, bird).
//!
//! These prefabs are rendered as groups of sub-sprites with relative positions.
//! Sprite data constants live in compound_data.rs.

use eframe::egui;

use crate::data::prefab_sprites::PrefabSpriteLayer;
use crate::data::sprite_db::UvRect;

use super::compound_data::*;
use super::compound_overrides::{project_bridge_runtime, project_fan_runtime};
use super::particles::fan_propeller_foreshorten;
use super::sprite_shader;
use super::{CompoundTransform, DrawCtx};

struct QuadDraw<'a> {
    atlas: &'a str,
    uv: &'a UvRect,
    half_w: f32,
    half_h: f32,
    world_x: f32,
    world_y: f32,
    flip_x: bool,
    flip_y: bool,
    rotation_z: f32,
}

// ─── Public API ─────────────────────────────────────────────────────────

/// Public wrapper for fan runtime config (used by mod.rs for state machine init).
pub fn project_fan_runtime_public(raw_text: Option<&str>) -> FanRuntimePublic {
    let runtime = project_fan_runtime(raw_text);
    FanRuntimePublic {
        target_force: runtime.target_force,
        start_time: runtime.start_time,
        on_time: runtime.on_time,
        off_time: runtime.off_time,
        delayed_start: runtime.delayed_start,
        always_on: runtime.always_on,
    }
}

/// Effective fan runtime config (public subset).
pub struct FanRuntimePublic {
    pub target_force: f32,
    pub start_time: f32,
    pub on_time: f32,
    pub off_time: f32,
    pub delayed_start: f32,
    pub always_on: bool,
}

/// Draw a compound prefab's sub-sprites.
/// Returns true if a compound was drawn (caller should skip normal sprite rendering).
pub fn draw_compound(
    ctx: &DrawCtx<'_>,
    name: &str,
    xf: CompoundTransform,
    time: f64,
    override_text: Option<&str>,
    fan_angle: Option<f32>,
) -> bool {
    if name == "Fan" {
        // Unity Z-order: propeller (Z=0, back) → engine (Z=-0.05) → frame (Z=-0.1, front)
        // Draw propeller first with foreshortening animation
        let foreshorten = fan_propeller_foreshorten(fan_angle);
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        let lx = FAN_PROPELLER.offset_x * xf.scale_x;
        let ly = FAN_PROPELLER.offset_y * xf.scale_y;
        let px = xf.world_x + lx * cos_r - ly * sin_r;
        let py = xf.world_y + lx * sin_r + ly * cos_r;
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: FAN_PROPELLER.atlas,
                uv: &FAN_PROPELLER.uv,
                half_w: FAN_PROPELLER.world_w * xf.scale_x.abs() * foreshorten,
                half_h: FAN_PROPELLER.world_h * xf.scale_y.abs(),
                world_x: px,
                world_y: py,
                flip_x: FAN_PROPELLER.flip_x != (xf.scale_x < 0.0),
                flip_y: FAN_PROPELLER.flip_y != (xf.scale_y < 0.0),
                rotation_z: xf.rotation_z,
            },
        );
        // Then engine (middle) and frame (front)
        draw_sub_sprites_rotated(ctx, &[&*FAN_ENGINE, &*FAN_FRAME], xf);
        return true; // skip root sprite (propeller already drawn)
    }

    if name.starts_with("Bird_") && !name.starts_with("BirdCompass") {
        // Face is drawn by draw_bird_face() AFTER the body sprite, so it renders in front.
        return false; // still draw root bird sprite (body)
    }

    if name == "Bridge" {
        let runtime = project_bridge_runtime(override_text);
        let step_length = runtime.step_length;
        let endpoint_x = runtime.runtime_end_point_x;
        let endpoint_y = runtime.runtime_end_point_y;
        let stride = runtime.stride;
        let step_count = runtime.step_count;
        let angle = runtime.angle;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // First rope: origin → first step left edge
        if step_count > 0 {
            let half_step = step_length * 0.5;
            draw_bridge_rope_segment(
                ctx,
                crate::domain::types::Vec2 {
                    x: xf.world_x,
                    y: xf.world_y,
                },
                crate::domain::types::Vec2 {
                    x: xf.world_x + (0.5 * stride - half_step) * cos_a * xf.scale_x,
                    y: xf.world_y + (0.5 * stride - half_step) * sin_a * xf.scale_y,
                },
            );
        }

        for i in 0..step_count {
            if i > 0 {
                let prev_along = (i as f32 - 0.5) * stride;
                let along = (i as f32 + 0.5) * stride;
                let half_step = step_length * 0.5;
                let rope_start_d = prev_along + half_step;
                let rope_end_d = along - half_step;
                draw_bridge_rope_segment(
                    ctx,
                    crate::domain::types::Vec2 {
                        x: xf.world_x + rope_start_d * cos_a * xf.scale_x,
                        y: xf.world_y + rope_start_d * sin_a * xf.scale_y,
                    },
                    crate::domain::types::Vec2 {
                        x: xf.world_x + rope_end_d * cos_a * xf.scale_x,
                        y: xf.world_y + rope_end_d * sin_a * xf.scale_y,
                    },
                );
            }

            let along = (i as f32 + 0.5) * stride;
            let lx = along * cos_a;
            let ly = along * sin_a;
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: BRIDGE_STEP.atlas,
                    uv: &BRIDGE_STEP.uv,
                    half_w: BRIDGE_STEP.world_w * xf.scale_x.abs(),
                    half_h: BRIDGE_STEP.world_h * xf.scale_y.abs(),
                    world_x: xf.world_x + lx * xf.scale_x,
                    world_y: xf.world_y + ly * xf.scale_y,
                    flip_x: false,
                    flip_y: false,
                    rotation_z: angle,
                },
            );
        }

        // Last rope: last step right edge → endpoint
        if step_count > 0 {
            let half_step = step_length * 0.5;
            let last_right = ((step_count - 1) as f32 + 0.5) * stride + half_step;
            draw_bridge_rope_segment(
                ctx,
                crate::domain::types::Vec2 {
                    x: xf.world_x + last_right * cos_a * xf.scale_x,
                    y: xf.world_y + last_right * sin_a * xf.scale_y,
                },
                crate::domain::types::Vec2 {
                    x: xf.world_x + endpoint_x * xf.scale_x,
                    y: xf.world_y + endpoint_y * xf.scale_y,
                },
            );
        }
        return true;
    }

    if name.starts_with("FloatingStarBox") || name.starts_with("FloatingPartBox") {
        let is_part_box = name.starts_with("FloatingPartBox");
        let sx = xf.scale_x.abs();
        let sy = xf.scale_y.abs();

        // Unity SpringJoint (spring=100, damper=10) makes both box and balloon oscillate.
        // Balloon bobs more (lighter, receives upForce), box bobs less (heavier end).
        let phase = (time * 1.8).sin() as f32;
        let box_bob = 0.06 * phase * sy;
        let balloon_bob = 0.15 * phase * sy;
        let box_y = xf.world_y + box_bob;

        // 1. Box sprite at root position (bobs gently)
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: FLOATING_BOX.atlas,
                uv: &FLOATING_BOX.uv,
                half_w: FLOATING_BOX.world_w * sx,
                half_h: FLOATING_BOX.world_h * sy,
                world_x: xf.world_x,
                world_y: box_y,
                flip_x: false,
                flip_y: false,
                rotation_z: xf.rotation_z,
            },
        );

        // 2. Balloon above at physics equilibrium distance + bobbing
        let balloon_dist = if is_part_box {
            *FLOATING_PART_BALLOON_DISTANCE
        } else {
            *FLOATING_STAR_BALLOON_DISTANCE
        } * sx;
        let balloon_y = box_y + balloon_dist + (balloon_bob - box_bob);
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: FLOATING_BALLOON.atlas,
                uv: &FLOATING_BALLOON.uv,
                half_w: FLOATING_BALLOON.world_w * sx,
                half_h: FLOATING_BALLOON.world_h * sy,
                world_x: xf.world_x,
                world_y: balloon_y,
                flip_x: false,
                flip_y: false,
                rotation_z: 0.0,
            },
        );

        // 3. Rope: black line from rotated box anchor to balloon bottom
        //    Unity LineRenderer width = 0.05 world units
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        let rope_box_anchor_local = if is_part_box {
            *FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL
        } else {
            *FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL
        };
        let rope_balloon_anchor_local = if is_part_box {
            *FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL
        } else {
            *FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL
        };
        let [rbx, rby] = rotate_scaled_xy(rope_box_anchor_local, sx, sy, cos_r, sin_r);
        let [rtx, rty] = rotate_scaled_xy(rope_balloon_anchor_local, sx, sy, cos_r, sin_r);
        let rope_bot = ctx.camera.world_to_screen(
            crate::domain::types::Vec2 {
                x: xf.world_x + rbx,
                y: box_y + rby,
            },
            ctx.canvas_center,
        );
        let rope_top = ctx.camera.world_to_screen(
            crate::domain::types::Vec2 {
                x: xf.world_x + rtx,
                y: box_y + rty + (balloon_bob - box_bob),
            },
            ctx.canvas_center,
        );
        let rope_width = (0.05 * ctx.camera.zoom).max(1.0);
        ctx.painter.line_segment(
            [rope_bot, rope_top],
            egui::Stroke::new(rope_width, egui::Color32::BLACK),
        );

        return true; // skip root sprite (balloon is drawn above)
    }

    if let Some(layers) = crate::data::prefab_sprites::get_multi_sprite_layers(name) {
        let mut generic_xf = xf;
        let name_lower = name.to_ascii_lowercase();
        if name_lower.contains("goal") {
            generic_xf.world_y += (time * 3.0).sin() as f32 * 0.25;
        }
        draw_prefab_layers(ctx, layers, generic_xf);
        return true;
    }

    false
}

/// Draw Bird face sprite AFTER the body has been rendered, so it appears in front.
/// `world_y` (in xf) should already include the sleep bob offset.
/// `breath_sx`/`breath_sy` are the hermite-evaluated scale factors from the vizGroup.
pub fn draw_bird_face(
    ctx: &DrawCtx<'_>,
    name: &str,
    xf: CompoundTransform,
    breath_sx: f32,
    breath_sy: f32,
) {
    // Strip trailing _01 etc to match face lookup
    let base_name = name.trim_end_matches(|c: char| c == '_' || c.is_ascii_digit());
    let base_name = if base_name.is_empty() {
        name
    } else {
        base_name
    };

    if let Some(face) = BIRD_FACES
        .iter()
        .find(|f| base_name.starts_with(f.name_prefix))
    {
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        // Face offset in visualization-local space (breathing scales the viz group).
        // Per-bird offset accounts for both face mesh pivot and body mesh pivot.
        let lx = face.offset_x * breath_sx * xf.scale_x;
        let ly = face.offset_y * breath_sy * xf.scale_y;
        let fx = xf.world_x + lx * cos_r - ly * sin_r;
        let fy = xf.world_y + lx * sin_r + ly * cos_r;
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: face.atlas,
                uv: &face.uv,
                half_w: face.world_w * xf.scale_x.abs() * breath_sx,
                half_h: face.world_h * xf.scale_y.abs() * breath_sy,
                world_x: fx,
                world_y: fy,
                flip_x: xf.scale_x < 0.0,
                flip_y: xf.scale_y < 0.0,
                rotation_z: xf.rotation_z,
            },
        );
    }
}

// ─── Internal helpers ───────────────────────────────────────────────────

fn rotate_scaled_xy(
    local: (f32, f32),
    scale_x: f32,
    scale_y: f32,
    cos_r: f32,
    sin_r: f32,
) -> [f32; 2] {
    let scaled_x = local.0 * scale_x;
    let scaled_y = local.1 * scale_y;
    [
        scaled_x * cos_r - scaled_y * sin_r,
        scaled_x * sin_r + scaled_y * cos_r,
    ]
}

fn draw_sub_sprites_rotated(ctx: &DrawCtx, sprites: &[&SubSprite], xf: CompoundTransform) {
    let cos_r = xf.rotation_z.cos();
    let sin_r = xf.rotation_z.sin();
    for s in sprites {
        let lx = s.offset_x * xf.scale_x;
        let ly = s.offset_y * xf.scale_y;
        let sx = xf.world_x + lx * cos_r - ly * sin_r;
        let sy = xf.world_y + lx * sin_r + ly * cos_r;
        let flip_x = s.flip_x != (xf.scale_x < 0.0);
        let flip_y = s.flip_y != (xf.scale_y < 0.0);
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: s.atlas,
                uv: &s.uv,
                half_w: s.world_w * xf.scale_x.abs(),
                half_h: s.world_h * xf.scale_y.abs(),
                world_x: sx,
                world_y: sy,
                flip_x,
                flip_y,
                rotation_z: xf.rotation_z,
            },
        );
    }
}

fn bridge_rope_transform(
    start_world: crate::domain::types::Vec2,
    end_world: crate::domain::types::Vec2,
) -> CompoundTransform {
    let delta_x = end_world.x - start_world.x;
    let delta_y = end_world.y - start_world.y;
    let segment_length = (delta_x * delta_x + delta_y * delta_y).sqrt();
    let (scale_x, scale_y) = if segment_length >= 1.0 {
        (0.01, 0.01)
    } else {
        (1.0 - segment_length, 1.0)
    };

    CompoundTransform {
        world_x: start_world.x,
        world_y: start_world.y,
        scale_x,
        scale_y,
        rotation_z: delta_y.atan2(delta_x),
    }
}

fn draw_bridge_rope_segment(
    ctx: &DrawCtx<'_>,
    start_world: crate::domain::types::Vec2,
    end_world: crate::domain::types::Vec2,
) {
    let layers = crate::data::prefab_sprites::get_multi_sprite_layers("StepRope")
        .expect("StepRope prefab layers must load from embedded assets");
    draw_prefab_layers(ctx, layers, bridge_rope_transform(start_world, end_world));
}

fn draw_prefab_layers(ctx: &DrawCtx<'_>, layers: &[PrefabSpriteLayer], xf: CompoundTransform) {
    let cos_r = xf.rotation_z.cos();
    let sin_r = xf.rotation_z.sin();

    for layer in layers {
        let Some(tex_id) = ctx.tex_cache.get(&layer.atlas) else {
            continue;
        };
        let Some([atlas_w, atlas_h]) = ctx.tex_cache.texture_size(&layer.atlas) else {
            continue;
        };

        let world_to_screen = |vertex: crate::domain::types::Vec2| {
            let lx = vertex.x * xf.scale_x;
            let ly = vertex.y * xf.scale_y;
            let world_x = xf.world_x + lx * cos_r - ly * sin_r;
            let world_y = xf.world_y + lx * sin_r + ly * cos_r;
            ctx.camera.world_to_screen(
                crate::domain::types::Vec2 {
                    x: world_x,
                    y: world_y,
                },
                ctx.canvas_center,
            )
        };

        let positions = [
            world_to_screen(layer.vertices[0]),
            world_to_screen(layer.vertices[1]),
            world_to_screen(layer.vertices[2]),
            world_to_screen(layer.vertices[3]),
        ];

        let min_x = positions.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = positions
            .iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = positions.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = positions
            .iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        let screen_rect =
            egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y));
        if !screen_rect.intersects(ctx.canvas_rect) {
            continue;
        }

        let (uv_min, uv_max) =
            sprite_shader::compute_uvs(&layer.uv, atlas_w as f32, atlas_h as f32, false, false);
        let uvs = [
            egui::pos2(uv_min[0], uv_max[1]),
            egui::pos2(uv_min[0], uv_min[1]),
            egui::pos2(uv_max[0], uv_min[1]),
            egui::pos2(uv_max[0], uv_max[1]),
        ];
        let mesh = mesh_quad(tex_id, positions, uvs);
        ctx.painter.add(egui::Shape::mesh(mesh));
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::bridge_rope_transform;
    use crate::domain::types::Vec2;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn bridge_rope_transform_matches_unity_short_segment_scale() {
        let xf = bridge_rope_transform(Vec2 { x: 1.0, y: 2.0 }, Vec2 { x: 1.2, y: 2.0 });
        assert_close(xf.world_x, 1.0);
        assert_close(xf.world_y, 2.0);
        assert_close(xf.scale_x, 0.8);
        assert_close(xf.scale_y, 1.0);
        assert_close(xf.rotation_z, 0.0);
    }

    #[test]
    fn bridge_rope_transform_clamps_long_segments_like_unity() {
        let xf = bridge_rope_transform(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 1.2, y: 0.0 });
        assert_close(xf.scale_x, 0.01);
        assert_close(xf.scale_y, 0.01);
        assert_close(xf.rotation_z, 0.0);
    }
}

fn mesh_quad(
    tex_id: egui::TextureId,
    positions: [egui::Pos2; 4],
    uvs: [egui::Pos2; 4],
) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(tex_id);
    let white = egui::Color32::WHITE;
    for (pos, uv) in positions.into_iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: white,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    mesh
}

fn draw_uv_quad_rotated(ctx: &DrawCtx, q: QuadDraw<'_>) {
    let tex_id = match ctx.tex_cache.get(q.atlas) {
        Some(id) => id,
        None => return,
    };

    let center = ctx.camera.world_to_screen(
        crate::domain::types::Vec2 {
            x: q.world_x,
            y: q.world_y,
        },
        ctx.canvas_center,
    );

    let hw = q.half_w * ctx.camera.zoom;
    let hh = q.half_h * ctx.camera.zoom;

    // Frustum cull
    let margin = hw.max(hh);
    if center.x + margin < ctx.canvas_rect.left()
        || center.x - margin > ctx.canvas_rect.right()
        || center.y + margin < ctx.canvas_rect.top()
        || center.y - margin > ctx.canvas_rect.bottom()
    {
        return;
    }

    let (u0, u1) = if q.flip_x {
        (q.uv.x + q.uv.w, q.uv.x)
    } else {
        (q.uv.x, q.uv.x + q.uv.w)
    };
    // UV Y flip: Unity V=0 at bottom, egui V=0 at top
    let (v0, v1) = if q.flip_y {
        (1.0 - q.uv.y, 1.0 - q.uv.y - q.uv.h)
    } else {
        (1.0 - q.uv.y - q.uv.h, 1.0 - q.uv.y)
    };

    let mut mesh = egui::Mesh::with_texture(tex_id);
    if q.rotation_z.abs() > 0.001 {
        let cos_r = q.rotation_z.cos();
        let sin_r = q.rotation_z.sin();
        // Screen Y is inverted (down = positive), negate sin for rotation
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
        let rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));
        let uv_rect = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
        mesh.add_rect_with_uv(rect, uv_rect, egui::Color32::WHITE);
    }
    ctx.painter.add(egui::Shape::mesh(mesh));
}
