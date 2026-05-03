//! Contraption preview rendering — used by the save-file viewer to draw a
//! grid-based preview of a `.contraption` blueprint.

use eframe::egui;

use crate::data::assets::TextureCache;
use crate::data::icon_db;
use crate::renderer::grid::ConstructionGridCellStyle;
use crate::renderer::{LevelRenderer, sprite_shader};
use crate::io::save::parser::ContraptionPart;
use crate::data::sprite_db::UvRect;

/// Part type integer to display name.
fn part_type_name(part_type: i32) -> &'static str {
    match part_type {
        0 => "Unknown",
        1 => "Balloon",
        2 => "Balloons2",
        3 => "Balloons3",
        4 => "Fan",
        5 => "WoodenFrame",
        6 => "Bellows",
        7 => "CartWheel",
        8 => "Basket",
        9 => "Sandbag",
        10 => "Pig",
        11 => "Sandbag2",
        12 => "Sandbag3",
        13 => "Propeller",
        14 => "Wings",
        15 => "Tailplane",
        16 => "Engine",
        17 => "Rocket",
        18 => "MetalFrame",
        19 => "SmallWheel",
        20 => "MetalWing",
        21 => "MetalTail",
        22 => "Rotor",
        23 => "MotorWheel",
        24 => "TNT",
        25 => "EngineSmall",
        26 => "EngineBig",
        27 => "NormalWheel",
        28 => "Spring",
        29 => "Umbrella",
        30 => "Rope",
        31 => "CokeBottle",
        32 => "KingPig",
        33 => "RedRocket",
        34 => "SodaBottle",
        35 => "PoweredUmbrella",
        36 => "Egg",
        37 => "JetEngine",
        38 => "ObsoleteWheel",
        39 => "SpringBoxingGlove",
        40 => "StickyWheel",
        41 => "GrapplingHook",
        42 => "Pumpkin",
        43 => "Kicker",
        44 => "Gearbox",
        45 => "GoldenPig",
        46 => "PointLight",
        47 => "SpotLight",
        48 => "TimeBomb",
        _ => "?",
    }
}

/// Unity ChangeVisualConnections: determine if a layer should be visible based
/// on the part type, grid rotation, and neighboring parts.
///
/// Part types with overrides:
///   14 Wings          – TopFrameSprite / BottomFrameSprite (no rotation)
///   17 Rocket         – 4 cardinal attachments + bottom fallback (rotation-aware)
///   24 TNT            – 4 cardinal attachments, simple (rotation-aware)
///   39 SpringBoxingGlove – always hide SpringVisualization
///   41 GrapplingHook   – 8-directional attachments + hide SpringVisualization
///   47 SpotLight       – 8-directional attachments
fn layer_visible(
    part: &ContraptionPart,
    layer: &icon_db::IconLayer,
    _occupied: &std::collections::HashSet<(i32, i32)>,
    rotated_has: &dyn Fn(i32, i32, i32, i32) -> bool,
    has_neighbor: &dyn Fn(i32, i32, i32, i32) -> bool,
) -> bool {
    let name = layer.go_name.as_str();
    let (x, y, rot) = (part.x, part.y, part.rot);

    match part.part_type {
        // --- Wings (14): no rotation applied to directions ---
        14 => {
            let has_up = has_neighbor(x, y, 0, 1);
            let has_down = has_neighbor(x, y, 0, -1);
            let has_left = has_neighbor(x, y, -1, 0);
            let has_right = has_neighbor(x, y, 1, 0);
            match name {
                "TopFrameSprite" => has_up || has_left || has_right,
                "BottomFrameSprite" => {
                    let top_connected = has_up || has_left || has_right;
                    let bot_connected = has_down || has_left || has_right;
                    bot_connected || !top_connected
                }
                _ => true,
            }
        }
        // --- Rocket (17): 4 cardinal, rotation-aware, bottom fallback ---
        17 => {
            // Direction enum: Right=0, Up=1, Left=2, Down=3
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            match name {
                "LeftAttachment" => has_left,
                "RightAttachment" => has_right,
                "TopAttachment" => has_up,
                "BottomAttachment" => has_down || (!has_up && !has_left && !has_right),
                _ => true,
            }
        }
        // --- TNT (24): 4 cardinal, rotation-aware, simple ---
        24 => {
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            match name {
                "LeftAttachment" => has_left,
                "RightAttachment" => has_right,
                "TopAttachment" => has_up,
                "BottomAttachment" => has_down,
                _ => true,
            }
        }
        // --- SpringBoxingGlove (39): always hide spring ---
        39 => name != "SpringVisualization",
        // --- GrapplingHook (41) & SpotLight (47): 8-directional ---
        41 | 47 => {
            // SpringVisualization always hidden for GrapplingHook
            if part.part_type == 41 && name == "SpringVisualization" {
                return false;
            }
            // Cardinal neighbor flags (rotation-aware)
            let has_up = rotated_has(x, y, 1, rot);
            let has_down = rotated_has(x, y, 3, rot);
            let has_left = rotated_has(x, y, 2, rot);
            let has_right = rotated_has(x, y, 0, rot);
            // Diagonal neighbor flags: Rotate(DiagDir, rot) % 4 maps to cardinal for CanConnectTo
            // UpRight=4, UpLeft=5, DownLeft=6, DownRight=7
            let has_up_left = rotated_has(x, y, 5, rot);
            let has_down_left = rotated_has(x, y, 6, rot);
            let has_up_right = rotated_has(x, y, 4, rot);
            let has_down_right = rotated_has(x, y, 7, rot);
            // Is this a diagonal rotation? GridRotation: Deg_45=4, Deg_135=5, Deg_225=6, Deg_315=7
            let diag = rot >= 4 && rot <= 7;
            let visible = match name {
                "LeftAttachment" => has_left && !diag,
                "RightAttachment" => has_right && !diag,
                "TopAttachment" => has_up && !diag,
                "BottomAttachment" => (has_down && !diag) || (!has_up && !has_left && !has_right && !diag),
                "BottomLeftAttachment" => has_down_left && diag,
                "BottomRightAttachment" => has_down_right && diag,
                "TopLeftAttachment" => has_up_left && diag,
                "TopRightAttachment" => has_up_right && diag,
                _ => return true,
            };
            // Global fallback: if absolutely no connections, show bottom
            if !visible && name == "BottomAttachment" {
                let any = has_up || has_down_left || has_left || has_right
                    || has_up_left || has_up_right || has_down_right;
                if !any {
                    return true;
                }
            }
            visible
        }
        _ => true,
    }
}

/// Render a grid-based contraption preview using part icons from the sprite atlas.
pub(super) fn render_contraption_canvas(
    ui: &mut egui::Ui,
    parts: &[ContraptionPart],
    tex_cache: &mut TextureCache,
    renderer: &mut LevelRenderer,
) {
    let cell_size = 48.0_f32; // pixels per grid cell

    // Compute grid bounds
    let min_x = parts.iter().map(|p| p.x).min().unwrap_or(0);
    let max_x = parts.iter().map(|p| p.x).max().unwrap_or(0);
    let min_y = parts.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = parts.iter().map(|p| p.y).max().unwrap_or(0);
    let grid_w = (max_x - min_x + 1) as f32;
    let grid_h = (max_y - min_y + 1) as f32;
    let canvas_w = grid_w * cell_size;
    let canvas_h = grid_h * cell_size;

    // Center the grid in the available space
    let avail = ui.available_size();
    let scale = (avail.x / canvas_w).min(avail.y / canvas_h).min(1.5);
    let scaled_cell = cell_size * scale;
    let total_w = grid_w * scaled_cell;
    let total_h = grid_h * scaled_cell;

    let (response, painter) = ui.allocate_painter(
        egui::vec2(total_w.max(avail.x), total_h.max(avail.y)),
        egui::Sense::hover(),
    );
    let origin = egui::pos2(
        response.rect.center().x - total_w * 0.5,
        response.rect.center().y - total_h * 0.5,
    );
    let grid_style = if ui.visuals().dark_mode {
        ConstructionGridCellStyle::Light
    } else {
        ConstructionGridCellStyle::Default
    };

    let gpu_resources = renderer.preview_sprite_resources();
    let mut gpu_draws: Vec<sprite_shader::SpriteBatchDraw> = Vec::new();

    let draw_line_grid = || {
        let grid_line_color = ui
            .visuals()
            .widgets
            .noninteractive
            .bg_stroke
            .color
            .linear_multiply(0.3);
        for gx in 0..=(grid_w as i32) {
            let x = origin.x + gx as f32 * scaled_cell;
            painter.line_segment(
                [egui::pos2(x, origin.y), egui::pos2(x, origin.y + total_h)],
                egui::Stroke::new(1.0, grid_line_color),
            );
        }
        for gy in 0..=(grid_h as i32) {
            let y = origin.y + gy as f32 * scaled_cell;
            painter.line_segment(
                [egui::pos2(origin.x, y), egui::pos2(origin.x + total_w, y)],
                egui::Stroke::new(1.0, grid_line_color),
            );
        }
    };

    // The save preview uses the light construction-cell variant in dark UI,
    // matching the in-game construction background more closely.
    if let Some(sprite) = crate::data::sprite_db::get_sprite_info(grid_style.sprite_name()) {
        let (half_w, half_h) = grid_style.half_extents();
        let grid_sprite_w = scaled_cell * half_w * 2.0;
        let grid_sprite_h = scaled_cell * half_h * 2.0;
        if gpu_resources.is_some()
            && let Some(atlas) = renderer.preview_sprite_atlas(&sprite.atlas)
        {
            let (uv_min, uv_max) = sprite_shader::compute_uvs(
                &sprite.uv,
                atlas.width as f32,
                atlas.height as f32,
                false,
                false,
            );

            for gx in 0..(grid_w as i32) {
                for gy in 0..(grid_h as i32) {
                    let center = egui::pos2(
                        origin.x + (gx as f32 + 0.5) * scaled_cell,
                        origin.y + (gy as f32 + 0.5) * scaled_cell,
                    );
                    gpu_draws.push(sprite_shader::SpriteBatchDraw {
                        atlas: atlas.clone(),
                        slot: gpu_draws.len() as u32,
                        uniforms: sprite_shader::SpriteUniforms {
                            screen_size: [response.rect.width(), response.rect.height()],
                            camera_center: [0.0, 0.0],
                            zoom: 1.0,
                            rotation: 0.0,
                            world_center: [
                                center.x - response.rect.center().x,
                                response.rect.center().y - center.y,
                            ],
                            half_size: [grid_sprite_w * 0.5, grid_sprite_h * 0.5],
                            uv_min,
                            uv_max,
                            mode: 0.0,
                            shine_center: 0.0,
                            tint_color: [1.0, 1.0, 1.0, 1.0],
                        },
                    });
                }
            }
        } else {
            let atlas_path = format!("sprites/{}", sprite.atlas);
            let tex_id = tex_cache.load_sprite_crop(
                ui.ctx(),
                grid_style.texture_cache_key(),
                &atlas_path,
                [sprite.uv.x, sprite.uv.y, sprite.uv.w, sprite.uv.h],
            );
            if let Some(tex_id) = tex_id {
                let uv_rect =
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                let tint = egui::Color32::WHITE;
                for gx in 0..(grid_w as i32) {
                    for gy in 0..(grid_h as i32) {
                        let center = egui::pos2(
                            origin.x + (gx as f32 + 0.5) * scaled_cell,
                            origin.y + (gy as f32 + 0.5) * scaled_cell,
                        );
                        let cell_rect = egui::Rect::from_center_size(
                            center,
                            egui::vec2(grid_sprite_w, grid_sprite_h),
                        );
                        let mut mesh = egui::Mesh::with_texture(tex_id);
                        mesh.add_rect_with_uv(cell_rect, uv_rect, tint);
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
            } else {
                draw_line_grid();
            }
        }
    } else {
        draw_line_grid();
    }

    // Sort parts by Z depth for correct draw order (back-to-front).
    // Unity formula: Z = -0.1 + m_ZOffset - (x + 2*y) / 100000
    // Camera looks along -Z, so more positive world Z = farther = drawn first.
    // Within each part, child layers have z_local offsets that add to the part Z.
    // Unity sorts ALL mesh renderers globally (interleaved across parts), not
    // per-part.  We replicate this by collecting every layer from every part,
    // computing its world Z, sorting descending (farthest first), and drawing
    // in that order.

    // Build a grid occupancy set for neighbor checking (ChangeVisualConnections).
    let mut occupied: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
    for p in parts {
        occupied.insert((p.x, p.y));
    }
    // Unity: Rotate(dir, rot) = (dir + rot) % 4  →  Direction enum order: Right=0 Up=1 Left=2 Down=3
    // Maps rotated direction to grid (dx,dy).
    const DIR_OFFSETS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let rotated_has = |x: i32, y: i32, dir: i32, rot: i32| -> bool {
        let (dx, dy) = DIR_OFFSETS[((dir + rot) % 4) as usize];
        occupied.contains(&(x + dx, y + dy))
    };
    let has_neighbor =
        |x: i32, y: i32, dx: i32, dy: i32| -> bool { occupied.contains(&(x + dx, y + dy)) };

    struct DrawLayer<'a> {
        part: &'a ContraptionPart,
        layer: &'a icon_db::IconLayer,
        world_z: f32,
    }

    let mut draw_layers: Vec<DrawLayer> = Vec::new();

    for part in parts {
        let part_info = icon_db::get_part_info(part.part_type, part.custom_part_index);
        if let Some(info) = part_info {
            let part_z = -0.1_f32 + info.z_offset
                - (part.x as f32 + 2.0 * part.y as f32) / 100000.0;
            for layer in &info.layers {
                if !layer_visible(part, layer, &occupied, &rotated_has, &has_neighbor) {
                    continue;
                }

                let world_z = part_z + layer.z_local;
                draw_layers.push(DrawLayer {
                    part,
                    layer,
                    world_z,
                });
            }
        }
    }

    // Sort descending by world Z: farthest (most positive) drawn first (behind),
    // nearest (most negative) drawn last (on top).
    draw_layers.sort_by(|a, b| {
        b.world_z
            .partial_cmp(&a.world_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Draw each layer
    for dl in &draw_layers {
        let part = dl.part;
        let layer = dl.layer;
        let gx = (part.x - min_x) as f32;
        let gy = (max_y - part.y) as f32; // flip Y
        let cell_center = egui::pos2(
            origin.x + (gx + 0.5) * scaled_cell,
            origin.y + (gy + 0.5) * scaled_cell,
        );

        let fixed_scale = scaled_cell;

        // v0..v3 order from TOML: BL, TL, TR, BR
        let mut verts = [
            (layer.v0_x, layer.v0_y),
            (layer.v1_x, layer.v1_y),
            (layer.v2_x, layer.v2_y),
            (layer.v3_x, layer.v3_y),
        ];

        // Apply part-level flip/rotation.
        // Unity uses if/else: flipped parts don't get grid rotation applied.
        for (x, y) in &mut verts {
            let mut px = *x;
            let py = *y;

            if part.flipped {
                // 180° Y-axis rotation: negate X (Z negation handled separately)
                px = -px;
            } else {
                let rot = part.rot % 4;
                let (rx, ry) = match rot {
                    1 => (-py, px),
                    2 => (-px, -py),
                    3 => (py, -px),
                    _ => (px, py),
                };
                px = rx;
                *y = ry;
            }

            *x = px;
        }

        let to_screen = |vx: f32, vy: f32| {
            egui::pos2(
                cell_center.x + vx * fixed_scale,
                cell_center.y - vy * fixed_scale, // world Y-up -> screen Y-down
            )
        };

        let screen_positions = [
            to_screen(verts[0].0, verts[0].1),
            to_screen(verts[1].0, verts[1].1),
            to_screen(verts[2].0, verts[2].1),
            to_screen(verts[3].0, verts[3].1),
        ];

        // Match the main sprite_shader path: Unity V-flip + half-texel inset to
        // avoid atlas bleeding. Geometry flip/rotation is already applied to
        // vertices above, so UV flip flags stay false here.
        let uv_rect = UvRect {
            x: layer.uv_x,
            y: layer.uv_y,
            w: layer.uv_w,
            h: layer.uv_h,
        };

        if gpu_resources.is_some()
            && let Some(atlas) = renderer.preview_sprite_atlas(&layer.atlas)
        {
            let to_shader_space = |p: egui::Pos2| -> [f32; 2] {
                [
                    p.x - response.rect.center().x,
                    response.rect.center().y - p.y,
                ]
            };
            let bl = to_shader_space(screen_positions[0]);
            let tl = to_shader_space(screen_positions[1]);
            let tr = to_shader_space(screen_positions[2]);
            let br = to_shader_space(screen_positions[3]);

            let center = [
                (bl[0] + tl[0] + tr[0] + br[0]) * 0.25,
                (bl[1] + tl[1] + tr[1] + br[1]) * 0.25,
            ];
            let mut x_axis = [br[0] - bl[0], br[1] - bl[1]];
            let y_axis = [tl[0] - bl[0], tl[1] - bl[1]];

            let mut flip_x = false;
            let det = x_axis[0] * y_axis[1] - x_axis[1] * y_axis[0];
            if det < 0.0 {
                x_axis = [-x_axis[0], -x_axis[1]];
                flip_x = true;
            }

            let half_w = x_axis[0].hypot(x_axis[1]) * 0.5;
            let half_h = y_axis[0].hypot(y_axis[1]) * 0.5;
            if half_w > 0.0 && half_h > 0.0 {
                let rotation = x_axis[1].atan2(x_axis[0]);
                let (uv_min, uv_max) = sprite_shader::compute_uvs(
                    &uv_rect,
                    atlas.width as f32,
                    atlas.height as f32,
                    flip_x,
                    false,
                );
                gpu_draws.push(sprite_shader::SpriteBatchDraw {
                    atlas,
                    slot: gpu_draws.len() as u32,
                    uniforms: sprite_shader::SpriteUniforms {
                        screen_size: [response.rect.width(), response.rect.height()],
                        camera_center: [0.0, 0.0],
                        zoom: 1.0,
                        rotation,
                        world_center: center,
                        half_size: [half_w, half_h],
                        uv_min,
                        uv_max,
                        mode: 0.0,
                        shine_center: 0.0,
                        tint_color: [1.0, 1.0, 1.0, 1.0],
                    },
                });
                continue;
            }
        }

        let atlas_path = format!("sprites/{}", layer.atlas);
        let tex_id = match tex_cache.load_texture(ui.ctx(), &atlas_path, &layer.atlas) {
            Some(id) => id,
            None => continue,
        };
        let [atlas_w, atlas_h] = match tex_cache.texture_size(&layer.atlas) {
            Some(size) => size,
            None => continue,
        };
        let (uv_min, uv_max) = sprite_shader::compute_uvs(
            &uv_rect,
            atlas_w as f32,
            atlas_h as f32,
            false,
            false,
        );
        let uv_bl = egui::pos2(uv_min[0], uv_max[1]);
        let uv_tl = egui::pos2(uv_min[0], uv_min[1]);
        let uv_tr = egui::pos2(uv_max[0], uv_min[1]);
        let uv_br = egui::pos2(uv_max[0], uv_max[1]);

        let mesh = part_icon_mesh_quad(tex_id, screen_positions, [uv_bl, uv_tl, uv_tr, uv_br]);
        painter.add(egui::Shape::mesh(mesh));
    }

    if let Some(resources) = gpu_resources
        && !gpu_draws.is_empty()
    {
        painter.add(sprite_shader::make_sprite_batch_callback(
            response.rect,
            resources,
            gpu_draws,
        ));
    }

    // Tooltips (based on grid cell hover)
    if let Some(pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
        let mut hovered_parts: Vec<String> = Vec::new();
        for part in parts {
            let gx = (part.x - min_x) as f32;
            let gy = (max_y - part.y) as f32;
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + gx * scaled_cell, origin.y + gy * scaled_cell),
                egui::vec2(scaled_cell, scaled_cell),
            );
            if cell_rect.contains(pos) {
                let name = part_type_name(part.part_type);
                hovered_parts.push(format!(
                    "{name} ({}, {})  rot={} flipped={}",
                    part.x, part.y, part.rot, part.flipped
                ));
            }
        }

        if !hovered_parts.is_empty() {
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                ui.id().with("part_tip"),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                for line in &hovered_parts {
                    ui.label(line);
                }
            });
        }
    }
}

/// Build a textured quad mesh with arbitrary vertex positions and UV corners.
/// Vertex order must be BL, TL, TR, BR to match Unity mesh order.
fn part_icon_mesh_quad(
    tex_id: egui::TextureId,
    positions: [egui::Pos2; 4],
    uvs: [egui::Pos2; 4],
) -> egui::Mesh {
    let mut mesh = egui::Mesh::with_texture(tex_id);
    let white = egui::Color32::WHITE;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[0],
        uv: uvs[0],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[1],
        uv: uvs[1],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[2],
        uv: uvs[2],
        color: white,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: positions[3],
        uv: uvs[3],
        color: white,
    });
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    mesh
}

