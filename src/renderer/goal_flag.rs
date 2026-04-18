//! Goal flag mesh rendering — UV-scroll animated flag drawn from pre-transformed Unity mesh data.

use eframe::egui;

use crate::types::*;

use super::Camera;
use super::sprites::SpriteDrawData;

/// 33 vertices as (x, y) pairs — pre-transformed from Unity GoalSprite mesh.
/// World-space offsets from GoalArea position. Range: X [-0.65, 0.65], Y [-1.32, 1.32].
#[rustfmt::skip]
const GOAL_FLAG_POS: &[f32] = &[
    0.639989, 0.000000,
    0.597152, -0.006390,
    0.597152, 1.308977,
    0.639989, 1.321756,
    0.597152, -1.321756,
    0.639989, -1.321756,
    0.647497, -0.991000,
    0.650000, 0.001267,
    0.647497, 0.992900,
    0.488666, -0.023022,
    0.488666, -1.321756,
    0.299633, -1.321756,
    0.299633, -0.033104,
    0.015157, -0.019842,
    0.015157, -1.321756,
    -0.273813, -1.321756,
    -0.273813, -0.004341,
    -0.476330, -0.007708,
    -0.476330, -1.321756,
    -0.597596, -1.321756,
    -0.597596, -0.017464,
    -0.597596, 1.286827,
    -0.476330, 1.306_34,
    -0.273813, 1.313074,
    0.015157, 1.282072,
    0.299633, 1.255547,
    -0.642811, -0.021132,
    -0.642811, -1.321756,
    -0.648203, -0.996756,
    -0.650000, -0.021756,
    -0.648203, 0.953556,
    -0.642811, 1.279491,
    0.488666, 1.275711,
];

/// 33 base UVs as (u, v) pairs. U will be flipped (1-u) due to Unity Euler(180,0,90).
/// V coordinates extend beyond [0,1] — texture needs repeat wrapping.
#[rustfmt::skip]
const GOAL_FLAG_UVS_BASE: &[f32] = &[
    0.992299, 1.123081,
    0.956771, 1.126739,
    0.956771, 0.007316,
    0.992299, 0.000000,
    0.956771, 2.246162,
    0.992299, 2.246162,
    0.998075, 1.965272,
    1.000000, 1.122604,
    0.998075, 0.280173,
    0.865590, 1.136757,
    0.865590, 2.246162,
    0.713230, 2.246162,
    0.713230, 1.143056,
    0.494167, 1.135558,
    0.494167, 2.246162,
    0.276019, 2.246162,
    0.276019, 1.125967,
    0.126406, 1.125986,
    0.126406, 2.246162,
    0.038514, 2.246162,
    0.038514, 1.129662,
    0.038514, 0.013162,
    0.126406, 0.005809,
    0.276019, 0.005772,
    0.494167, 0.024954,
    0.713230, 0.039950,
    0.005530, 1.131044,
    0.005530, 2.246162,
    0.001383, 1.967441,
    0.000000, 1.131279,
    0.001383, 0.295000,
    0.005530, 0.015926,
    0.865590, 0.027351,
];

/// 40 triangles (120 indices).
#[rustfmt::skip]
const GOAL_FLAG_IDX: &[u32] = &[
    0, 1, 2,   0, 2, 3,   0, 4, 1,   0, 5, 4,
    0, 6, 5,   0, 7, 6,   0, 3, 8,   0, 8, 7,
    9, 1, 4,   9, 2, 1,   9, 4, 10,  9, 10, 11,
    9, 11, 12, 13, 12, 11, 13, 11, 14, 13, 14, 15,
    13, 15, 16, 17, 16, 15, 17, 15, 18, 17, 18, 19,
    17, 19, 20, 17, 20, 21, 17, 21, 22, 17, 22, 23,
    17, 23, 16, 13, 16, 23, 13, 23, 24, 13, 24, 25,
    13, 25, 12, 26, 20, 19, 26, 21, 20, 26, 19, 27,
    26, 27, 28, 26, 28, 29, 26, 29, 30, 26, 30, 31,
    26, 31, 21, 9, 12, 25, 9, 25, 32, 9, 32, 2,
];

/// Draw the Goal flag mesh with UV scroll animation.
pub(super) fn draw_goal_flag(
    painter: &egui::Painter,
    sprite: &SpriteDrawData,
    camera: &Camera,
    canvas_center: egui::Vec2,
    canvas_rect: egui::Rect,
    time: f64,
    tex_id: egui::TextureId,
) {
    // Flag mesh is stationary (no bobbing); only the GoalArea sprite bobs.
    // The flag gets UV-scroll animation only, matching the TS editor.
    let base_x = sprite.world_pos.x;
    let base_y = sprite.world_pos.y;

    // Quick frustum cull (flag is ~1.3 x 2.65 world units)
    let center_screen = camera.world_to_screen(
        Vec2 {
            x: base_x,
            y: base_y,
        },
        canvas_center,
    );
    let margin = 1.4 * camera.zoom;
    if center_screen.x + margin < canvas_rect.left()
        || center_screen.x - margin > canvas_rect.right()
        || center_screen.y + margin < canvas_rect.top()
        || center_screen.y - margin > canvas_rect.bottom()
    {
        return;
    }

    // UV scroll: V offset advances at 0.25 units/sec
    let v_offset = (time * 0.25 % 1.0) as f32;

    let num_verts = GOAL_FLAG_POS.len() / 2;
    let mut mesh = egui::Mesh::with_texture(tex_id);
    mesh.vertices.reserve(num_verts);

    for i in 0..num_verts {
        let wx = base_x + GOAL_FLAG_POS[i * 2];
        let wy = base_y + GOAL_FLAG_POS[i * 2 + 1];
        let screen_pos = camera.world_to_screen(Vec2 { x: wx, y: wy }, canvas_center);

        // UV: flip U (1-u) due to Unity rotation, scroll V
        // V coords use original Unity convention (V=0=bottom); texture is loaded
        // with vertical flip to match, so GPU repeat wrapping handles values > 1.
        let u = 1.0 - GOAL_FLAG_UVS_BASE[i * 2];
        let v = GOAL_FLAG_UVS_BASE[i * 2 + 1] + v_offset;

        mesh.vertices.push(egui::epaint::Vertex {
            pos: screen_pos,
            uv: egui::pos2(u, v),
            color: egui::Color32::WHITE,
        });
    }

    mesh.indices.reserve(GOAL_FLAG_IDX.len());
    for &idx in GOAL_FLAG_IDX {
        mesh.indices.push(idx);
    }

    painter.add(egui::Shape::mesh(mesh));
}
