//! Sprite rendering — draw prefab objects as colored squares with correct sizes.
//!
//! Uses sprite database for accurate sizing. Falls back to colored rectangles
//! when atlas textures aren't loaded. Supports textured rendering via egui when available.

mod data;
mod draw;
mod glow;

pub use data::{build_sprite, SpriteDrawData, SpriteDrawOpts};
pub use draw::draw_sprite;
pub use glow::{draw_glow, has_glow};

use eframe::egui;

use crate::assets;
use crate::types::*;

use super::compounds;
use super::{Camera, CompoundTransform, DrawCtx, LevelRenderer};
use super::{background, opaque_shader, sprite_shader};

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
                let bt = ((t as f32 + sprite.bird_phase) % BIRD_SLEEP_DURATION).max(0.0);
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
                && let (Some(resources), Some(batch)) = (&self.opaque_resources, &self.opaque_batch)
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
