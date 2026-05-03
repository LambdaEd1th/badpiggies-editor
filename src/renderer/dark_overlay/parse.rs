//! Parsing of LitArea / PointLight prefabs and ConstructionGrid into lit-area polygons.

use crate::types::*;

use super::super::grid::ConstructionGrid;
use super::{LIT_AREA_BORDER_ALPHA, LitAreaPolygon, POINT_LIGHT_BORDER_ALPHA};

/// Parse `m_darkLevel` from LevelManager and LitArea bezier curves from the level.
pub(in crate::renderer) fn parse_dark_level_data(
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
        border_alpha: LIT_AREA_BORDER_ALPHA,
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

fn build_point_light_polygon(cx: f32, cy: f32, size: f32, border_width: f32) -> LitAreaPolygon {
    let segments = 64;
    let mut vertices = Vec::with_capacity(segments);
    let mut border_vertices = if border_width > 0.0 {
        Vec::with_capacity(segments)
    } else {
        Vec::new()
    };
    let outer_size = size + border_width;
    for i in 0..segments {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (segments as f32);
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        vertices.push((cx + size * cos_a, cy + size * sin_a));
        if border_width > 0.0 {
            border_vertices.push((cx + outer_size * cos_a, cy + outer_size * sin_a));
        }
    }
    LitAreaPolygon {
        vertices,
        border_vertices,
        border_alpha: POINT_LIGHT_BORDER_ALPHA,
    }
}

pub(in crate::renderer) fn construction_grid_start_light(grid: &ConstructionGrid) -> LitAreaPolygon {
    let mut cx = grid.base_x;
    let cy = grid.base_y + 0.5 * grid.grid_height as f32 - 0.5;
    if grid.grid_width % 2 == 0 {
        cx += 0.5;
    }
    let size = 1.0 + 0.5 * grid.grid_width.max(grid.grid_height) as f32;
    build_point_light_polygon(cx, cy, size, 0.5)
}

/// Parse a PointLightSource-bearing prefab into a circular polygon.
/// Returns None if the prefab is not a known light source type.
fn parse_point_light(prefab: &PrefabInstance) -> Option<LitAreaPolygon> {
    // Default (size, borderWidth) from prefab data. Unity draws point-light
    // borders as a separate feathered light mesh. In the editor we approximate
    // that transition as a lighter dark-overlay penumbra.
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

    let border_width = prefab
        .override_data
        .as_ref()
        .and_then(|od| parse_next_float(&od.raw_text, "Float borderWidth = "))
        .unwrap_or(default_border);
    let cx = prefab.position.x;
    let cy = prefab.position.y;

    log::info!(
        "{} at ({:.1}, {:.1}): size={:.2}, border={:.2} → 64 vertex circle",
        prefab.name,
        cx,
        cy,
        size,
        border_width
    );

    Some(build_point_light_polygon(cx, cy, size, border_width))
}

