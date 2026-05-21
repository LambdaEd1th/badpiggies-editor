//! Dark level overlay: lit area polygon parsing, scanline mesh generation.

use std::sync::Arc;

use eframe::egui;

use super::LevelRenderer;
use super::dark_mask_shader;

mod intervals;
mod mesh;
mod parse;

pub(super) use intervals::{can_transform_overlay, overlay_key, transformed_overlay_mesh};
pub(super) use mesh::build_dark_overlay_meshes;
pub(super) use parse::{construction_grid_start_light, parse_dark_level_data};

pub(super) type DarkOverlayKey = (f32, f32, f32, f32, f32, f32, f32);

/// A pre-computed lit area polygon from a LitArea prefab's bezier curve.
pub(super) struct LitAreaPolygon {
    /// World-space polygon vertices (closed loop) — the lit area boundary.
    pub vertices: Vec<(f32, f32)>,
    /// World-space polygon vertices for the outer border ring.
    /// Empty means this light has no separate dark border region.
    pub border_vertices: Vec<(f32, f32)>,
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
// Unity's depth-mask border material darkens the scene to roughly 68.6% of
// the original color (`DepthMaskTransparent.mat` _Color ~= 0.686), which is
// visually closest to a black overlay with alpha ~= 80.
const POINT_LIGHT_BORDER_ALPHA: u8 = 80;
const POINT_LIGHT_BORDER_ALPHA_NIGHT_VISION: u8 = 53;

fn color32_from_uniform(color: [f32; 4]) -> egui::Color32 {
    let to_u8 = |component: f32| (component.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgba_unmultiplied(
        to_u8(color[0]),
        to_u8(color[1]),
        to_u8(color[2]),
        to_u8(color[3]),
    )
}

fn recolor_mesh(mesh: &mut egui::Mesh, color: egui::Color32) {
    for vertex in &mut mesh.vertices {
        vertex.color = color;
    }
}

fn build_fullscreen_overlay_mesh(rect: egui::Rect) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    mesh.vertices = vec![
        egui::epaint::Vertex {
            pos: rect.left_top(),
            uv: egui::Pos2::ZERO,
            color: egui::Color32::WHITE,
        },
        egui::epaint::Vertex {
            pos: rect.right_top(),
            uv: egui::Pos2::ZERO,
            color: egui::Color32::WHITE,
        },
        egui::epaint::Vertex {
            pos: rect.right_bottom(),
            uv: egui::Pos2::ZERO,
            color: egui::Color32::WHITE,
        },
        egui::epaint::Vertex {
            pos: rect.left_bottom(),
            uv: egui::Pos2::ZERO,
            color: egui::Color32::WHITE,
        },
    ];
    mesh.indices = vec![0, 1, 2, 0, 2, 3];
    mesh
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
    fn dark_overlay_uniform_colors(&self) -> ([f32; 4], [f32; 4], egui::Color32, egui::Color32) {
        if self.night_vision_enabled {
            let dark = color32_from_uniform(dark_mask_shader::MASK_OVERLAY_NIGHT_VISION_COLOR);
            let ring = egui::Color32::from_rgba_unmultiplied(
                0,
                0,
                0,
                POINT_LIGHT_BORDER_ALPHA_NIGHT_VISION,
            );
            (
                dark_mask_shader::MASK_OVERLAY_NIGHT_VISION_COLOR,
                dark_mask_shader::DEPTH_MASK_TRANSPARENT_NIGHT_VISION_COLOR,
                dark,
                ring,
            )
        } else {
            let dark = color32_from_uniform(dark_mask_shader::MASK_OVERLAY_COLOR);
            let ring = egui::Color32::from_rgba_unmultiplied(0, 0, 0, POINT_LIGHT_BORDER_ALPHA);
            (
                dark_mask_shader::MASK_OVERLAY_COLOR,
                dark_mask_shader::DEPTH_MASK_TRANSPARENT_COLOR,
                dark,
                ring,
            )
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
            .map(Arc::new);
        self.dark_overlay_light_gpu = self
            .dark_overlay_light
            .as_ref()
            .and_then(|mesh| dark_mask_shader::build_dark_mask_gpu_mesh(device, mesh))
            .map(Arc::new);
        self.dark_overlay_ring_gpu = self
            .dark_overlay_ring
            .as_ref()
            .and_then(|mesh| dark_mask_shader::build_dark_mask_gpu_mesh(device, mesh))
            .map(Arc::new);
    }

    fn draw_dark_overlay_gpu_layer(
        &mut self,
        painter: &egui::Painter,
        rect: egui::Rect,
        cpu_mesh: Option<&egui::Mesh>,
        gpu_mesh: Option<Arc<dark_mask_shader::DarkMaskGpuMesh>>,
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
            return;
        }

        if let Some(mesh) = cpu_mesh
            && !mesh.vertices.is_empty()
        {
            painter.add(egui::Shape::mesh(mesh.clone()));
        }
    }

    pub(super) fn draw_dark_overlay(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        let (dark_uniform, ring_uniform, dark_cpu_color, ring_cpu_color) =
            self.dark_overlay_uniform_colors();
        let key = overlay_key(&self.camera, rect);

        if self.dark_overlay_live_key != key {
            self.dark_overlay_live_key = key;
            self.dark_overlay_stable_frames = 0;
        } else if self.dark_overlay_key != key {
            self.dark_overlay_stable_frames = self.dark_overlay_stable_frames.saturating_add(1);
        }

        if key != self.dark_overlay_key {
            if self.dark_overlay_stable_frames < CPU_REBUILD_SETTLE_FRAMES
                && can_transform_overlay(self.dark_overlay_key, key)
                && self.dark_overlay_mesh.is_some()
            {
                if let Some(ref mesh) = self.dark_overlay_mesh {
                    let mut transformed = transformed_overlay_mesh(mesh, self.dark_overlay_key, key);
                    recolor_mesh(&mut transformed, dark_cpu_color);
                    if !transformed.vertices.is_empty() {
                        painter.add(egui::Shape::mesh(transformed));
                    }
                }
                if let Some(ref mesh) = self.dark_overlay_ring {
                    let mut transformed = transformed_overlay_mesh(mesh, self.dark_overlay_key, key);
                    recolor_mesh(&mut transformed, ring_cpu_color);
                    if !transformed.vertices.is_empty() {
                        painter.add(egui::Shape::mesh(transformed));
                    }
                }
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
        self.dark_mask_slot_counter = 0;

        let mut dark_cpu = self.dark_overlay_mesh.clone();
        let mut ring_cpu = self.dark_overlay_ring.clone();
        if let Some(mesh) = dark_cpu.as_mut() {
            recolor_mesh(mesh, dark_cpu_color);
        }
        if let Some(mesh) = ring_cpu.as_mut() {
            recolor_mesh(mesh, ring_cpu_color);
        }

        self.draw_dark_overlay_gpu_layer(
            painter,
            rect,
            dark_cpu.as_ref(),
            self.dark_overlay_mesh_gpu.clone(),
            dark_mask_shader::DarkMaskPipelineKind::Alpha,
            dark_mask_shader::DarkMaskUniforms::for_viewport(rect, dark_uniform),
        );
        self.draw_dark_overlay_gpu_layer(
            painter,
            rect,
            ring_cpu.as_ref(),
            self.dark_overlay_ring_gpu.clone(),
            dark_mask_shader::DarkMaskPipelineKind::Multiply,
            dark_mask_shader::DarkMaskUniforms::for_viewport(rect, ring_uniform),
        );
    }

    pub(super) fn draw_night_vision_overlay(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        let overlay_mesh = build_fullscreen_overlay_mesh(rect);
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

        if let (Some(resources), Some(device)) =
            (self.dark_mask_resources.clone(), self.wgpu_device.as_ref())
            && self.dark_mask_slot_counter < dark_mask_shader::max_draw_slots()
            && let Some(gpu_mesh) = dark_mask_shader::build_dark_mask_gpu_mesh(device, &overlay_mesh)
        {
            let slot = self.dark_mask_slot_counter;
            self.dark_mask_slot_counter += 1;
            painter.add(dark_mask_shader::make_dark_mask_callback(
                rect,
                resources,
                Arc::new(gpu_mesh),
                dark_mask_shader::DarkMaskPipelineKind::NightVisionOverlay,
                slot,
                uniforms,
            ));
            return;
        }

        painter.rect_filled(
            rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(13, 109, 0, 24),
        );
    }
}
