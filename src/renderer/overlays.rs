//! HUD overlays, selection visuals, terrain wireframe, grid, and lazy texture loading.

use std::collections::BTreeSet;

use crate::types::{ObjectIndex, Vec2};

use super::{ATLAS_FILES, CursorMode, GLOW_ATLAS, GOAL_FLAG_TEXTURE, LevelRenderer};

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
        tr: &'static crate::locale::I18n,
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
        // Sprite atlases (sprites/ and props/ subdirs)
        for atlas in ATLAS_FILES {
            if self.tex_cache.get(atlas).is_none() {
                let sprite_key = format!("sprites/{}", atlas);
                let props_key = format!("props/{}", atlas);
                // Props_Generic_Sheet is rendered via wgpu opaque shader (GPU pipeline),
                // so skip egui texture loading for it entirely.
                if atlas == &"Props_Generic_Sheet_01.png" {
                    continue;
                } else if self
                    .tex_cache
                    .load_texture(ctx, &sprite_key, atlas)
                    .is_none()
                {
                    self.tex_cache.load_texture(ctx, &props_key, atlas);
                }
            }
        }
        // Background atlases (bg/ subdir)
        for atlas in crate::bg_data::bg_atlas_files() {
            if self.tex_cache.get(atlas).is_none() {
                self.tex_cache
                    .load_texture(ctx, &format!("bg/{}", atlas), atlas);
            }
        }
        // Sky textures (sky/ subdir)
        for sky in crate::bg_data::sky_texture_files() {
            if self.tex_cache.get(sky).is_none() {
                self.tex_cache
                    .load_texture(ctx, &format!("sky/{}", sky), sky);
            }
        }
        // Ground fill textures (ground/ subdir) — loaded with repeat wrap
        for td in &self.terrain_data {
            if let Some(ref tex_name) = td.fill_texture
                && self.tex_cache.get(tex_name).is_none()
            {
                self.tex_cache
                    .load_texture_repeat(ctx, &format!("ground/{}", tex_name), tex_name);
            }
            // Splat textures for CPU-textured edge fallback
            if let Some(ref tex_name) = td.edge_splat0
                && self.tex_cache.get(tex_name).is_none()
            {
                self.tex_cache
                    .load_texture_repeat(ctx, &format!("ground/{}", tex_name), tex_name);
            }
            if let Some(ref tex_name) = td.edge_splat1
                && self.tex_cache.get(tex_name).is_none()
            {
                self.tex_cache
                    .load_texture_repeat(ctx, &format!("ground/{}", tex_name), tex_name);
            }
        }
        // Goal flag texture (props/ subdir) — repeat wrap + flip V for UV scroll
        if self.tex_cache.get(GOAL_FLAG_TEXTURE).is_none() {
            self.tex_cache.load_texture_repeat_flipv(
                ctx,
                &format!("props/{}", GOAL_FLAG_TEXTURE),
                GOAL_FLAG_TEXTURE,
            );
        }
        // Glow/starburst particle atlas
        if self.tex_cache.get(GLOW_ATLAS).is_none() {
            self.tex_cache
                .load_texture(ctx, &format!("particles/{}", GLOW_ATLAS), GLOW_ATLAS);
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
            .min_by(|a, b| (a - base).abs().partial_cmp(&(b - base).abs()).unwrap())
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
                // Draw the point-by-point terrain preview
                if !self.draw_terrain_points.is_empty() {
                    let color = egui::Color32::from_rgb(100, 220, 100);
                    let close_color = egui::Color32::from_rgb(255, 200, 60);
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

                    // Highlight first point when closeable (≥3 points)
                    if points.len() >= 3 {
                        painter.circle_stroke(points[0], 8.0, egui::Stroke::new(2.0, close_color));
                    }

                    // Draw preview line from last point to mouse cursor
                    if self.draw_terrain_active
                        && let Some(mouse) = painter.ctx().input(|i| i.pointer.latest_pos())
                    {
                        let dash_color = egui::Color32::from_rgba_unmultiplied(100, 220, 100, 140);
                        if let Some(&last) = points.last() {
                            painter.line_segment([last, mouse], egui::Stroke::new(1.0, dash_color));
                        }

                        // If near first point and ≥3 points, show closing preview
                        if points.len() >= 3 {
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
