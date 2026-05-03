//! Dark level overlay: lit area polygon parsing, scanline mesh generation.

use eframe::egui;

use super::LevelRenderer;

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
// Unity renders an explicit light mesh inside lit regions instead of leaving a
// completely untouched hole. Approximate that middle layer as a faint darkening
// so the editor has distinct dark / lit / border bands.
const LIGHT_FILL_ALPHA: u8 = 28;
// Keep using the GPU interaction path until the camera has been stable for a
// few frames; rebuilding the CPU exact mesh too eagerly causes visible hitches
// while dragging slowly or during wheel-zoom deceleration.
const CPU_REBUILD_SETTLE_FRAMES: u8 = 6;
// Unity's depth-mask border material darkens the scene to roughly 68.6% of
// the original color (`DepthMaskTransparent.mat` _Color ~= 0.686), which is
// visually closest to a black overlay with alpha ~= 80.
const POINT_LIGHT_BORDER_ALPHA: u8 = 80;

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
    pub(super) fn draw_dark_overlay(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
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
                    let transformed = transformed_overlay_mesh(mesh, self.dark_overlay_key, key);
                    if !transformed.vertices.is_empty() {
                        painter.add(egui::Shape::mesh(transformed));
                    }
                }
                if let Some(ref mesh) = self.dark_overlay_light {
                    let transformed = transformed_overlay_mesh(mesh, self.dark_overlay_key, key);
                    if !transformed.vertices.is_empty() {
                        painter.add(egui::Shape::mesh(transformed));
                    }
                }
                if let Some(ref mesh) = self.dark_overlay_ring {
                    let transformed = transformed_overlay_mesh(mesh, self.dark_overlay_key, key);
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
            self.dark_overlay_key = key;
            self.dark_overlay_stable_frames = CPU_REBUILD_SETTLE_FRAMES;
        }

        self.dark_overlay_live_key = key;

        if let Some(ref mesh) = self.dark_overlay_mesh
            && !mesh.vertices.is_empty()
        {
            painter.add(egui::Shape::mesh(mesh.clone()));
        }
        if let Some(ref mesh) = self.dark_overlay_light
            && !mesh.vertices.is_empty()
        {
            painter.add(egui::Shape::mesh(mesh.clone()));
        }
        if let Some(ref mesh) = self.dark_overlay_ring
            && !mesh.vertices.is_empty()
        {
            painter.add(egui::Shape::mesh(mesh.clone()));
        }
    }
}
