//! Dark level overlay: lit area polygon parsing, scanline mesh generation.

use eframe::egui;

use crate::types::*;

use super::dark_shader;
use super::{Camera, LevelRenderer};

/// A pre-computed lit area polygon from a LitArea prefab's bezier curve.
pub(super) struct LitAreaPolygon {
    /// World-space polygon vertices (closed loop) — the lit area boundary.
    pub vertices: Vec<(f32, f32)>,
    /// World-space polygon vertices for the outer border ring.
    /// The border region is between `vertices` and `border_vertices`.
    pub border_vertices: Vec<(f32, f32)>,
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

/// Parse `m_darkLevel` from LevelManager and LitArea bezier curves from the level.
pub(super) fn parse_dark_level_data(
    level: &LevelData,
    dark_level: &mut bool,
    lit_areas: &mut Vec<LitAreaPolygon>,
) {
    *dark_level = false;
    lit_areas.clear();

    for obj in &level.objects {
        if let LevelObject::Prefab(p) = obj {
            // Check LevelManager for m_darkLevel
            if p.name == "LevelManager"
                && let Some(ref od) = p.override_data
                && let Some(pos) = od.raw_text.find("m_darkLevel")
            {
                let after = &od.raw_text[pos..];
                if let Some(eq) = after.find("= ") {
                    let val = after[eq + 2..].trim_start();
                    if val.starts_with("True") || val.starts_with("true") {
                        *dark_level = true;
                    }
                }
            }

            // Parse LitArea bezier curves
            if p.name == "LitArea"
                && let Some(polygon) = parse_lit_area_bezier(p)
            {
                lit_areas.push(polygon);
            }

            // Parse point light sources (LitCrystal, LitMushroom)
            if let Some(polygon) = parse_point_light(p) {
                lit_areas.push(polygon);
            }
        }
    }
}

/// Parse a LitArea prefab's override data to extract bezier curve polygon vertices.
fn parse_lit_area_bezier(prefab: &PrefabInstance) -> Option<LitAreaPolygon> {
    let od = prefab.override_data.as_ref()?;
    let text = &od.raw_text;

    // Find bezier node array
    let nodes_start = text.find("Array nodes")?;
    let after_nodes = &text[nodes_start..];

    // Get array size
    let size_pos = after_nodes.find("size = ")?;
    let after_size = &after_nodes[size_pos + 7..];
    let end = after_size.find(|c: char| !c.is_ascii_digit())?;
    let node_count: usize = after_size[..end].parse().ok()?;
    if node_count < 2 {
        return None;
    }

    // Parse bezierPointCount
    let bpc = if let Some(bpc_pos) = text.find("bezierPointCount = ") {
        let after_bpc = &text[bpc_pos + 19..];
        let end = after_bpc
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_bpc.len());
        after_bpc[..end].parse::<usize>().unwrap_or(100)
    } else {
        100
    };

    // Parse each bezier node: position + tangent0 (forward) + tangent1 (backward)
    struct BezierNode {
        px: f32,
        py: f32,
        t0x: f32,
        t0y: f32,
        t1x: f32,
        t1y: f32,
    }

    let mut nodes = Vec::with_capacity(node_count);
    let mut search = after_nodes;

    for _ in 0..node_count {
        // Find "Vector3 position" followed by Float x/y
        let pos_idx = search.find("Vector3 position")?;
        search = &search[pos_idx..];

        let px = parse_next_float(search, "Float x = ")?;
        let py = parse_next_float(search, "Float y = ")?;

        // Find tangent0 / tangent1
        let t0_idx = search.find("Vector3 tangent0")?;
        let t0_search = &search[t0_idx..];
        let t0x = parse_next_float(t0_search, "Float x = ")?;
        let t0y = parse_next_float(t0_search, "Float y = ")?;

        let t1_idx = search.find("Vector3 tangent1")?;
        let t1_search = &search[t1_idx..];
        let t1x = parse_next_float(t1_search, "Float x = ")?;
        let t1y = parse_next_float(t1_search, "Float y = ")?;

        nodes.push(BezierNode {
            px,
            py,
            t0x,
            t0y,
            t1x,
            t1y,
        });

        // Advance past this node's tangent1 section
        search = &search[t1_idx + 20..];
    }

    // Evaluate cubic bezier curve to generate polygon points
    // Unity formula: B(t) = (1-t)³·P0 + 3(1-t)²·t·(P0+T0) + 3(1-t)·t²·(P1+T1) + t³·P1
    let count = nodes.len();
    // Use the full bezierPointCount from level data (120-391) for smooth curves
    let render_points = bpc.max(count * 10);
    let mut polygon = Vec::with_capacity(render_points);
    let world_x = prefab.position.x;
    let world_y = prefab.position.y;

    for i in 1..=render_points {
        let ct = i as f32 / render_points as f32;
        // Map ct → segment index + local t
        let num = ct / (1.0 / count as f32);
        let seg = if (ct - 1.0).abs() < 1e-6 {
            count - 1
        } else {
            (num.floor() as usize).min(count - 1)
        };
        let t = if (ct - 1.0).abs() < 1e-6 {
            1.0
        } else {
            num % 1.0
        };

        let n0 = &nodes[seg];
        let n1 = &nodes[(seg + 1) % count];

        // Control points:
        // P0 = position of node[seg]
        // C0 = P0 + forwardTangent of node[seg]   (tangent0)
        // C1 = P1 + backwardTangent of node[seg+1] (tangent1)
        // P1 = position of node[seg+1]
        let p0x = n0.px;
        let p0y = n0.py;
        let c0x = p0x + n0.t0x;
        let c0y = p0y + n0.t0y;
        let p1x = n1.px;
        let p1y = n1.py;
        let c1x = p1x + n1.t1x;
        let c1y = p1y + n1.t1y;

        let omt = 1.0 - t;
        let omt2 = omt * omt;
        let omt3 = omt2 * omt;
        let t2 = t * t;
        let t3 = t2 * t;

        let x = omt3 * p0x + 3.0 * omt2 * t * c0x + 3.0 * omt * t2 * c1x + t3 * p1x;
        let y = omt3 * p0y + 3.0 * omt2 * t * c0y + 3.0 * omt * t2 * c1y + t3 * p1y;

        polygon.push((world_x + x, world_y + y));
    }

    if polygon.len() < 3 {
        return None;
    }

    // Compute border vertices by expanding polygon outward along vertex normals.
    // BezierMesh border strip uses borderWidth=0.5 (from level override or prefab).
    let border_width = parse_border_width(prefab).unwrap_or(0.5);
    let border_vertices = expand_polygon(&polygon, border_width);

    log::info!(
        "LitArea at ({:.1}, {:.1}): {} bezier nodes → {} polygon vertices, borderWidth={:.2}",
        world_x,
        world_y,
        count,
        polygon.len(),
        border_width
    );

    Some(LitAreaPolygon {
        vertices: polygon,
        border_vertices,
    })
}

/// Helper: find the next occurrence of a pattern like "Float x = " and parse the float value.
fn parse_next_float(text: &str, pattern: &str) -> Option<f32> {
    let pos = text.find(pattern)?;
    let after = &text[pos + pattern.len()..];
    let end = after
        .find(|c: char| {
            !c.is_ascii_digit() && c != '.' && c != '-' && c != 'E' && c != 'e' && c != '+'
        })
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Parse borderWidth from a LitArea prefab's override data.
fn parse_border_width(prefab: &PrefabInstance) -> Option<f32> {
    let od = prefab.override_data.as_ref()?;
    parse_next_float(&od.raw_text, "Float borderWidth = ")
}

/// Expand a closed polygon outward by `width` along each vertex's averaged normal.
fn expand_polygon(polygon: &[(f32, f32)], width: f32) -> Vec<(f32, f32)> {
    let n = polygon.len();
    if n < 3 || width <= 0.0 {
        return polygon.to_vec();
    }
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let prev = polygon[(i + n - 1) % n];
        let curr = polygon[i];
        let next = polygon[(i + 1) % n];
        // Compute outward normal as average of two edge normals
        // Edge prev→curr normal (pointing outward for CCW polygon)
        let e1x = curr.0 - prev.0;
        let e1y = curr.1 - prev.1;
        let len1 = (e1x * e1x + e1y * e1y).sqrt().max(1e-6);
        let n1x = -e1y / len1;
        let n1y = e1x / len1;
        // Edge curr→next normal
        let e2x = next.0 - curr.0;
        let e2y = next.1 - curr.1;
        let len2 = (e2x * e2x + e2y * e2y).sqrt().max(1e-6);
        let n2x = -e2y / len2;
        let n2y = e2x / len2;
        // Average and normalize
        let nx = n1x + n2x;
        let ny = n1y + n2y;
        let len = (nx * nx + ny * ny).sqrt().max(1e-6);
        result.push((curr.0 + width * nx / len, curr.1 + width * ny / len));
    }
    result
}

/// Parse a PointLightSource-bearing prefab into a circular polygon.
/// Returns None if the prefab is not a known light source type.
fn parse_point_light(prefab: &PrefabInstance) -> Option<LitAreaPolygon> {
    // Default (size, borderWidth) from prefab data
    let (default_size, default_border) = if prefab.name.starts_with("LitCrystal") {
        (2.0f32, 0.5f32)
    } else if prefab.name.starts_with("LitMushroom") {
        (1.0, 0.5)
    } else if prefab.name == "GoalArea_MM_Light" || prefab.name == "GoalArea_MM_Grey_Light" {
        (3.0, 0.5)
    } else if prefab.name == "Cake" || prefab.name == "CakeFloating" {
        (2.0, 0.5)
    } else if prefab.name == "SecretStatue" {
        (3.0, 0.5)
    } else if prefab.name == "Part_MetalFrame_11_SET" {
        (5.0, 0.5)
    } else if prefab.name.ends_with("Crate") {
        // WoodenCrate, MetalCrate, GoldenCrate, MarbleCrate, CardboardCrate, BronzeCrate, GlassCrate
        (3.5, 0.3)
    } else if prefab.name.starts_with("Part_PointLight") {
        // Part_PointLight_04_SET is size=14, others are 7
        if prefab.name.contains("_04") {
            (14.0, 0.5)
        } else {
            (7.0, 0.5)
        }
    } else if prefab.name.starts_with("Part_SpotLight") {
        (7.0, 0.5) // TODO: beam shape, currently approximated as circle
    } else {
        return None;
    };

    // Check if override data specifies a custom size
    let size = if let Some(ref od) = prefab.override_data {
        if let Some(pos) = od.raw_text.find("Float size = ") {
            let after = &od.raw_text[pos + 13..];
            let end = after
                .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .unwrap_or(after.len());
            after[..end].parse::<f32>().unwrap_or(default_size)
        } else {
            default_size
        }
    } else {
        default_size
    };

    // Generate circle polygons: inner (light) and outer (border)
    let border_width = default_border;
    let segments = 64;
    let cx = prefab.position.x;
    let cy = prefab.position.y;
    let mut vertices = Vec::with_capacity(segments);
    let mut border_vertices = Vec::with_capacity(segments);
    let outer_size = size + border_width;
    for i in 0..segments {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (segments as f32);
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        vertices.push((cx + size * cos_a, cy + size * sin_a));
        border_vertices.push((cx + outer_size * cos_a, cy + outer_size * sin_a));
    }

    log::info!(
        "{} at ({:.1}, {:.1}): size={:.2}, border={:.2} → {} vertex circle",
        prefab.name,
        cx,
        cy,
        size,
        border_width,
        segments
    );

    Some(LitAreaPolygon {
        vertices,
        border_vertices,
    })
}

/// Build dark overlay meshes (complement + ring) without drawing.
/// Returns (dark_complement_mesh, optional_ring_mesh).
pub(super) fn build_dark_overlay_meshes(
    rect: egui::Rect,
    camera: &Camera,
    canvas_center: egui::Vec2,
    lit_areas: &[LitAreaPolygon],
) -> (egui::Mesh, Option<egui::Mesh>) {
    let dark_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200);
    let border_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80);

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
        return (m, None);
    }

    let to_screen = |wx: f32, wy: f32| -> egui::Pos2 {
        camera.world_to_screen(Vec2 { x: wx, y: wy }, canvas_center)
    };

    let border_polys: Vec<Vec<egui::Pos2>> = lit_areas
        .iter()
        .filter_map(|la| {
            let pts: Vec<egui::Pos2> = la
                .border_vertices
                .iter()
                .map(|&(wx, wy)| to_screen(wx, wy))
                .collect();
            if pts.len() >= 3 { Some(pts) } else { None }
        })
        .collect();

    let inner_polys: Vec<Vec<egui::Pos2>> = lit_areas
        .iter()
        .filter_map(|la| {
            let pts: Vec<egui::Pos2> = la
                .vertices
                .iter()
                .map(|&(wx, wy)| to_screen(wx, wy))
                .collect();
            if pts.len() >= 3 { Some(pts) } else { None }
        })
        .collect();

    if border_polys.is_empty() {
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
        return (m, None);
    }

    let mut dark_mesh = build_scanline_complement_mesh(rect, &border_polys, dark_color);
    if dark_mesh.vertices.is_empty() {
        let uv = egui::pos2(0.0, 0.0);
        emit_quad(
            &mut dark_mesh,
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
    }

    let ring_mesh = if !inner_polys.is_empty() {
        let m = build_scanline_ring_mesh(rect, &border_polys, &inner_polys, border_color);
        if m.vertices.is_empty() { None } else { Some(m) }
    } else {
        None
    };

    (dark_mesh, ring_mesh)
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
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
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
        let top_xs = find_poly_x_intersections(y_top + eps, polys);
        let bot_xs = find_poly_x_intersections(y_bot - eps, polys);

        if top_xs.len() == bot_xs.len() {
            let count = top_xs.len();
            let mut prev_top = rect.left();
            let mut prev_bot = rect.left();
            for i in 0..count {
                if i % 2 == 0 {
                    let x_rt = top_xs[i].min(rect.right());
                    let x_rb = bot_xs[i].min(rect.right());
                    if x_rt - prev_top > 0.5 || x_rb - prev_bot > 0.5 {
                        emit_quad(
                            &mut mesh,
                            Trapezoid {
                                left_top: prev_top,
                                right_top: x_rt,
                                left_bot: prev_bot,
                                right_bot: x_rb,
                                y_top,
                                y_bot,
                            },
                            color,
                            uv,
                        );
                    }
                }
                prev_top = top_xs[i].max(rect.left());
                prev_bot = bot_xs[i].max(rect.left());
            }
            if count.is_multiple_of(2)
                && (rect.right() - prev_top > 0.5 || rect.right() - prev_bot > 0.5)
            {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: prev_top,
                        right_top: rect.right(),
                        left_bot: prev_bot,
                        right_bot: rect.right(),
                        y_top,
                        y_bot,
                    },
                    color,
                    uv,
                );
            }
        } else {
            let y_mid = (y_top + y_bot) * 0.5;
            let mid_xs = find_poly_x_intersections(y_mid, polys);
            let mut prev_x = rect.left();
            for (i, &x) in mid_xs.iter().enumerate() {
                let x = x.clamp(rect.left(), rect.right());
                if i % 2 == 0 && x - prev_x > 0.5 {
                    emit_quad(
                        &mut mesh,
                        Trapezoid {
                            left_top: prev_x,
                            right_top: x,
                            left_bot: prev_x,
                            right_bot: x,
                            y_top,
                            y_bot,
                        },
                        color,
                        uv,
                    );
                }
                prev_x = x.max(rect.left());
            }
            if mid_xs.len().is_multiple_of(2) && rect.right() - prev_x > 0.5 {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: prev_x,
                        right_top: rect.right(),
                        left_bot: prev_x,
                        right_bot: rect.right(),
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
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
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

        // Get intersections from both polygon sets
        let outer_top = find_poly_x_intersections(y_top + eps, outer_polys);
        let outer_bot = find_poly_x_intersections(y_bot - eps, outer_polys);
        let inner_top = find_poly_x_intersections(y_top + eps, inner_polys);
        let inner_bot = find_poly_x_intersections(y_bot - eps, inner_polys);

        // Merge inner intersections into outer to create ring segments
        // Inside outer but outside inner = border ring
        if outer_top.len() != outer_bot.len() || inner_top.len() != inner_bot.len() {
            continue;
        }

        // Combine all X coords and classify each interval
        let mut all_top: Vec<f32> = Vec::new();
        all_top.extend_from_slice(&outer_top);
        all_top.extend_from_slice(&inner_top);
        all_top.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_top.dedup_by(|a, b| (*a - *b).abs() < 0.1);

        let mut all_bot: Vec<f32> = Vec::new();
        all_bot.extend_from_slice(&outer_bot);
        all_bot.extend_from_slice(&inner_bot);
        all_bot.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_bot.dedup_by(|a, b| (*a - *b).abs() < 0.1);

        if all_top.len() != all_bot.len() {
            continue;
        }

        let count = all_top.len();
        let mut prev_top = rect.left();
        let mut prev_bot = rect.left();
        for i in 0..count {
            let x_top = all_top[i].clamp(rect.left(), rect.right());
            let x_bot = all_bot[i].clamp(rect.left(), rect.right());
            // Sample midpoint of the interval to check if inside ring
            let mid_x_top = (prev_top + x_top) * 0.5;
            let mid_x_bot = (prev_bot + x_bot) * 0.5;
            let mid_y = (y_top + y_bot) * 0.5;
            let mid_x = (mid_x_top + mid_x_bot) * 0.5;
            let in_outer = is_point_in_polys(mid_x, mid_y, outer_polys);
            let in_inner = is_point_in_polys(mid_x, mid_y, inner_polys);
            if in_outer && !in_inner && (x_top - prev_top > 0.5 || x_bot - prev_bot > 0.5) {
                emit_quad(
                    &mut mesh,
                    Trapezoid {
                        left_top: prev_top,
                        right_top: x_top,
                        left_bot: prev_bot,
                        right_bot: x_bot,
                        y_top,
                        y_bot,
                    },
                    color,
                    uv,
                );
            }
            prev_top = x_top;
            prev_bot = x_bot;
        }
    }
    mesh
}

/// Check if a point is inside any of the polygons (ray-casting even-odd rule).
fn is_point_in_polys(px: f32, py: f32, polys: &[Vec<egui::Pos2>]) -> bool {
    let mut crossings = 0;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let (y1, y2) = (poly[i].y, poly[j].y);
            if (y1 < py && y2 >= py) || (y2 < py && y1 >= py) {
                let t = (py - y1) / (y2 - y1);
                if poly[i].x + t * (poly[j].x - poly[i].x) < px {
                    crossings += 1;
                }
            }
        }
    }
    crossings % 2 == 1
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

/// Find X intersections of all polygon edges with a horizontal line at the given Y.
/// Returns sorted X values. Used for even-odd scanline fill.
fn find_poly_x_intersections(y: f32, polys: &[Vec<egui::Pos2>]) -> Vec<f32> {
    let mut xs = Vec::new();
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let (y1, y2) = (poly[i].y, poly[j].y);
            if (y1 < y && y2 >= y) || (y2 < y && y1 >= y) {
                let t = (y - y1) / (y2 - y1);
                xs.push(poly[i].x + t * (poly[j].x - poly[i].x));
            }
        }
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs
}

// ── Dark overlay draw method (extracted from show()) ──

impl LevelRenderer {
    pub(super) fn draw_dark_overlay(
        &mut self,
        painter: &egui::Painter,
        canvas_center: egui::Vec2,
        rect: egui::Rect,
    ) {
        if let (Some(resources), Some(gpu_meshes)) =
            (&self.dark_resources, &self.dark_gpu_meshes)
        {
            painter.add(dark_shader::make_dark_callback(
                rect,
                resources.clone(),
                gpu_meshes.clone(),
                [self.camera.center.x, self.camera.center.y],
                [rect.left(), rect.top(), rect.width(), rect.height()],
                self.camera.zoom,
            ));
        } else {
            let key = (
                self.camera.center.x,
                self.camera.center.y,
                self.camera.zoom,
                rect.width(),
                rect.height(),
            );
            if key != self.dark_overlay_key || self.dark_overlay_mesh.is_none() {
                let (dark_mesh, ring_mesh) = build_dark_overlay_meshes(
                    rect,
                    &self.camera,
                    canvas_center,
                    &self.lit_area_polygons,
                );
                self.dark_overlay_mesh = Some(dark_mesh);
                self.dark_overlay_ring = ring_mesh;
                self.dark_overlay_key = key;
            }
            if let Some(ref mesh) = self.dark_overlay_mesh
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
}
