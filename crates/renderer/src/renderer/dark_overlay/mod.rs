//! Dark level overlay: lit area polygon parsing, scanline mesh generation.

use std::rc::Rc;

use super::LevelRenderer;
use super::dark_mask_shader;

mod intervals;
mod mesh;
mod parse;

pub(super) use intervals::{can_transform_overlay, overlay_key, overlay_vertex_transform};
pub(super) use mesh::build_dark_overlay_meshes;
pub(super) use parse::{construction_grid_start_light, parse_dark_level_data};

pub(super) type DarkOverlayKey = (f32, f32, f32, f32, f32, f32, f32);

/// A pre-computed lit area polygon from a LitArea prefab's bezier curve.
pub(super) struct LitAreaPolygon {
    /// World-space polygon vertices (closed loop) — the lit area boundary.
    pub vertices: Vec<(f32, f32)>,
    /// World-space polygon vertices for the outer border extent.
    /// Empty means this light has no separate dark border region.
    pub border_vertices: Vec<(f32, f32)>,
    /// World-space polygon vertices for the inner edge of the visible border.
    /// Empty means the border begins directly at `vertices`.
    pub border_inner_vertices: Vec<(f32, f32)>,
    /// Alpha used when drawing the border ring approximation.
    pub border_alpha: u8,
}

const LIT_AREA_BORDER_ALPHA: u8 = 80;
// The runtime point-light core uses `DepthMask`, which acts as a depth-written
// cutout rather than a visible darkening layer in normal mode.
const LIGHT_FILL_ALPHA: u8 = 0;
// Keep using the GPU interaction path until the camera has been stable for a
// few frames; rebuilding the CPU exact mesh too eagerly causes visible hitches
// while dragging slowly or during wheel-zoom deceleration.
const CPU_REBUILD_SETTLE_FRAMES: u8 = 6;
const POINT_LIGHT_BORDER_ALPHA: u8 = 80;

fn build_fullscreen_overlay_mesh(rect: crate::gpu2d::Rect) -> crate::gpu2d::Mesh {
    crate::gpu2d::Mesh {
        vertices: vec![
            crate::gpu2d::epaint::Vertex {
                pos: rect.left_top(),
                uv: crate::gpu2d::Pos2::ZERO,
                color: crate::gpu2d::Color32::WHITE,
            },
            crate::gpu2d::epaint::Vertex {
                pos: rect.right_top(),
                uv: crate::gpu2d::Pos2::ZERO,
                color: crate::gpu2d::Color32::WHITE,
            },
            crate::gpu2d::epaint::Vertex {
                pos: rect.right_bottom(),
                uv: crate::gpu2d::Pos2::ZERO,
                color: crate::gpu2d::Color32::WHITE,
            },
            crate::gpu2d::epaint::Vertex {
                pos: rect.left_bottom(),
                uv: crate::gpu2d::Pos2::ZERO,
                color: crate::gpu2d::Color32::WHITE,
            },
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    }
}

/// Trapezoid defined by top/bottom edge X-ranges and Y values.
struct Trapezoid {
    left_top: f32,
    right_top: f32,
    left_bot: f32,
    right_bot: f32,
    y_top: f32,
    y_bot: f32,
}

// ── Dark overlay draw method (extracted from show()) ──

impl LevelRenderer {
    fn dark_overlay_uniform_colors(&self) -> ([f32; 4], [f32; 4]) {
        if self.night_vision_enabled {
            let dark_uniform = dark_mask_shader::MASK_OVERLAY_NIGHT_VISION_COLOR;
            let ring_uniform = dark_mask_shader::DEPTH_MASK_TRANSPARENT_NIGHT_VISION_COLOR;
            (dark_uniform, ring_uniform)
        } else {
            let dark_uniform = dark_mask_shader::MASK_OVERLAY_COLOR;
            let ring_uniform = dark_mask_shader::DEPTH_MASK_TRANSPARENT_COLOR;
            (dark_uniform, ring_uniform)
        }
    }

    fn rebuild_dark_overlay_gpu_meshes(&mut self) {
        let Some(device) = self.wgpu_device.as_ref() else {
            self.dark_overlay_mesh_gpu = None;
            self.dark_overlay_light_gpu = None;
            self.dark_overlay_ring_gpu = None;
            return;
        };
        if self.dark_mask_resources.is_none() {
            self.dark_overlay_mesh_gpu = None;
            self.dark_overlay_light_gpu = None;
            self.dark_overlay_ring_gpu = None;
            return;
        }

        self.dark_overlay_mesh_gpu = self
            .dark_overlay_mesh
            .as_ref()
            .and_then(|mesh| dark_mask_shader::build_dark_mask_gpu_mesh(device, mesh))
            .map(Rc::new);
        self.dark_overlay_light_gpu = self
            .dark_overlay_light
            .as_ref()
            .and_then(|mesh| dark_mask_shader::build_dark_mask_gpu_mesh(device, mesh))
            .map(Rc::new);
        self.dark_overlay_ring_gpu = self
            .dark_overlay_ring
            .as_ref()
            .and_then(|mesh| dark_mask_shader::build_dark_mask_gpu_mesh(device, mesh))
            .map(Rc::new);
    }

    fn draw_dark_overlay_gpu_layer(
        &mut self,
        painter: &crate::gpu2d::Painter,
        rect: crate::gpu2d::Rect,
        gpu_mesh: Option<Rc<dark_mask_shader::DarkMaskGpuMesh>>,
        pipeline_kind: dark_mask_shader::DarkMaskPipelineKind,
        uniforms: dark_mask_shader::DarkMaskUniforms,
    ) {
        if let (Some(resources), Some(gpu_mesh)) = (self.dark_mask_resources.clone(), gpu_mesh)
            && self.dark_mask_slot_counter < dark_mask_shader::max_draw_slots()
        {
            let slot = self.dark_mask_slot_counter;
            self.dark_mask_slot_counter += 1;
            painter.add(dark_mask_shader::make_dark_mask_callback(
                rect,
                resources,
                gpu_mesh,
                pipeline_kind,
                slot,
                uniforms,
            ));
        }
    }

    pub(super) fn draw_dark_overlay(
        &mut self,
        painter: &crate::gpu2d::Painter,
        canvas_center: crate::gpu2d::Vec2,
        rect: crate::gpu2d::Rect,
    ) {
        let (dark_uniform, ring_uniform) = self.dark_overlay_uniform_colors();
        let key = overlay_key(&self.camera, rect);

        if self.dark_overlay_live_key != key {
            self.dark_overlay_live_key = key;
            self.dark_overlay_stable_frames = 0;
        } else if self.dark_overlay_key != key {
            self.dark_overlay_stable_frames = self.dark_overlay_stable_frames.saturating_add(1);
        }
        self.dark_mask_slot_counter = 0;

        if key != self.dark_overlay_key {
            if self.dark_overlay_stable_frames < CPU_REBUILD_SETTLE_FRAMES
                && can_transform_overlay(self.dark_overlay_key, key)
                && self.dark_overlay_mesh_gpu.is_some()
            {
                let (scale, offset) = overlay_vertex_transform(self.dark_overlay_key, key);
                self.draw_dark_overlay_gpu_layer(
                    painter,
                    rect,
                    self.dark_overlay_mesh_gpu.clone(),
                    dark_mask_shader::DarkMaskPipelineKind::Alpha,
                    dark_mask_shader::DarkMaskUniforms::for_viewport(rect, dark_uniform)
                        .with_vertex_transform(scale, offset),
                );
                self.draw_dark_overlay_gpu_layer(
                    painter,
                    rect,
                    self.dark_overlay_ring_gpu.clone(),
                    dark_mask_shader::DarkMaskPipelineKind::Multiply,
                    dark_mask_shader::DarkMaskUniforms::for_viewport(rect, ring_uniform)
                        .with_vertex_transform(scale, offset),
                );
                painter.ctx().request_repaint();
                return;
            }

            let (dark_mesh, light_fill_mesh, ring_mesh) = build_dark_overlay_meshes(
                rect,
                &self.camera,
                canvas_center,
                &self.lit_area_polygons,
            );
            self.dark_overlay_mesh = Some(dark_mesh);
            self.dark_overlay_light = light_fill_mesh;
            self.dark_overlay_ring = ring_mesh;
            self.rebuild_dark_overlay_gpu_meshes();
            self.dark_overlay_key = key;
            self.dark_overlay_stable_frames = CPU_REBUILD_SETTLE_FRAMES;
        }

        self.dark_overlay_live_key = key;

        self.draw_dark_overlay_gpu_layer(
            painter,
            rect,
            self.dark_overlay_mesh_gpu.clone(),
            dark_mask_shader::DarkMaskPipelineKind::Alpha,
            dark_mask_shader::DarkMaskUniforms::for_viewport(rect, dark_uniform),
        );
        self.draw_dark_overlay_gpu_layer(
            painter,
            rect,
            self.dark_overlay_ring_gpu.clone(),
            dark_mask_shader::DarkMaskPipelineKind::Multiply,
            dark_mask_shader::DarkMaskUniforms::for_viewport(rect, ring_uniform),
        );
    }

    pub(super) fn draw_night_vision_overlay(
        &mut self,
        painter: &crate::gpu2d::Painter,
        rect: crate::gpu2d::Rect,
    ) {
        let overlay_rect = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
        if self.night_vision_overlay_rect != Some(overlay_rect) {
            self.night_vision_overlay_gpu = self.wgpu_device.as_ref().and_then(|device| {
                dark_mask_shader::build_dark_mask_gpu_mesh(
                    device,
                    &build_fullscreen_overlay_mesh(rect),
                )
                .map(Rc::new)
            });
            self.night_vision_overlay_rect = Some(overlay_rect);
        }
        let uniforms = dark_mask_shader::DarkMaskUniforms::for_viewport_with_params(
            rect,
            dark_mask_shader::NIGHT_VISION_OVERLAY_COLOR,
            [
                dark_mask_shader::NIGHT_VISION_OVERLAY_RADIUS,
                dark_mask_shader::NIGHT_VISION_OVERLAY_SOFTNESS,
                0.0,
                0.0,
            ],
        );

        self.draw_dark_overlay_gpu_layer(
            painter,
            rect,
            self.night_vision_overlay_gpu.clone(),
            dark_mask_shader::DarkMaskPipelineKind::NightVisionOverlay,
            uniforms,
        );
    }
}
