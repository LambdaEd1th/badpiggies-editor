//! `draw_sprite` and helpers.

use eframe::egui;

use crate::domain::types::*;
use crate::data::goal_animation::goal_visual_state;

use super::super::particles::{FAN_FIELD_CENTER_Y, FAN_FIELD_HALF_H, FAN_FIELD_HALF_W, WindAreaDef};
use super::super::{CompoundTransform, DrawCtx, PreviewPlaybackState};
use super::{bird_sleep_scale_factors, bird_sleep_y_offset, SpriteDrawData, SpriteDrawOpts};

fn with_alpha(color: egui::Color32, alpha: f32) -> egui::Color32 {
    let scaled_alpha = ((color.a() as f32) * alpha.clamp(0.0, 1.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), scaled_alpha)
}

fn selection_outline_metrics(
    sprite: &SpriteDrawData,
    camera: &super::super::Camera,
    canvas_center: egui::Vec2,
    animated_center: egui::Pos2,
    animated_hw: f32,
    animated_hh: f32,
) -> (egui::Pos2, f32, f32) {
    if sprite.name_lower.starts_with("goalarea") {
        (
            camera.world_to_screen(
                Vec2 {
                    x: sprite.world_pos.x,
                    y: sprite.world_pos.y,
                },
                canvas_center,
            ),
            (sprite.half_size.0 * camera.zoom).max(2.0),
            (sprite.half_size.1 * camera.zoom).max(2.0),
        )
    } else {
        (animated_center, animated_hw, animated_hh)
    }
}

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
        wind_area,
        preview_state,
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

    let name_lower = &sprite.name_lower;
    let goal_visual = name_lower
        .starts_with("goalarea")
        .then(|| goal_visual_state(sprite.goal_animation_state, time, sprite.index));
    let y_offset = if let Some(visual) = goal_visual {
        visual.y_offset
    } else if name_lower.contains("dessert") {
        (time * 3.0).sin() as f32 * 0.25
    } else if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
        bird_sleep_y_offset(time, sprite.bird_phase)
    } else {
        0.0
    };
    let draw_color = with_alpha(
        sprite.color,
        goal_visual.map(|visual| visual.alpha).unwrap_or(1.0),
    );
    if draw_color.a() == 0 && !is_selected {
        return;
    }

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

    let (goal_scale_x, goal_scale_y) = goal_visual
        .map(|visual| (visual.scale_x.max(0.0), visual.scale_y.max(0.0)))
        .unwrap_or((1.0, 1.0));
    let hw = sprite.half_size.0 * goal_scale_x * camera.zoom;
    let hh = sprite.half_size.1 * goal_scale_y * camera.zoom;

    // Fan propeller rotation: foreshorten X via cos(angle) from state machine
    let (hw, hh) = if sprite.name == "Fan" {
        let angle = fan_angle.unwrap_or((time * 10.472) as f32);
        let foreshorten = angle.cos().abs().max(0.05);
        (hw * foreshorten, hh)
    } else if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
        let (sx, sy) = bird_sleep_scale_factors(time, sprite.bird_phase);
        (hw * sx, hh * sy)
    } else {
        (hw, hh)
    };

    // Ensure minimum visible size
    let hw = hw.max(2.0);
    let hh = hh.max(2.0);

    let rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));
    let skip_goal_fallback_body = name_lower.starts_with("goalarea") && sprite.uv.is_none();

    // Glow is drawn in a separate pass before terrain (see draw_glow).

    // Draw textured quad if atlas texture is available, otherwise colored rectangle.
    // Skip if already rendered via GPU opaque shader.
    if opaque_rendered {
        // Sprite body already drawn by wgpu opaque pipeline — only selection/label below.
    } else if skip_goal_fallback_body {
        // Regular GoalArea prefabs use the dedicated GoalSprite mesh/flag path.
        // Suppress the colored fallback quad when there is no rectangular sprite body.
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
            let i = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tl,
                uv: egui::pos2(u0, v0),
                color: draw_color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: tr,
                uv: egui::pos2(u1, v0),
                color: draw_color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: br,
                uv: egui::pos2(u1, v1),
                color: draw_color,
            });
            mesh.vertices.push(egui::epaint::Vertex {
                pos: bl,
                uv: egui::pos2(u0, v1),
                color: draw_color,
            });
            mesh.indices
                .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
        } else {
            let uv_rect = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
            mesh.add_rect_with_uv(rect, uv_rect, draw_color);
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
            draw_color,
            egui::Stroke::NONE,
        ));
    } else {
        painter.rect_filled(rect, 1.0, draw_color);
    }

    // Selection highlight
    if is_selected {
        let (sel_center, sel_hw, sel_hh) = selection_outline_metrics(
            sprite,
            camera,
            canvas_center,
            center,
            hw,
            hh,
        );
        if sprite.rotation.abs() > 0.001 {
            let cos_r = sprite.rotation.cos();
            let sin_r = sprite.rotation.sin();
            let ehw = sel_hw + 2.0;
            let ehh = sel_hh + 2.0;
            let rot = |dx: f32, dy: f32| -> egui::Pos2 {
                egui::pos2(
                    sel_center.x + dx * cos_r + dy * sin_r,
                    sel_center.y - dx * sin_r + dy * cos_r,
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
            let sel_rect = egui::Rect::from_center_size(sel_center, egui::vec2(sel_hw * 2.0, sel_hh * 2.0));
            painter.rect_stroke(
                sel_rect.expand(2.0),
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
    if name_lower.starts_with("windarea") && let Some(area) = wind_area {
        draw_wind_area_overlay(ctx, area, preview_state);
    }
}

fn draw_arrow(
    painter: &egui::Painter,
    start: egui::Pos2,
    end: egui::Pos2,
    color: egui::Color32,
    stroke_width: f32,
) {
    painter.line_segment([start, end], egui::Stroke::new(stroke_width, color));
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / len;
    let uy = dy / len;
    let head = 5.0;
    let side = 3.0;
    painter.line_segment(
        [
            end,
            egui::pos2(end.x - ux * head - uy * side, end.y - uy * head + ux * side),
        ],
        egui::Stroke::new(stroke_width, color),
    );
    painter.line_segment(
        [
            end,
            egui::pos2(end.x - ux * head + uy * side, end.y - uy * head - ux * side),
        ],
        egui::Stroke::new(stroke_width, color),
    );
}

pub fn draw_wind_area_overlay(
    ctx: &DrawCtx<'_>,
    area: WindAreaDef,
    preview_state: PreviewPlaybackState,
) {
    let center = ctx.camera.world_to_screen(
        Vec2 {
            x: area.center_x,
            y: area.center_y,
        },
        ctx.canvas_center,
    );
    let zone_rect = egui::Rect::from_center_size(
        center,
        egui::vec2(
            area.half_w * ctx.camera.zoom * 2.0,
            area.half_h * ctx.camera.zoom * 2.0,
        ),
    );
    let active_alpha = if preview_state == PreviewPlaybackState::Play {
        1.0
    } else {
        0.35
    };
    ctx.painter.rect_filled(
        zone_rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(100, 200, 255, (20.0 * active_alpha) as u8),
    );
    ctx.painter.rect_stroke(
        zone_rect,
        0.0,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(100, 200, 255, (70.0 * active_alpha) as u8),
        ),
        egui::StrokeKind::Outside,
    );

    let dir_len = (area.dir_x * area.dir_x + area.dir_y * area.dir_y)
        .sqrt()
        .max(f32::EPSILON);
    let dir_x = area.dir_x / dir_len;
    let dir_y = area.dir_y / dir_len;
    let arrow_world_len = 2.4 * area.power_factor.max(0.5);
    let arrow_color = egui::Color32::from_rgba_unmultiplied(120, 220, 255, (110.0 * active_alpha) as u8);
    let mut local_y = -area.half_h;
    while local_y <= area.half_h + 0.001 {
        let mut local_x = -area.half_w;
        while local_x <= area.half_w + 0.001 {
            let start = ctx.camera.world_to_screen(
                Vec2 {
                    x: area.center_x + local_x,
                    y: area.center_y + local_y,
                },
                ctx.canvas_center,
            );
            let end = ctx.camera.world_to_screen(
                Vec2 {
                    x: area.center_x + local_x + dir_x * arrow_world_len,
                    y: area.center_y + local_y + dir_y * arrow_world_len,
                },
                ctx.canvas_center,
            );
            draw_arrow(ctx.painter, start, end, arrow_color, 1.5);
            local_x += 8.0;
        }
        local_y += 8.0;
    }
}

fn fan_field_point(xf: CompoundTransform, local_x: f32, local_y: f32) -> Vec2 {
    let cos_r = xf.rotation_z.cos();
    let sin_r = xf.rotation_z.sin();
    Vec2 {
        x: xf.world_x + local_x * cos_r - local_y * sin_r,
        y: xf.world_y + local_x * sin_r + local_y * cos_r,
    }
}

pub fn draw_fan_field_overlay(
    ctx: &DrawCtx<'_>,
    xf: CompoundTransform,
    fan_force: Option<f32>,
    preview_state: PreviewPlaybackState,
) {
    let field_half_w = FAN_FIELD_HALF_W * xf.scale_x.abs();
    let field_half_h = FAN_FIELD_HALF_H * xf.scale_y.abs();
    let field_center_y = FAN_FIELD_CENTER_Y * xf.scale_y;
    let outline = [
        fan_field_point(xf, -field_half_w, field_center_y - field_half_h),
        fan_field_point(xf, field_half_w, field_center_y - field_half_h),
        fan_field_point(xf, field_half_w, field_center_y + field_half_h),
        fan_field_point(xf, -field_half_w, field_center_y + field_half_h),
        fan_field_point(xf, -field_half_w, field_center_y - field_half_h),
    ]
    .map(|p| ctx.camera.world_to_screen(p, ctx.canvas_center))
    .to_vec();
    ctx.painter.add(egui::Shape::line(
        outline,
        egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(120, 220, 255, 110)),
    ));

    let preview_scale = if preview_state == PreviewPlaybackState::Play {
        fan_force.unwrap_or(0.0).clamp(0.0, 10.0) / 10.0
    } else {
        0.0
    };
    let active_scale = preview_scale.max(0.2);
    let cols = 5;
    let rows = 4;
    for row in 0..rows {
        let v = (row as f32 + 0.5) / rows as f32;
        let local_y = field_center_y - field_half_h + v * field_half_h * 2.0;
        for col in 0..cols {
            let u = (col as f32 + 0.5) / cols as f32;
            let local_x = -field_half_w + u * field_half_w * 2.0;
            let horizontal = 1.0 - (local_x.abs() / field_half_w.max(f32::EPSILON));
            let vertical = v;
            let weight = (horizontal * vertical).clamp(0.0, 1.0);
            let arrow_world_len = (0.5 + 1.0 * weight * active_scale) * xf.scale_y.abs().max(1.0);
            let start = fan_field_point(xf, local_x, local_y);
            let end = fan_field_point(xf, local_x, local_y + arrow_world_len);
            let alpha = (50.0 + 150.0 * weight * active_scale) as u8;
            draw_arrow(
                ctx.painter,
                ctx.camera.world_to_screen(start, ctx.canvas_center),
                ctx.camera.world_to_screen(end, ctx.canvas_center),
                egui::Color32::from_rgba_unmultiplied(120, 220, 255, alpha),
                1.5,
            );
        }
    }
}

/// Y offset per dessert variant (from prefab BoxCollider center.y).
pub(in crate::renderer::sprites) fn dessert_y_offset(name: &str) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::selection_outline_metrics;
    use crate::domain::types::Vec3;
    use crate::renderer::Camera;

    use super::super::data::SpriteDrawData;
    use crate::data::goal_animation::GoalAnimationState;

    fn test_sprite(name: &str) -> SpriteDrawData {
        SpriteDrawData {
            world_pos: Vec3 {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            },
            color: egui::Color32::WHITE,
            half_size: (1.0, 2.0),
            scale: (1.0, 1.0),
            name: name.to_string(),
            index: 0,
            is_terrain: false,
            atlas: None,
            uv: None,
            is_hidden: false,
            parent: None,
            override_text: None,
            rotation: 0.0,
            bird_phase: 0.0,
            name_lower: name.to_ascii_lowercase(),
            goal_animation_state: GoalAnimationState::Idle,
        }
    }

    #[test]
    fn goal_area_selection_outline_ignores_animated_center() {
        let camera = Camera::default();
        let sprite = test_sprite("GoalArea_Night");
        let canvas_center = egui::vec2(100.0, 100.0);
        let animated_center = egui::pos2(130.0, 170.0);
        let (sel_center, sel_hw, sel_hh) =
            selection_outline_metrics(&sprite, &camera, canvas_center, animated_center, 5.0, 6.0);

        let expected_center = camera.world_to_screen(
            crate::domain::types::Vec2 { x: 10.0, y: 20.0 },
            canvas_center,
        );
        assert_eq!(sel_center, expected_center);
        assert_eq!(sel_hw, 40.0);
        assert_eq!(sel_hh, 80.0);
    }

    #[test]
    fn non_goal_selection_outline_keeps_animated_center() {
        let camera = Camera::default();
        let sprite = test_sprite("BoxChallenge");
        let canvas_center = egui::vec2(100.0, 100.0);
        let animated_center = egui::pos2(130.0, 170.0);
        let (sel_center, sel_hw, sel_hh) =
            selection_outline_metrics(&sprite, &camera, canvas_center, animated_center, 5.0, 6.0);

        assert_eq!(sel_center, animated_center);
        assert_eq!(sel_hw, 5.0);
        assert_eq!(sel_hh, 6.0);
    }
}
