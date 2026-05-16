//! Sprite rendering — draw prefab objects as colored squares with correct sizes.
//!
//! Uses sprite database for accurate sizing. Falls back to colored rectangles
//! when atlas textures aren't loaded. Supports textured rendering via egui when available.

mod data;
mod draw;
mod glow;

pub use data::{SpriteDrawData, SpriteDrawOpts, build_sprite};
pub(in crate::renderer::sprites) use draw::dessert_y_offset;
pub use draw::draw_sprite;
pub use glow::{draw_glow, has_glow};

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use eframe::egui;

use crate::data::{assets, unity_anim};
use crate::domain::types::*;

use super::compounds;
use super::{CompoundTransform, DrawCtx, LevelRenderer};
use super::{background, opaque_shader, sprite_shader};

// BirdSleep2.anim fallback curves (t, value, inSlope, outSlope).
const DEFAULT_BIRD_SLEEP_DURATION: f32 = 4.0;
const DEFAULT_BIRD_SLEEP_POS_Y: &[unity_anim::HermiteKey] = &[
    (0.0, 0.0, -0.03356487, -0.03356487),
    (1.833333, -0.061, -0.00255944, -0.00255944),
    (4.0, 0.0, 0.02840104, 0.02840104),
];
const DEFAULT_BIRD_SLEEP_SCALE_X: &[unity_anim::HermiteKey] = &[
    (0.0, 1.0, 0.05454547, 0.05454547),
    (1.833333, 1.1, 0.004195808, 0.004195808),
    (4.0, 1.0, -0.04615385, -0.04615385),
];
const DEFAULT_BIRD_SLEEP_SCALE_Y: &[unity_anim::HermiteKey] = &[
    (0.0, 1.0, -0.05454547, -0.05454547),
    (1.833333, 0.9, -0.004195808, -0.004195808),
    (4.0, 1.0, 0.04615385, 0.04615385),
];

pub(super) fn bird_sleep_duration() -> f32 {
    unity_anim::bird_sleep_clip()
        .map(|clip| clip.duration)
        .filter(|duration| *duration > 0.0)
        .unwrap_or(DEFAULT_BIRD_SLEEP_DURATION)
}

fn bird_sleep_loops() -> bool {
    unity_anim::bird_sleep_clip()
        .map(|clip| clip.loops)
        .unwrap_or(true)
}

fn bird_sleep_pos_y_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::bird_sleep_clip()
        .and_then(|clip| clip.root_position())
        .map(|curve| curve.y.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_BIRD_SLEEP_POS_Y)
}

fn bird_sleep_scale_x_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::bird_sleep_clip()
        .and_then(|clip| clip.root_scale())
        .map(|curve| curve.x.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_BIRD_SLEEP_SCALE_X)
}

fn bird_sleep_scale_y_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::bird_sleep_clip()
        .and_then(|clip| clip.root_scale())
        .map(|curve| curve.y.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_BIRD_SLEEP_SCALE_Y)
}

fn bird_sleep_time(time: f64, phase: f32) -> f32 {
    let raw_time = time as f32 + phase;
    let duration = bird_sleep_duration();
    if bird_sleep_loops() {
        raw_time.rem_euclid(duration)
    } else {
        raw_time.clamp(0.0, duration)
    }
}

pub(super) fn bird_sleep_y_offset(time: f64, phase: f32) -> f32 {
    background::hermite(bird_sleep_pos_y_curve(), bird_sleep_time(time, phase))
}

pub(super) fn bird_sleep_scale_factors(time: f64, phase: f32) -> (f32, f32) {
    let sleep_time = bird_sleep_time(time, phase);
    (
        background::hermite(bird_sleep_scale_x_curve(), sleep_time),
        background::hermite(bird_sleep_scale_y_curve(), sleep_time),
    )
}

// Consecutive same-type GPU draws can still be batched, but they may need to be
// flushed at terrain/wind insertion points to preserve global transparent order.
enum GpuDraw {
    Opaque(opaque_shader::OpaqueBatchDraw),
    Transparent(sprite_shader::SpriteBatchDraw),
}

impl LevelRenderer {
    fn flush_gpu_draws(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        gpu_draws: &mut Vec<GpuDraw>,
    ) {
        if gpu_draws.is_empty() {
            return;
        }

        let mut pending_opaque: Vec<opaque_shader::OpaqueBatchDraw> = Vec::new();
        let mut pending_transparent: Vec<sprite_shader::SpriteBatchDraw> = Vec::new();
        let props_tint = assets::props_tint_color(self.bg_theme);

        for draw in gpu_draws.drain(..) {
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

    /// Draw all sprites with GPU batching, compound sub-sprites, and bird face deferral.
    pub(super) fn draw_sprites(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
        selected: &BTreeSet<ObjectIndex>,
    ) {
        let t = self.time;
        let active_transform_index = self
            .dragging
            .as_ref()
            .and_then(|drag| match drag.mode {
                super::DragMode::Rotate { .. } | super::DragMode::Scale { .. } => Some(drag.index),
                super::DragMode::Move => None,
            })
            .or(self.pending_transform_preview);

        // Pre-compute world-space visible rect for frustum culling
        let world_half_w = rect.width() * 0.5 / self.camera.zoom;
        let world_half_h = rect.height() * 0.5 / self.camera.zoom;
        let visible_min_x = self.camera.center.x - world_half_w;
        let visible_max_x = self.camera.center.x + world_half_w;
        let visible_min_y = self.camera.center.y - world_half_h;
        let visible_max_y = self.camera.center.y + world_half_h;

        // Build fan state lookup (avoids O(sprites × fans) per-frame scan)
        let mut fan_angle_map: Vec<Option<(f32, f32)>> = vec![None; self.sprite_data.len()];
        for e in &self.fan_emitters {
            if e.sprite_index < fan_angle_map.len() {
                fan_angle_map[e.sprite_index] = Some((e.angle, e.force));
            }
        }

        let mut wind_area_map: Vec<Option<super::particles::WindAreaDef>> =
            vec![None; self.sprite_data.len()];
        for area in &self.wind_areas {
            if area.sprite_index < wind_area_map.len() {
                wind_area_map[area.sprite_index] = Some(area.clone());
            }
        }
        let collider_terrain_map: HashMap<ObjectIndex, usize> = self
            .terrain_data
            .iter()
            .enumerate()
            .filter_map(|(terrain_index, terrain)| {
                (!terrain.decorative).then_some((terrain.object_index, terrain_index))
            })
            .collect();
        let mut wind_render_queue: Vec<(f32, usize)> = self
            .wind_areas
            .iter()
            .map(|area| (area.render_z, area.sprite_index))
            .collect();
        wind_render_queue.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
        let mut wind_render_cursor = 0usize;

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

        for si in 0..self.sprite_data.len() {
            let sprite_z = self.sprite_data[si].world_pos.z;
            while wind_render_cursor < wind_render_queue.len()
                && wind_render_queue[wind_render_cursor].0 >= sprite_z
            {
                self.flush_gpu_draws(painter, rect, &mut gpu_draws);
                let source_sprite_index = wind_render_queue[wind_render_cursor].1;
                super::particles::draw_wind_particles(
                    &self.wind_particles,
                    Some(source_sprite_index),
                    &self.camera,
                    painter,
                    canvas_center,
                    rect,
                    self.tex_cache.get(super::GLOW_ATLAS),
                );
                wind_render_cursor += 1;
            }

            let (is_terrain, sprite_index, is_goal_area, is_sel, half_size, world_pos) = {
                let sprite = &self.sprite_data[si];
                (
                    sprite.is_terrain,
                    sprite.index,
                    sprite.name_lower.starts_with("goalarea"),
                    selected.contains(&sprite.index)
                        || (sprite.is_hidden
                            && sprite.parent.is_some()
                            && sprite.parent.is_some_and(|p| selected.contains(&p))),
                    sprite.half_size,
                    sprite.world_pos,
                )
            };

            // Early world-space frustum cull
            if !is_sel {
                let margin = half_size.0.max(half_size.1)
                    + if is_goal_area { 16.0 } else { 2.0 };
                let sx = world_pos.x;
                let sy = world_pos.y;
                if sx + margin < visible_min_x
                    || sx - margin > visible_max_x
                    || sy + margin < visible_min_y
                    || sy - margin > visible_max_y
                {
                    continue;
                }
            }

            if is_terrain {
                if let Some(&terrain_index) = collider_terrain_map.get(&sprite_index) {
                    self.flush_gpu_draws(painter, rect, &mut gpu_draws);
                    self.draw_terrain_index(terrain_index, painter, canvas_center, rect);
                }
                continue;
            }

            let sprite = &self.sprite_data[si];

            let fan_state = fan_angle_map[si];
            let fan_angle = fan_state.map(|state| state.0);
            let fan_force = fan_state.map(|state| state.1);
            let wind_area = wind_area_map[si].clone();
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
                fan_angle,
            );

            if sprite.name == "Fan" && is_sel {
                draw::draw_fan_field_overlay(
                    &DrawCtx {
                        painter,
                        camera: &self.camera,
                        canvas_center,
                        canvas_rect: rect,
                        tex_cache: &self.tex_cache,
                    },
                    CompoundTransform {
                        world_x: sprite.world_pos.x,
                        world_y: sprite.world_pos.y,
                        scale_x: sprite.scale.0,
                        scale_y: sprite.scale.1,
                        rotation_z: sprite.rotation,
                    },
                    fan_force,
                    self.preview_playback_state,
                );
            }

            let mut is_gpu_rendered = false;

            if !skip_root {
                let opaque_idx = self.opaque_sprite_map.get(si).copied().flatten();
                // Props sprites: render via GPU opaque shader (exact Unity shader port)
                if let Some(oidx) = opaque_idx
                    && !is_goal_area
                    && active_transform_index != Some(sprite.index)
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
                        bird_sleep_y_offset(t, sprite.bird_phase)
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
                    && !is_goal_area
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
                        bird_sleep_y_offset(t, sprite.bird_phase)
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
                        let (sx, sy) = bird_sleep_scale_factors(t, sprite.bird_phase);
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
                if !gpu_rendered
                    && active_transform_index == Some(sprite.index)
                    && let Some(atlas_name) = sprite.atlas.as_deref()
                    && self.tex_cache.get(atlas_name).is_none()
                {
                    let sprite_key = format!("sprites/{atlas_name}");
                    if self
                        .tex_cache
                        .load_texture(painter.ctx(), &sprite_key, atlas_name)
                        .is_none()
                    {
                        let props_key = format!("props/{atlas_name}");
                        self.tex_cache
                            .load_texture(painter.ctx(), &props_key, atlas_name);
                    }
                }
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
                        wind_area,
                        preview_state: self.preview_playback_state,
                        opaque_rendered: gpu_rendered,
                    },
                );
            }

            // Bird face: defer if GPU-rendered so faces draw after batch callback
            if sprite.name.starts_with("Bird_") && !sprite.name.starts_with("BirdCompass") {
                let breath_y = bird_sleep_y_offset(t, sprite.bird_phase);
                let (breath_sx, breath_sy) = bird_sleep_scale_factors(t, sprite.bird_phase);
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

        self.flush_gpu_draws(painter, rect, &mut gpu_draws);
        while wind_render_cursor < wind_render_queue.len() {
            let source_sprite_index = wind_render_queue[wind_render_cursor].1;
            super::particles::draw_wind_particles(
                &self.wind_particles,
                Some(source_sprite_index),
                &self.camera,
                painter,
                canvas_center,
                rect,
                self.tex_cache.get(super::GLOW_ATLAS),
            );
            wind_render_cursor += 1;
        }

        // Emit GPU sprite callbacks in Z order, batching consecutive same-type
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

        if let Some(sprite) = self.selected_transform_sprite(selected) {
            self.draw_rotation_handle(painter, sprite, canvas_center);
            self.draw_scale_handle(painter, sprite, canvas_center);
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
                let glow_margin = if sprite.name_lower.starts_with("goalarea") {
                    16.0
                } else {
                    glow_margin
                };
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
                let margin = 16.0;
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
