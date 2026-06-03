//! HUD overlays, selection visuals, terrain wireframe, grid, and lazy texture loading.

use std::collections::BTreeSet;

use crate::data::assets;
use crate::domain::types::{ObjectIndex, Vec2};

use super::{ATLAS_FILES, CursorMode, GLOW_ATLAS, GOAL_FLAG_TEXTURE, LevelRenderer, particles};

fn circle_center_from_three_points(p0: Vec2, p1: Vec2, p2: Vec2) -> Option<Vec2> {
    let x1 = p0.x;
    let y1 = p0.y;
    let x2 = p1.x;
    let y2 = p1.y;
    let x3 = p2.x;
    let y3 = p2.y;

    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-6 {
        return None;
    }

    let x1_sq_y1_sq = x1 * x1 + y1 * y1;
    let x2_sq_y2_sq = x2 * x2 + y2 * y2;
    let x3_sq_y3_sq = x3 * x3 + y3 * y3;

    let ux = (x1_sq_y1_sq * (y2 - y3) + x2_sq_y2_sq * (y3 - y1) + x3_sq_y3_sq * (y1 - y2)) / d;
    let uy = (x1_sq_y1_sq * (x3 - x2) + x2_sq_y2_sq * (x1 - x3) + x3_sq_y3_sq * (x2 - x1)) / d;
    Some(Vec2 { x: ux, y: uy })
}

fn angle_delta_ccw(from: f32, to: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (to - from).rem_euclid(tau)
}

fn sample_circular_arc_preview(
    p0: Vec2,
    through: Vec2,
    p2: Vec2,
    nodes: usize,
) -> Option<Vec<Vec2>> {
    let center = circle_center_from_three_points(p0, through, p2)?;
    let radius = ((p0.x - center.x).powi(2) + (p0.y - center.y).powi(2)).sqrt();
    if radius < 1e-6 {
        return None;
    }

    let a0 = (p0.y - center.y).atan2(p0.x - center.x);
    let a1 = (through.y - center.y).atan2(through.x - center.x);
    let a2 = (p2.y - center.y).atan2(p2.x - center.x);

    let sweep = {
        let ccw_01 = angle_delta_ccw(a0, a1);
        let ccw_02 = angle_delta_ccw(a0, a2);
        if ccw_01 <= ccw_02 {
            ccw_02
        } else {
            -angle_delta_ccw(a2, a0)
        }
    };

    let n = nodes.clamp(3, 256);
    let mut points = Vec::with_capacity(n);
    let denom = (n - 1) as f32;
    for i in 0..n {
        let t = i as f32 / denom;
        let angle = a0 + sweep * t;
        points.push(Vec2 {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
        });
    }
    Some(points)
}

impl LevelRenderer {
    /// Draw terrain triangulation wireframe overlay.
    pub(super) fn draw_terrain_wireframe(
        &self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
    ) {
        let tri_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 255, 128, 160));
        for td in self.terrain_data.iter() {
            if let Some(ref fill) = td.fill_mesh {
                let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
                let verts = &fill.vertices;
                let indices = &fill.indices;
                let tri_count = indices.len() / 3;
                for t in 0..tri_count {
                    let i0 = indices[t * 3] as usize;
                    let i1 = indices[t * 3 + 1] as usize;
                    let i2 = indices[t * 3 + 2] as usize;
                    if i0 >= verts.len() || i1 >= verts.len() || i2 >= verts.len() {
                        continue;
                    }
                    let p0 = self.camera.world_to_screen(
                        Vec2 {
                            x: verts[i0].pos.x + tdx,
                            y: verts[i0].pos.y + tdy,
                        },
                        canvas_center,
                    );
                    let p1 = self.camera.world_to_screen(
                        Vec2 {
                            x: verts[i1].pos.x + tdx,
                            y: verts[i1].pos.y + tdy,
                        },
                        canvas_center,
                    );
                    let p2 = self.camera.world_to_screen(
                        Vec2 {
                            x: verts[i2].pos.x + tdx,
                            y: verts[i2].pos.y + tdy,
                        },
                        canvas_center,
                    );
                    painter.line_segment([p0, p1], tri_stroke);
                    painter.line_segment([p1, p2], tri_stroke);
                    painter.line_segment([p2, p0], tri_stroke);
                }
            }
        }
    }

    /// Draw selection outlines and node handles for terrain curves.
    pub(super) fn draw_terrain_selection(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        selected: &BTreeSet<ObjectIndex>,
    ) {
        self.hovered_terrain_node = None;
        for td in self.terrain_data.iter() {
            if selected.contains(&td.object_index) && td.curve_world_verts.len() >= 2 {
                let (tdx, tdy) = self.terrain_drag_offset(td.object_index);
                let screen_pts: Vec<egui::Pos2> = td
                    .curve_world_verts
                    .iter()
                    .map(|&(wx, wy)| {
                        self.camera.world_to_screen(
                            Vec2 {
                                x: wx + tdx,
                                y: wy + tdy,
                            },
                            canvas_center,
                        )
                    })
                    .collect();
                let stroke = egui::Stroke::new(2.0, egui::Color32::YELLOW);
                for pair in screen_pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }

                // Node handles: draw small squares at each curve node position
                const NODE_RADIUS: f32 = 4.0;
                const HOVER_RADIUS: f32 = 8.0;

                // Find hovered node (closest within threshold)
                if let Some(mouse_w) = self.mouse_world {
                    let mouse_wx = mouse_w.x - tdx;
                    let mouse_wy = mouse_w.y - tdy;
                    let threshold = HOVER_RADIUS / self.camera.zoom;
                    let mut best_dist = threshold;
                    let mut best_idx = None;
                    for (i, &(nx, ny)) in td.curve_world_verts.iter().enumerate() {
                        let dx = nx - mouse_wx;
                        let dy = ny - mouse_wy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = Some(i);
                        }
                    }
                    if let Some(idx) = best_idx {
                        self.hovered_terrain_node = Some((td.object_index, idx));
                    }
                }

                for (i, &pt) in screen_pts.iter().enumerate() {
                    let is_hovered = self.hovered_terrain_node == Some((td.object_index, i));
                    let tex_idx = td.node_textures.get(i).copied().unwrap_or(0);
                    let base_color = if tex_idx == 1 {
                        egui::Color32::from_rgb(0x99, 0x66, 0x33) // brown = outline/splat1
                    } else {
                        egui::Color32::from_rgb(0x70, 0xB0, 0x30) // green = grass/splat0
                    };
                    let fill = if is_hovered {
                        egui::Color32::WHITE
                    } else {
                        base_color
                    };
                    let r = if is_hovered {
                        NODE_RADIUS + 2.0
                    } else {
                        NODE_RADIUS
                    };
                    let node_rect = egui::Rect::from_center_size(pt, egui::vec2(r * 2.0, r * 2.0));
                    painter.rect_filled(node_rect, 1.0, fill);
                    painter.rect_stroke(
                        node_rect,
                        1.0,
                        egui::Stroke::new(1.0, egui::Color32::BLACK),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
    }

    /// Draw HUD overlays: origin axes, physics ground, level bounds, zoom info.
    pub(super) fn draw_hud(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        canvas_center: egui::Vec2,
        tr: &'static crate::i18n::locale::I18n,
    ) {
        // Origin axes
        let origin = self
            .camera
            .world_to_screen(Vec2 { x: 0.0, y: 0.0 }, canvas_center);
        if rect.contains(origin) {
            let axis_len = 30.0;
            painter.line_segment(
                [origin, egui::pos2(origin.x + axis_len, origin.y)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 80, 80)),
            );
            painter.line_segment(
                [origin, egui::pos2(origin.x, origin.y - axis_len)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 255, 80)),
            );
        }

        // Physics ground line
        if self.show_ground {
            const PHYSICS_GROUND_Y: f32 = -6.599;
            let ground_left = self.camera.world_to_screen(
                Vec2 {
                    x: -1000.0,
                    y: PHYSICS_GROUND_Y,
                },
                canvas_center,
            );
            let ground_right = self.camera.world_to_screen(
                Vec2 {
                    x: 1000.0,
                    y: PHYSICS_GROUND_Y,
                },
                canvas_center,
            );
            let left_x = ground_left.x.max(rect.left());
            let right_x = ground_right.x.min(rect.right());
            if left_x < right_x {
                let gy = ground_left.y;
                painter.line_segment(
                    [egui::pos2(left_x, gy), egui::pos2(right_x, gy)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 165, 0, 180)),
                );
                painter.text(
                    egui::pos2(rect.left() + 8.0, gy - 14.0),
                    egui::Align2::LEFT_TOP,
                    format!("Y = {:.3}", PHYSICS_GROUND_Y),
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(255, 165, 0, 200),
                );
                painter.text(
                    egui::pos2(right_x - 8.0, gy - 14.0),
                    egui::Align2::RIGHT_TOP,
                    tr.get("menu_physics_ground"),
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgba_unmultiplied(255, 165, 0, 200),
                );
            }
        }

        // Level bounds border (drawn on top of everything)
        if self.show_level_bounds
            && let Some([tl_x, tl_y, w, h]) = self.camera_limits
        {
            let min_x = tl_x;
            let max_x = tl_x + w;
            let min_y = tl_y - h;
            let max_y = tl_y;

            let p_tl = self
                .camera
                .world_to_screen(Vec2 { x: min_x, y: max_y }, canvas_center);
            let p_br = self
                .camera
                .world_to_screen(Vec2 { x: max_x, y: min_y }, canvas_center);
            let bounds_rect = egui::Rect::from_two_pos(p_tl, p_br);
            let clipped = bounds_rect.intersect(rect);
            if clipped.width() > 0.0 && clipped.height() > 0.0 {
                let is_dragging = self.bounds_dragging.is_some();
                let is_hovered = self.bounds_hovered_handle.is_some();
                let color = if is_dragging {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 100, 220)
                } else if is_hovered {
                    egui::Color32::from_rgba_unmultiplied(255, 220, 50, 220)
                } else {
                    egui::Color32::from_rgba_unmultiplied(255, 200, 0, 180)
                };
                let stroke_w = if is_dragging || is_hovered { 2.5 } else { 2.0 };
                let stroke = egui::Stroke::new(stroke_w, color);
                painter.rect_stroke(bounds_rect, 0.0, stroke, egui::StrokeKind::Inside);

                // Draw handle squares when hovered or dragging
                if is_hovered || is_dragging {
                    let hs = 4.0; // half-size of handle square
                    let handle_fill = egui::Color32::from_rgba_unmultiplied(255, 200, 0, 200);
                    let handle_stroke = egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(180, 140, 0, 255),
                    );
                    let cx = (bounds_rect.left() + bounds_rect.right()) / 2.0;
                    let cy = (bounds_rect.top() + bounds_rect.bottom()) / 2.0;
                    // Corners
                    for pos in [
                        bounds_rect.left_top(),
                        bounds_rect.right_top(),
                        bounds_rect.left_bottom(),
                        bounds_rect.right_bottom(),
                    ] {
                        let r = egui::Rect::from_center_size(pos, egui::vec2(hs * 2.0, hs * 2.0));
                        painter.rect(r, 0.0, handle_fill, handle_stroke, egui::StrokeKind::Inside);
                    }
                    // Edge midpoints
                    for pos in [
                        egui::pos2(cx, bounds_rect.top()),
                        egui::pos2(cx, bounds_rect.bottom()),
                        egui::pos2(bounds_rect.left(), cy),
                        egui::pos2(bounds_rect.right(), cy),
                    ] {
                        let r = egui::Rect::from_center_size(pos, egui::vec2(hs * 2.0, hs * 2.0));
                        painter.rect(r, 0.0, handle_fill, handle_stroke, egui::StrokeKind::Inside);
                    }
                }

                painter.text(
                    egui::pos2(bounds_rect.left() + 4.0, bounds_rect.top() - 16.0),
                    egui::Align2::LEFT_TOP,
                    tr.get("menu_level_bounds"),
                    egui::FontId::proportional(11.0),
                    color,
                );
                painter.text(
                    egui::pos2(bounds_rect.left() + 4.0, bounds_rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("({:.1}, {:.1}) {}x{}", tl_x, tl_y, w, h),
                    egui::FontId::proportional(11.0),
                    color,
                );
            }
        }

        // Zoom + theme info
        let theme_label = self
            .bg_theme
            .map(|s| s.to_owned())
            .unwrap_or_else(|| tr.get("hud_unknown_theme"));
        painter.text(
            rect.left_top() + egui::vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{}: {:.1}x  {}: {}",
                tr.get("hud_zoom"),
                self.camera.zoom,
                tr.get("hud_theme"),
                theme_label
            ),
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(150, 150, 150),
        );
    }

    /// Lazy-load atlas textures (only attempt once per atlas).
    pub(super) fn lazy_load_textures(&mut self, ctx: &egui::Context) {
        // Sprite atlases (sprites/, props/, and Texture2D/ subdirs)
        for atlas in ATLAS_FILES {
            if self.tex_cache.get(atlas).is_none() {
                let sprite_key = format!("Assets/Texture2D/{}", atlas);
                let props_key = format!("Assets/Texture2D/{}", atlas);
                if self
                    .tex_cache
                    .load_texture(ctx, &sprite_key, atlas)
                    .is_none()
                    && self
                        .tex_cache
                        .load_texture(ctx, &props_key, atlas)
                        .is_none()
                {
                    self.tex_cache
                        .load_texture(ctx, &format!("Assets/Texture2D/{}", atlas), atlas);
                }
            }
        }
        // Background atlases (bg/ subdir)
        for atlas in crate::data::bg_data::bg_atlas_files() {
            if self.tex_cache.get(atlas).is_none() {
                self.tex_cache
                    .load_texture(ctx, &format!("Assets/Texture2D/{}", atlas), atlas);
            }
        }
        // Sky textures (sky/ subdir)
        for sky in crate::data::bg_data::sky_texture_files() {
            if self.tex_cache.get(sky).is_none() {
                self.tex_cache
                    .load_texture(ctx, &format!("Assets/Texture2D/{}", sky), sky);
            }
        }
        // Terrain textures — usually Texture2D/ground, with a few Resources/* fallbacks.
        for td in &self.terrain_data {
            if let Some(ref tex_name) = td.fill_texture
                && self.tex_cache.get(tex_name).is_none()
                && let Some(asset_key) = assets::terrain_texture_asset_key(tex_name)
            {
                self.tex_cache
                    .load_texture_repeat(ctx, &asset_key, tex_name);
            }
        }
        // Goal flag texture (props/ subdir) — repeat wrap + flip V for UV scroll
        if self.tex_cache.get(GOAL_FLAG_TEXTURE).is_none() {
            self.tex_cache.load_texture_repeat_flipv(
                ctx,
                &format!("Assets/Texture2D/{}", GOAL_FLAG_TEXTURE),
                GOAL_FLAG_TEXTURE,
            );
        }
        for texture_name in [
            particles::zzz_particle_texture_name(),
            particles::fan_particle_texture_name(),
            particles::wind_particle_texture_name(),
        ]
        .into_iter()
        .flatten()
        {
            if self.tex_cache.get(texture_name).is_none() {
                self.tex_cache.load_texture(
                    ctx,
                    &format!("Assets/Texture2D/{}", texture_name),
                    texture_name,
                );
            }
        }

        if self.tex_cache.get(GLOW_ATLAS).is_none() {
            self.tex_cache.load_texture(
                ctx,
                &format!("Assets/Texture2D/{}", GLOW_ATLAS),
                GLOW_ATLAS,
            );
        }
    }

    /// Draw adaptive world grid overlay.
    pub(super) fn draw_grid(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        canvas_center: egui::Vec2,
    ) {
        let target_px = 60.0;
        let base = target_px / self.camera.zoom;
        let nice = [
            0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
        ];
        let grid_step = nice
            .iter()
            .copied()
            .min_by(|a, b| (a - base).abs().total_cmp(&(b - base).abs()))
            .unwrap_or(5.0);
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25);

        let tl = self.camera.screen_to_world(rect.left_top(), canvas_center);
        let br = self
            .camera
            .screen_to_world(rect.right_bottom(), canvas_center);

        let min_x = tl.x.min(br.x);
        let max_x = tl.x.max(br.x);
        let min_y = tl.y.min(br.y);
        let max_y = tl.y.max(br.y);

        let start_x = (min_x / grid_step).floor() as i32;
        let end_x = (max_x / grid_step).ceil() as i32;
        for ix in start_x..=end_x {
            let wx = ix as f32 * grid_step;
            let top = self
                .camera
                .world_to_screen(Vec2 { x: wx, y: max_y }, canvas_center);
            let bot = self
                .camera
                .world_to_screen(Vec2 { x: wx, y: min_y }, canvas_center);
            painter.line_segment([top, bot], egui::Stroke::new(0.5, color));
        }

        let start_y = (min_y / grid_step).floor() as i32;
        let end_y = (max_y / grid_step).ceil() as i32;
        for iy in start_y..=end_y {
            let wy = iy as f32 * grid_step;
            let left = self
                .camera
                .world_to_screen(Vec2 { x: min_x, y: wy }, canvas_center);
            let right = self
                .camera
                .world_to_screen(Vec2 { x: max_x, y: wy }, canvas_center);
            painter.line_segment([left, right], egui::Stroke::new(0.5, color));
        }
    }

    /// Draw tool-mode overlays: box-select rectangle, terrain draw preview.
    pub(super) fn draw_tool_overlay(
        &self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        cursor_mode: CursorMode,
    ) {
        match cursor_mode {
            CursorMode::BoxSelect => {
                // Draw the active box-selection rectangle
                if let Some(start) = self.box_select_start
                    && let Some(end) = painter.ctx().input(|i| i.pointer.latest_pos())
                {
                    let rect = egui::Rect::from_two_pos(start, end);
                    let fill = egui::Color32::from_rgba_unmultiplied(80, 140, 255, 40);
                    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 140, 255));
                    painter.rect(rect, 0.0, fill, stroke, egui::StrokeKind::Inside);
                }
            }
            CursorMode::DrawTerrain => {
                if let Some(preview_points) = self.terrain_preset_preview_points() {
                    let color = egui::Color32::from_rgb(100, 220, 100);
                    let points: Vec<egui::Pos2> = preview_points
                        .iter()
                        .map(|p| self.camera.world_to_screen(*p, canvas_center))
                        .collect();

                    for pair in points.windows(2) {
                        painter.line_segment([pair[0], pair[1]], egui::Stroke::new(2.0, color));
                    }

                    for pt in points.iter().step_by(4.max(points.len() / 8)) {
                        painter.circle_filled(*pt, 3.0, color);
                    }
                }

                // Draw the point-by-point terrain preview
                if self.active_terrain_preset().is_none() && !self.draw_terrain_points.is_empty() {
                    let color = egui::Color32::from_rgb(100, 220, 100);
                    let close_color = egui::Color32::from_rgb(255, 200, 60);
                    let continuation_color = egui::Color32::from_rgb(80, 170, 255);
                    let points: Vec<egui::Pos2> = self
                        .draw_terrain_points
                        .iter()
                        .map(|p| self.camera.world_to_screen(*p, canvas_center))
                        .collect();

                    // Draw lines between placed points
                    for pair in points.windows(2) {
                        painter.line_segment([pair[0], pair[1]], egui::Stroke::new(2.0, color));
                    }

                    // Draw dots at each placed point
                    for pt in &points {
                        painter.circle_filled(*pt, 3.5, color);
                    }

                    // When only one point exists while active, this point is a
                    // continuation anchor from the previous stroke endpoint.
                    if self.draw_terrain_active && points.len() == 1 {
                        painter.circle_stroke(
                            points[0],
                            7.0,
                            egui::Stroke::new(2.0, continuation_color),
                        );
                    }

                    // Highlight first point when closeable (≥3 points) in polyline modes
                    if !matches!(
                        self.terrain_draw_mode,
                        super::TerrainDrawMode::Curve | super::TerrainDrawMode::CircularArc
                    ) && points.len() >= 3
                    {
                        painter.circle_stroke(points[0], 8.0, egui::Stroke::new(2.0, close_color));
                    }

                    // Draw preview for current draw mode
                    if self.draw_terrain_active
                        && let Some(mouse) = painter.ctx().input(|i| i.pointer.latest_pos())
                    {
                        let dash_color = egui::Color32::from_rgba_unmultiplied(100, 220, 100, 140);
                        if matches!(
                            self.terrain_draw_mode,
                            super::TerrainDrawMode::Curve | super::TerrainDrawMode::CircularArc
                        ) {
                            if self.draw_terrain_points.len() == 2 {
                                let start = self.draw_terrain_points[0];
                                let end = self.draw_terrain_points[1];
                                let control = self.camera.screen_to_world(mouse, canvas_center);
                                let nodes = self.terrain_curve_segments.clamp(3, 256);
                                let sampled_world = if self.terrain_draw_mode
                                    == super::TerrainDrawMode::CircularArc
                                {
                                    sample_circular_arc_preview(start, control, end, nodes)
                                        .unwrap_or_else(|| {
                                            let mut world_points = Vec::with_capacity(nodes);
                                            let denom = (nodes - 1) as f32;
                                            for i in 0..nodes {
                                                let t = i as f32 / denom;
                                                let omt = 1.0 - t;
                                                world_points.push(Vec2 {
                                                    x: omt * omt * start.x
                                                        + 2.0 * omt * t * control.x
                                                        + t * t * end.x,
                                                    y: omt * omt * start.y
                                                        + 2.0 * omt * t * control.y
                                                        + t * t * end.y,
                                                });
                                            }
                                            world_points
                                        })
                                } else {
                                    let mut world_points = Vec::with_capacity(nodes);
                                    let denom = (nodes - 1) as f32;
                                    for i in 0..nodes {
                                        let t = i as f32 / denom;
                                        let omt = 1.0 - t;
                                        world_points.push(Vec2 {
                                            x: omt * omt * start.x
                                                + 2.0 * omt * t * control.x
                                                + t * t * end.x,
                                            y: omt * omt * start.y
                                                + 2.0 * omt * t * control.y
                                                + t * t * end.y,
                                        });
                                    }
                                    world_points
                                };

                                let sampled: Vec<egui::Pos2> = sampled_world
                                    .iter()
                                    .map(|world| self.camera.world_to_screen(*world, canvas_center))
                                    .collect();
                                for pair in sampled.windows(2) {
                                    painter.line_segment(
                                        [pair[0], pair[1]],
                                        egui::Stroke::new(1.5, dash_color),
                                    );
                                }

                                let control_screen =
                                    self.camera.world_to_screen(control, canvas_center);
                                painter.circle_stroke(
                                    control_screen,
                                    6.0,
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 210, 80)),
                                );

                                if self.terrain_draw_mode == super::TerrainDrawMode::CircularArc
                                    && let Some(center) =
                                        circle_center_from_three_points(start, control, end)
                                {
                                    let center_screen =
                                        self.camera.world_to_screen(center, canvas_center);
                                    painter.circle_filled(
                                        center_screen,
                                        3.5,
                                        egui::Color32::from_rgb(80, 170, 255),
                                    );
                                    painter.line_segment(
                                        [center_screen, control_screen],
                                        egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgba_unmultiplied(
                                                80, 170, 255, 140,
                                            ),
                                        ),
                                    );
                                }
                            }
                        } else if let Some(&last) = points.last() {
                            let preview_mouse = {
                                let mouse_world = self.camera.screen_to_world(mouse, canvas_center);
                                let constrained_world = match self.terrain_draw_mode {
                                    super::TerrainDrawMode::Curve
                                    | super::TerrainDrawMode::CircularArc => mouse_world,
                                    super::TerrainDrawMode::Free => mouse_world,
                                    super::TerrainDrawMode::Horizontal => Vec2 {
                                        x: mouse_world.x,
                                        y: self
                                            .draw_terrain_points
                                            .last()
                                            .map(|p| p.y)
                                            .unwrap_or(mouse_world.y),
                                    },
                                    super::TerrainDrawMode::Vertical => Vec2 {
                                        x: self
                                            .draw_terrain_points
                                            .last()
                                            .map(|p| p.x)
                                            .unwrap_or(mouse_world.x),
                                        y: mouse_world.y,
                                    },
                                };
                                self.camera
                                    .world_to_screen(constrained_world, canvas_center)
                            };
                            painter.line_segment(
                                [last, preview_mouse],
                                egui::Stroke::new(1.0, dash_color),
                            );
                        }

                        // If near first point and ≥3 points, show closing preview
                        if !matches!(
                            self.terrain_draw_mode,
                            super::TerrainDrawMode::Curve | super::TerrainDrawMode::CircularArc
                        ) && points.len() >= 3
                        {
                            let first_screen = points[0];
                            let dx = mouse.x - first_screen.x;
                            let dy = mouse.y - first_screen.y;
                            if dx * dx + dy * dy < 12.0 * 12.0 {
                                painter.circle_filled(
                                    first_screen,
                                    8.0,
                                    egui::Color32::from_rgba_unmultiplied(255, 200, 60, 80),
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
