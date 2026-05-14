//! `draw_sprite` and helpers.

use eframe::egui;

use crate::domain::types::*;
use crate::goal_animation::goal_visual_state;

use super::super::DrawCtx;
use super::{bird_sleep_scale_factors, bird_sleep_y_offset, SpriteDrawData, SpriteDrawOpts};

fn with_alpha(color: egui::Color32, alpha: f32) -> egui::Color32 {
    let scaled_alpha = ((color.a() as f32) * alpha.clamp(0.0, 1.0)).round() as u8;
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), scaled_alpha)
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
