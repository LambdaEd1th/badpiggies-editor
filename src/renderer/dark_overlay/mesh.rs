//! Scanline-based mesh construction for the dark overlay (and its lit-area cutouts).

use eframe::egui;

use crate::domain::types::Vec2;

use super::super::Camera;
use super::Trapezoid;
use super::intervals::{complement_intervals, merged_poly_intervals, subtract_intervals};
use super::{LIGHT_FILL_ALPHA, LIT_AREA_BORDER_ALPHA, LitAreaPolygon, POINT_LIGHT_BORDER_ALPHA};

pub(in crate::renderer) fn build_dark_overlay_meshes(
    rect: egui::Rect,
    camera: &Camera,
    canvas_center: egui::Vec2,
    lit_areas: &[LitAreaPolygon],
) -> (egui::Mesh, Option<egui::Mesh>, Option<egui::Mesh>) {
    let dark_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200);
    let light_fill_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, LIGHT_FILL_ALPHA);
    let lit_area_border_color =
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, LIT_AREA_BORDER_ALPHA);
    let point_light_border_color =
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, POINT_LIGHT_BORDER_ALPHA);

    if lit_areas.is_empty() {
        let mut m = egui::Mesh::default();
        let uv = egui::pos2(0.0, 0.0);
        emit_quad(
            &mut m,
            Trapezoid {
                left_top: rect.left(),
                right_top: rect.right(),
                left_bot: rect.left(),
                right_bot: rect.right(),
                y_top: rect.top(),
                y_bot: rect.bottom(),
            },
            dark_color,
            uv,
        );
        return (m, None, None);
    }

    let to_screen = |wx: f32, wy: f32| -> egui::Pos2 {
        camera.world_to_screen(Vec2 { x: wx, y: wy }, canvas_center)
    };

    let hole_polys: Vec<Vec<egui::Pos2>> = lit_areas
        .iter()
        .filter_map(|la| {
            let source = if la.border_vertices.len() >= 3 {
                &la.border_vertices
            } else {
                &la.vertices
            };
            let pts: Vec<egui::Pos2> = source.iter().map(|&(wx, wy)| to_screen(wx, wy)).collect();
            if pts.len() >= 3 { Some(pts) } else { None }
        })
        .collect();

    struct ScreenLitArea {
        hole: Vec<egui::Pos2>,
        inner: Vec<egui::Pos2>,
        border_alpha: u8,
    }

    let mut screen_lit_areas = Vec::with_capacity(lit_areas.len());
    for la in lit_areas {
        let inner: Vec<egui::Pos2> = la
            .vertices
            .iter()
            .map(|&(wx, wy)| to_screen(wx, wy))
            .collect();
        if inner.len() < 3 {
            continue;
        }
        let hole = if la.border_vertices.len() >= 3 {
            la.border_vertices
                .iter()
                .map(|&(wx, wy)| to_screen(wx, wy))
                .collect()
        } else {
            inner.clone()
        };
        screen_lit_areas.push(ScreenLitArea {
            hole,
            inner,
            border_alpha: la.border_alpha,
        });
    }

    if hole_polys.is_empty() {
        let mut m = egui::Mesh::default();
        let uv = egui::pos2(0.0, 0.0);
        emit_quad(
            &mut m,
            Trapezoid {
                left_top: rect.left(),
                right_top: rect.right(),
                left_bot: rect.left(),
                right_bot: rect.right(),
                y_top: rect.top(),
                y_bot: rect.bottom(),
            },
            dark_color,
            uv,
        );
        return (m, None, None);
    }

    // When the lit-area union covers the whole viewport, the dark complement
    // is legitimately empty and should stay empty instead of blacking out the
    // entire screen.
    let dark_mesh = build_scanline_complement_mesh(rect, &hole_polys, dark_color);

    let inner_polys: Vec<Vec<egui::Pos2>> = screen_lit_areas
        .iter()
        .map(|area| area.inner.clone())
        .collect();

    let light_fill_mesh = if inner_polys.is_empty() {
        None
    } else {
        let mesh = build_scanline_fill_mesh(rect, &inner_polys, light_fill_color);
        if mesh.vertices.is_empty() {
            None
        } else {
            Some(mesh)
        }
    };

    let ring_mesh = if screen_lit_areas
        .iter()
        .any(|area| area.hole.len() >= 3 && area.hole != area.inner)
    {
        let mut combined = egui::Mesh::default();
        for (index, area) in screen_lit_areas.iter().enumerate() {
            if area.hole == area.inner {
                continue;
            }
            let color = if area.border_alpha == POINT_LIGHT_BORDER_ALPHA {
                point_light_border_color
            } else {
                lit_area_border_color
            };
            let mut exclusion_polys = Vec::with_capacity(screen_lit_areas.len());
            for (other_index, other) in screen_lit_areas.iter().enumerate() {
                if other_index != index {
                    exclusion_polys.push(other.hole.clone());
                }
            }
            exclusion_polys.push(area.inner.clone());
            let part = build_scanline_ring_mesh(
                rect,
                std::slice::from_ref(&area.hole),
                &exclusion_polys,
                color,
            );
            append_mesh(&mut combined, part);
        }
        if combined.vertices.is_empty() {
            None
        } else {
            Some(combined)
        }
    } else {
        None
    };

    (dark_mesh, light_fill_mesh, ring_mesh)
}

/// Build a scanline mesh covering `rect` minus the holes defined by `polys`.
fn build_scanline_complement_mesh(
    rect: egui::Rect,
    polys: &[Vec<egui::Pos2>],
    color: egui::Color32,
) -> egui::Mesh {
    let mut ys: Vec<f32> = Vec::new();
    ys.push(rect.top());
    ys.push(rect.bottom());
    for poly in polys {
        for pt in poly {
            let y = pt.y.clamp(rect.top(), rect.bottom());
            ys.push(y);
        }
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    let mut mesh = egui::Mesh::default();
    let uv = egui::pos2(0.0, 0.0);

    for si in 0..ys.len() - 1 {
        let y_top = ys[si];
        let y_bot = ys[si + 1];
        if y_bot - y_top < 0.5 {
            continue;
        }
        let eps = (y_bot - y_top).min(1.0) * 0.01;
        let top_dark = complement_intervals(
            rect.left(),
            rect.right(),
            &merged_poly_intervals(y_top + eps, polys),
        );
        let bot_dark = complement_intervals(
            rect.left(),
            rect.right(),
            &merged_poly_intervals(y_bot - eps, polys),
        );

        if top_dark.len() == bot_dark.len() {
            for (top, bot) in top_dark.iter().zip(bot_dark.iter()) {
                if top.1 - top.0 > 0.5 || bot.1 - bot.0 > 0.5 {
                    emit_quad(
                        &mut mesh,
                        Trapezoid {
                            left_top: top.0,
                            right_top: top.1,
                            left_bot: bot.0,
                            right_bot: bot.1,
                            y_top,
                            y_bot,
                        },
                        color,
                        uv,
                    );
                }
            }
        } else {
            let y_mid = (y_top + y_bot) * 0.5;
            let mid_dark = complement_intervals(
                rect.left(),
                rect.right(),
                &merged_poly_intervals(y_mid, polys),
            );
            for interval in mid_dark {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: interval.0,
                        right_top: interval.1,
                        left_bot: interval.0,
                        right_bot: interval.1,
                        y_top,
                        y_bot,
                    },
                    color,
                    uv,
                );
            }
        }
    }
    mesh
}

/// Build a scanline mesh filling the ring between `outer_polys` and `inner_polys`.
/// The ring is the region inside outer but outside inner.
fn build_scanline_ring_mesh(
    rect: egui::Rect,
    outer_polys: &[Vec<egui::Pos2>],
    inner_polys: &[Vec<egui::Pos2>],
    color: egui::Color32,
) -> egui::Mesh {
    // Collect Y coords from both polygon sets
    let mut ys: Vec<f32> = Vec::new();
    ys.push(rect.top());
    ys.push(rect.bottom());
    for poly in outer_polys.iter().chain(inner_polys.iter()) {
        for pt in poly {
            let y = pt.y.clamp(rect.top(), rect.bottom());
            ys.push(y);
        }
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    let mut mesh = egui::Mesh::default();
    let uv = egui::pos2(0.0, 0.0);

    for si in 0..ys.len() - 1 {
        let y_top = ys[si];
        let y_bot = ys[si + 1];
        if y_bot - y_top < 0.5 {
            continue;
        }
        let eps = (y_bot - y_top).min(1.0) * 0.01;

        let top_ring = subtract_intervals(
            &merged_poly_intervals(y_top + eps, outer_polys),
            &merged_poly_intervals(y_top + eps, inner_polys),
        );
        let bot_ring = subtract_intervals(
            &merged_poly_intervals(y_bot - eps, outer_polys),
            &merged_poly_intervals(y_bot - eps, inner_polys),
        );

        if top_ring.len() == bot_ring.len() {
            for (top, bot) in top_ring.iter().zip(bot_ring.iter()) {
                if top.1 - top.0 > 0.5 || bot.1 - bot.0 > 0.5 {
                    emit_quad(
                        &mut mesh,
                        Trapezoid {
                            left_top: top.0,
                            right_top: top.1,
                            left_bot: bot.0,
                            right_bot: bot.1,
                            y_top,
                            y_bot,
                        },
                        color,
                        uv,
                    );
                }
            }
        } else {
            let y_mid = (y_top + y_bot) * 0.5;
            let mid_ring = subtract_intervals(
                &merged_poly_intervals(y_mid, outer_polys),
                &merged_poly_intervals(y_mid, inner_polys),
            );
            for interval in mid_ring {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: interval.0,
                        right_top: interval.1,
                        left_bot: interval.0,
                        right_bot: interval.1,
                        y_top,
                        y_bot,
                    },
                    color,
                    uv,
                );
            }
        }
    }
    mesh
}

/// Build a scanline mesh filling the union of `polys`.
fn build_scanline_fill_mesh(
    rect: egui::Rect,
    polys: &[Vec<egui::Pos2>],
    color: egui::Color32,
) -> egui::Mesh {
    let mut ys: Vec<f32> = Vec::new();
    ys.push(rect.top());
    ys.push(rect.bottom());
    for poly in polys {
        for pt in poly {
            let y = pt.y.clamp(rect.top(), rect.bottom());
            ys.push(y);
        }
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    let mut mesh = egui::Mesh::default();
    let uv = egui::pos2(0.0, 0.0);

    for si in 0..ys.len() - 1 {
        let y_top = ys[si];
        let y_bot = ys[si + 1];
        if y_bot - y_top < 0.5 {
            continue;
        }
        let eps = (y_bot - y_top).min(1.0) * 0.01;

        let top_fill = merged_poly_intervals(y_top + eps, polys);
        let bot_fill = merged_poly_intervals(y_bot - eps, polys);

        if top_fill.len() == bot_fill.len() {
            for (top, bot) in top_fill.iter().zip(bot_fill.iter()) {
                if top.1 - top.0 > 0.5 || bot.1 - bot.0 > 0.5 {
                    emit_quad(
                        &mut mesh,
                        Trapezoid {
                            left_top: top.0,
                            right_top: top.1,
                            left_bot: bot.0,
                            right_bot: bot.1,
                            y_top,
                            y_bot,
                        },
                        color,
                        uv,
                    );
                }
            }
        } else {
            let y_mid = (y_top + y_bot) * 0.5;
            let mid_fill = merged_poly_intervals(y_mid, polys);
            for interval in mid_fill {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: interval.0,
                        right_top: interval.1,
                        left_bot: interval.0,
                        right_bot: interval.1,
                        y_top,
                        y_bot,
                    },
                    color,
                    uv,
                );
            }
        }
    }

    mesh
}

/// Emit a trapezoid quad into the mesh.
fn emit_quad(mesh: &mut egui::Mesh, t: Trapezoid, color: egui::Color32, uv: egui::Pos2) {
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: egui::pos2(t.left_top, t.y_top),
        uv,
        color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: egui::pos2(t.right_top, t.y_top),
        uv,
        color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: egui::pos2(t.right_bot, t.y_bot),
        uv,
        color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: egui::pos2(t.left_bot, t.y_bot),
        uv,
        color,
    });
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn append_mesh(dst: &mut egui::Mesh, src: egui::Mesh) {
    let base = dst.vertices.len() as u32;
    dst.vertices.extend(src.vertices);
    dst.indices
        .extend(src.indices.into_iter().map(|index| index + base));
}
