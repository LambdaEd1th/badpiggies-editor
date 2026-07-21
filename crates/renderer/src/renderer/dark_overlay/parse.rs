//! Parsing of LitArea / PointLight prefabs and ConstructionGrid into lit-area polygons.

use crate::data::assets;
use crate::domain::prefab_asset::PrefabAssetDocument;
use crate::domain::types::*;
use crate::unity_runtime::components::{BezierCurve, BezierMesh, LevelManager, PointLightSource};
use crate::unity_runtime::scene::{Scene, SceneValue};

use super::super::grid::ConstructionGrid;
use super::{LIT_AREA_BORDER_ALPHA, LitAreaPolygon, POINT_LIGHT_BORDER_ALPHA};

const CONSTRUCTION_GRID_START_LIGHT_BORDER_WIDTH: f32 = 0.5;
const SERIALIZED_LIT_AREA_BORDER_WIDTH: f32 = 0.5;

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
                && let Some((scene, root)) = Scene::from_override_text(&od.raw_text)
                && let Some((_, lm)) = scene.get_component_of::<LevelManager>(root)
                && lm.dark_level == Some(true)
            {
                *dark_level = true;
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
    let (scene, root) = Scene::from_override_text(&od.raw_text)?;
    let (_, bezier) = scene.get_component_of::<BezierCurve>(root)?;
    let bezier_mesh = scene
        .get_component_of::<BezierMesh>(root)
        .map(|(_, mesh)| mesh);
    let nodes = bezier.nodes.as_ref()?;
    let node_count = nodes.len();
    if node_count < 2 {
        return None;
    }

    let bpc = bezier
        .bezier_point_count
        .or_else(lit_area_prefab_bezier_point_count)
        .map(|count| count.max(1) as usize)
        .unwrap_or(1);

    // Local view onto the typed bezier nodes (2D projection).
    struct BN {
        px: f32,
        py: f32,
        t0x: f32,
        t0y: f32,
        t1x: f32,
        t1y: f32,
    }
    let nodes: Vec<BN> = nodes
        .iter()
        .map(|n| BN {
            px: n.position.x,
            py: n.position.y,
            t0x: n.tangent0.x,
            t0y: n.tangent0.y,
            t1x: n.tangent1.x,
            t1y: n.tangent1.y,
        })
        .collect();

    if nodes.len() < 2 {
        return None;
    }

    // Evaluate cubic bezier curve to generate polygon points
    // Unity formula: B(t) = (1-t)³·P0 + 3(1-t)²·t·(P0+T0) + 3(1-t)·t²·(P1+T1) + t³·P1
    let count = nodes.len();
    // Use the full bezierPointCount from level data (120-391) for smooth curves
    let render_points = bpc.max(count * 10);
    let mut local_polygon = Vec::with_capacity(render_points);

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

        local_polygon.push((x, y));
    }

    if local_polygon.len() < 3 {
        return None;
    }

    // Unity's BezierMesh border strip is centered on the original curve, but
    // the main lit polygon writes depth in front of it. The visible penumbra
    // therefore starts at the lit boundary and extends outward by borderWidth.
    let border_width = resolved_lit_area_border_width(bezier_mesh);
    let (local_border_inner, local_border_outer) = if border_width > 0.0 {
        (
            local_polygon.clone(),
            offset_polygon(&local_polygon, border_width),
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let polygon = transform_lit_area_polygon(prefab, &local_polygon);
    let border_inner_vertices = transform_lit_area_polygon(prefab, &local_border_inner);
    let border_vertices = transform_lit_area_polygon(prefab, &local_border_outer);

    log::info!(
        "LitArea at ({:.1}, {:.1}) scale=({:.2}, {:.2}): {} bezier nodes → {} polygon vertices, borderWidth={:.2}",
        prefab.position.x,
        prefab.position.y,
        prefab.scale.x,
        prefab.scale.y,
        count,
        polygon.len(),
        border_width
    );

    Some(LitAreaPolygon {
        vertices: polygon,
        border_vertices,
        border_inner_vertices,
        border_alpha: LIT_AREA_BORDER_ALPHA,
    })
}

fn resolved_lit_area_border_width(mesh: Option<&BezierMesh>) -> f32 {
    if let Some(mesh) = mesh {
        if let Some(border_width) = mesh.border_width {
            return border_width;
        }
        if bezier_mesh_has_serialized_border_polygon(mesh) {
            return SERIALIZED_LIT_AREA_BORDER_WIDTH;
        }
    }
    lit_area_prefab_border_width()
}

fn lit_area_prefab_bezier_point_count() -> Option<i32> {
    prefab_asset_document("LitArea")
        .and_then(|prefab| prefab.root_component("BezierCurve").cloned())
        .and_then(|component| component.field_i32("bezierPointCount"))
}

fn lit_area_prefab_border_width() -> f32 {
    prefab_asset_document("LitArea")
        .and_then(|prefab| prefab.root_component("BezierMesh").cloned())
        .and_then(|component| component.field_f32("borderWidth"))
        .unwrap_or(0.0)
}

fn bezier_mesh_has_serialized_border_polygon(mesh: &BezierMesh) -> bool {
    mesh.extra.iter().any(|(field, value)| {
        field == "borderPolygon" && scene_value_contains_object_reference(value)
    })
}

fn scene_value_contains_object_reference(value: &SceneValue) -> bool {
    match value {
        SceneValue::ObjectReference(_) => true,
        SceneValue::Generic(entries) => entries.iter().any(|(field, entry)| {
            field == "_unresolvedObjectReference"
                && matches!(entry, SceneValue::Integer(index) if *index != 0)
        }),
        _ => false,
    }
}

fn transform_lit_area_polygon(prefab: &PrefabInstance, points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    points
        .iter()
        .map(|&(x, y)| {
            (
                prefab.position.x + x * prefab.scale.x,
                prefab.position.y + y * prefab.scale.y,
            )
        })
        .collect()
}

fn prefab_asset_document(prefab_name: &str) -> Option<PrefabAssetDocument> {
    let asset_path = format!("Assets/Prefab/{prefab_name}.prefab");
    let text = assets::read_pathname_text(&asset_path)?;
    PrefabAssetDocument::parse(&text)
}

fn point_light_prefab_defaults(prefab_name: &str) -> Option<(f32, f32)> {
    let prefab = prefab_asset_document(prefab_name)?;
    let component = prefab.root_component("PointLightSource")?;
    Some((
        component.field_f32("size")?,
        component.field_f32("borderWidth")?,
    ))
}

/// Offset a closed polygon along its averaged outward normals.
fn offset_polygon(polygon: &[(f32, f32)], distance: f32) -> Vec<(f32, f32)> {
    let n = polygon.len();
    if n < 3 || distance.abs() <= f32::EPSILON {
        return polygon.to_vec();
    }
    let signed_area = polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(n)
        .fold(0.0, |area, (a, b)| area + (a.0 * b.1 - b.0 * a.1));
    if signed_area.abs() <= 1e-4 {
        return polygon.to_vec();
    }
    let outward_sign = if signed_area >= 0.0 { -1.0 } else { 1.0 };
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let prev = polygon[(i + n - 1) % n];
        let curr = polygon[i];
        let next = polygon[(i + 1) % n];
        let e1x = curr.0 - prev.0;
        let e1y = curr.1 - prev.1;
        let len1 = (e1x * e1x + e1y * e1y).sqrt().max(1e-6);
        let n1x = outward_sign * -e1y / len1;
        let n1y = outward_sign * e1x / len1;
        let e2x = next.0 - curr.0;
        let e2y = next.1 - curr.1;
        let len2 = (e2x * e2x + e2y * e2y).sqrt().max(1e-6);
        let n2x = outward_sign * -e2y / len2;
        let n2y = outward_sign * e2x / len2;
        let nx = n1x + n2x;
        let ny = n1y + n2y;
        let len = (nx * nx + ny * ny).sqrt().max(1e-6);
        result.push((curr.0 + distance * nx / len, curr.1 + distance * ny / len));
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
    let border_inner_vertices = if border_width > 0.0 {
        vertices.clone()
    } else {
        Vec::new()
    };
    LitAreaPolygon {
        vertices,
        border_vertices,
        border_inner_vertices,
        border_alpha: POINT_LIGHT_BORDER_ALPHA,
    }
}

pub(in crate::renderer) fn construction_grid_start_light(
    grid: &ConstructionGrid,
) -> LitAreaPolygon {
    // LightManager sets `startPls.size = 1 + 0.5 * max(GridWidth, GridHeight)` and
    // PointLightContainer gives the generated border mask a fixed `borderWidth = 0.5`.
    let mut cx = grid.base_x;
    let cy = grid.base_y + 0.5 * grid.grid_height as f32 - 0.5;
    if grid.grid_width % 2 == 0 {
        cx += 0.5;
    }
    let size = 1.0 + 0.5 * grid.grid_width.max(grid.grid_height) as f32;
    build_point_light_polygon(cx, cy, size, CONSTRUCTION_GRID_START_LIGHT_BORDER_WIDTH)
}

/// Parse a PointLightSource-bearing prefab into a circular polygon.
/// Returns None if the prefab is not a known light source type.
fn parse_point_light(prefab: &PrefabInstance) -> Option<LitAreaPolygon> {
    // Unity draws point-light borders as a separate feathered light mesh. In the
    // editor we approximate that transition as a lighter dark-overlay penumbra.
    let (default_size, default_border) = point_light_prefab_defaults(&prefab.name)?;

    let (size, border_width) = prefab
        .override_data
        .as_ref()
        .and_then(|od| {
            let (scene, root) = Scene::from_override_text(&od.raw_text)?;
            let (_, pls) = scene.get_component_of::<PointLightSource>(root)?;
            Some((
                pls.size.unwrap_or(default_size),
                pls.border_width.unwrap_or(default_border),
            ))
        })
        .unwrap_or((default_size, default_border));
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

#[cfg(test)]
mod tests {
    use super::{
        construction_grid_start_light, offset_polygon, parse_dark_level_data,
        parse_lit_area_bezier, parse_point_light,
    };
    use crate::domain::parser::parse_level;
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };
    use crate::renderer::grid::{ConstructionGrid, ConstructionGridCellStyle};
    use crate::unity_runtime::components::LevelManager;
    use crate::unity_runtime::scene::Scene;
    const LEVEL_MANAGER_OVERRIDE: &str =
        "GameObject LevelManager\n\tComponent LevelManager\n\t\tBoolean m_darkLevel = True\n";

    const LIT_AREA_OVERRIDE: &str = "GameObject LitArea\n\tComponent MentalTools.BezierCurve\n\t\tInteger bezierPointCount = 6\n\t\tGeneric bezierCurve\n\t\t\tArray nodes\n\t\t\t\tArraySize size = 2\n\t\t\t\tElement 0\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 1\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 4\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n\tComponent MentalTools.BezierMesh\n\t\tFloat borderWidth = 0.5\n";

    const LIT_AREA_OVERRIDE_WITHOUT_BORDER: &str = "GameObject LitArea\n\tComponent MentalTools.BezierCurve\n\t\tInteger bezierPointCount = 6\n\t\tGeneric bezierCurve\n\t\t\tArray nodes\n\t\t\t\tArraySize size = 2\n\t\t\t\tElement 0\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 1\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 4\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n";

    const LIT_AREA_OVERRIDE_WITHOUT_PREFAB_DEFAULTED_FIELDS: &str = "GameObject LitArea\n\tComponent MentalTools.BezierCurve\n\t\tGeneric bezierCurve\n\t\t\tArray nodes\n\t\t\t\tArraySize size = 2\n\t\t\t\tElement 0\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 1\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 4\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 1\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -1\n\t\t\t\t\t\t\tFloat y = 0\n";

    const POINT_LIGHT_OVERRIDE: &str = "GameObject LitCrystal_01\n\tComponent PointLightSource\n\t\tFloat size = 4.25\n\t\tFloat borderWidth = 0.7\n";

    #[test]
    fn parses_dark_level_flag_and_lit_area_from_ast() {
        let mut dark_level = false;
        let mut lit_areas = Vec::new();
        let level = LevelData {
            objects: vec![
                LevelObject::Prefab(prefab(
                    "LevelManager",
                    Vec3::default(),
                    LEVEL_MANAGER_OVERRIDE,
                )),
                LevelObject::Prefab(prefab(
                    "LitArea",
                    Vec3 {
                        x: 10.0,
                        y: 20.0,
                        z: 0.0,
                    },
                    LIT_AREA_OVERRIDE,
                )),
            ],
            roots: vec![0, 1],
        };

        parse_dark_level_data(&level, &mut dark_level, &mut lit_areas);

        assert!(dark_level);
        assert_eq!(lit_areas.len(), 1);
        assert!(lit_areas[0].vertices.len() >= 20);
        assert_eq!(
            lit_areas[0].vertices.len(),
            lit_areas[0].border_vertices.len()
        );
        assert_eq!(
            lit_areas[0].vertices.len(),
            lit_areas[0].border_inner_vertices.len()
        );
    }

    #[test]
    fn offset_polygon_expands_and_shrinks_square_for_both_windings() {
        let polygons = [
            vec![(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0)],
            vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        ];

        for polygon in polygons {
            let expanded = offset_polygon(&polygon, 0.5);
            let shrunk = offset_polygon(&polygon, -0.5);
            let bounds = |points: &[(f32, f32)]| {
                points.iter().fold(
                    (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                    |(min_x, min_y, max_x, max_y), &(x, y)| {
                        (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
                    },
                )
            };

            let original_bounds = bounds(&polygon);
            let expanded_bounds = bounds(&expanded);
            let shrunk_bounds = bounds(&shrunk);

            assert!(expanded_bounds.0 < original_bounds.0);
            assert!(expanded_bounds.1 < original_bounds.1);
            assert!(expanded_bounds.2 > original_bounds.2);
            assert!(expanded_bounds.3 > original_bounds.3);
            assert!(shrunk_bounds.0 > original_bounds.0);
            assert!(shrunk_bounds.1 > original_bounds.1);
            assert!(shrunk_bounds.2 < original_bounds.2);
            assert!(shrunk_bounds.3 < original_bounds.3);
        }
    }

    #[test]
    fn dark_sandbox_bytes_parse_as_dark_level() {
        let Some(level_path) = crate::test_support::external_test_level(
            "assetbundles/episode_sandbox_levels_2.unity3d/Episode_6_Dark Sandbox_data.bytes",
        ) else {
            return;
        };
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        let level = parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

        let level_manager = level
            .objects
            .iter()
            .find_map(|object| match object {
                LevelObject::Prefab(prefab) if prefab.name == "LevelManager" => Some(prefab),
                _ => None,
            })
            .expect("expected LevelManager prefab in dark sandbox");
        let override_data = level_manager
            .override_data
            .as_ref()
            .expect("expected LevelManager override data in dark sandbox");
        assert!(
            override_data.raw_text.contains("m_darkLevel = True"),
            "expected parsed LevelManager override text to keep m_darkLevel"
        );

        let (scene, root) = Scene::from_override_text(&override_data.raw_text)
            .expect("expected LevelManager override to parse as Scene text");
        let (_, level_manager_component) = scene
            .get_component_of::<LevelManager>(root)
            .expect("expected parsed Scene to include LevelManager component");
        assert_eq!(
            level_manager_component.dark_level,
            Some(true),
            "expected Scene parsing to preserve LevelManager.m_darkLevel"
        );

        let mut dark_level = false;
        let mut lit_areas = Vec::new();
        parse_dark_level_data(&level, &mut dark_level, &mut lit_areas);

        assert!(dark_level, "expected dark sandbox to set m_darkLevel");
        assert!(
            !lit_areas.is_empty(),
            "expected dark sandbox to produce at least one lit area"
        );
    }

    #[test]
    fn dark_sandbox_lit_area_gets_border_from_serialized_border_polygon() {
        let Some(level_path) = crate::test_support::external_test_level(
            "assetbundles/episode_sandbox_levels_2.unity3d/Episode_6_Dark Sandbox_data.bytes",
        ) else {
            return;
        };
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        let level = parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

        let lit_area = level
            .objects
            .iter()
            .find_map(|object| match object {
                LevelObject::Prefab(prefab) if prefab.name == "LitArea" => Some(prefab),
                _ => None,
            })
            .expect("expected Dark Sandbox to contain a LitArea prefab");

        let polygon =
            parse_lit_area_bezier(lit_area).expect("expected Dark Sandbox LitArea polygon");

        assert!(!polygon.border_vertices.is_empty());
        assert_eq!(polygon.vertices.len(), polygon.border_vertices.len());
        assert_eq!(polygon.vertices.len(), polygon.border_inner_vertices.len());
    }

    #[test]
    fn parses_point_light_size_and_border_from_ast() {
        let polygon = parse_point_light(&prefab(
            "LitCrystal_01",
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 0.0,
            },
            POINT_LIGHT_OVERRIDE,
        ))
        .expect("expected point light polygon");

        assert!((polygon.vertices[0].0 - 6.25).abs() < 1e-6);
        assert!((polygon.border_vertices[0].0 - 6.95).abs() < 1e-6);
    }

    #[test]
    fn parses_point_light_defaults_from_prefab_when_override_is_missing() {
        let polygon = parse_point_light(&prefab(
            "Part_PointLight_04_SET",
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 0.0,
            },
            "",
        ))
        .expect("expected point light polygon");

        assert!((polygon.vertices[0].0 - 16.0).abs() < 1e-6);
        assert!((polygon.border_vertices[0].0 - 16.5).abs() < 1e-6);
    }

    #[test]
    fn parses_goal_light_defaults_from_prefab_when_override_is_missing() {
        let polygon = parse_point_light(&prefab(
            "GoalArea_MM_Light",
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 0.0,
            },
            "",
        ))
        .expect("expected point light polygon");

        assert!((polygon.vertices[0].0 - 5.0).abs() < 1e-6);
        assert!((polygon.border_vertices[0].0 - 5.5).abs() < 1e-6);
    }

    #[test]
    fn parses_lit_area_border_width_from_bezier_mesh_component() {
        let polygon = parse_lit_area_bezier(&prefab("LitArea", Vec3::default(), LIT_AREA_OVERRIDE))
            .expect("expected lit area polygon");

        assert_eq!(polygon.vertices.len(), polygon.border_vertices.len());
        assert!(polygon.vertices.len() >= 20);
    }

    #[test]
    fn uses_lit_area_prefab_default_when_border_override_is_missing() {
        let polygon = parse_lit_area_bezier(&prefab(
            "LitArea",
            Vec3::default(),
            LIT_AREA_OVERRIDE_WITHOUT_BORDER,
        ))
        .expect("expected lit area polygon");

        assert!(polygon.border_vertices.is_empty());
        assert!(polygon.border_inner_vertices.is_empty());
    }

    #[test]
    fn uses_serialized_lit_area_border_default_when_border_polygon_exists() {
        const LIT_AREA_OVERRIDE_WITH_BORDER_POLYGON_ONLY: &str = "GameObject LitArea
	Component MentalTools.BezierCurve
		Integer bezierPointCount = 4
		Generic bezierCurve
			Array nodes
				ArraySize size = 4
				Element 0
					Generic data
						Vector3 position
							Float x = 0
							Float y = 0
						Vector3 tangent0
							Float x = 0
							Float y = 0
						Vector3 tangent1
							Float x = 0
							Float y = 0
				Element 1
					Generic data
						Vector3 position
							Float x = 2
							Float y = 0
						Vector3 tangent0
							Float x = 0
							Float y = 0
						Vector3 tangent1
							Float x = 0
							Float y = 0
				Element 2
					Generic data
						Vector3 position
							Float x = 2
							Float y = 2
						Vector3 tangent0
							Float x = 0
							Float y = 0
						Vector3 tangent1
							Float x = 0
							Float y = 0
				Element 3
					Generic data
						Vector3 position
							Float x = 0
							Float y = 2
						Vector3 tangent0
							Float x = 0
							Float y = 0
						Vector3 tangent1
							Float x = 0
							Float y = 0
	Component MentalTools.BezierMesh
		ObjectReference polygon = 197
		ObjectReference borderPolygon = 198
";

        let polygon = parse_lit_area_bezier(&prefab(
            "LitArea",
            Vec3::default(),
            LIT_AREA_OVERRIDE_WITH_BORDER_POLYGON_ONLY,
        ))
        .expect("expected lit area polygon");

        assert_eq!(polygon.vertices.len(), polygon.border_vertices.len());
        assert!(!polygon.border_vertices.is_empty());
        assert!(!polygon.border_inner_vertices.is_empty());
    }

    #[test]
    fn uses_lit_area_prefab_default_when_bezier_point_count_is_missing() {
        let polygon = parse_lit_area_bezier(&prefab(
            "LitArea",
            Vec3::default(),
            LIT_AREA_OVERRIDE_WITHOUT_PREFAB_DEFAULTED_FIELDS,
        ))
        .expect("expected lit area polygon");

        assert_eq!(polygon.vertices.len(), 20);
        assert!(polygon.border_vertices.is_empty());
        assert!(polygon.border_inner_vertices.is_empty());
    }

    #[test]
    fn scaled_lit_area_scales_curve_and_border_geometry() {
        const SCALED_LIT_AREA_OVERRIDE: &str = "GameObject LitArea\n\tComponent MentalTools.BezierCurve\n\t\tInteger bezierPointCount = 4\n\t\tGeneric bezierCurve\n\t\t\tArray nodes\n\t\t\t\tArraySize size = 4\n\t\t\t\tElement 0\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 1\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 2\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 2\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 2\n\t\t\t\t\t\t\tFloat y = 2\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\tElement 3\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 2\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = 0\n\t\t\t\t\t\t\tFloat y = 0\n\tComponent MentalTools.BezierMesh\n\t\tFloat borderWidth = 0.5\n";

        let unscaled = parse_lit_area_bezier(&prefab_with_scale(
            "LitArea",
            Vec3::default(),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            SCALED_LIT_AREA_OVERRIDE,
        ))
        .expect("expected lit area polygon");
        let scaled = parse_lit_area_bezier(&prefab_with_scale(
            "LitArea",
            Vec3::default(),
            Vec3 {
                x: 2.0,
                y: 2.0,
                z: 1.0,
            },
            SCALED_LIT_AREA_OVERRIDE,
        ))
        .expect("expected scaled lit area polygon");

        let max_x = |points: &[(f32, f32)]| points.iter().map(|(x, _)| *x).fold(f32::MIN, f32::max);
        let min_x = |points: &[(f32, f32)]| points.iter().map(|(x, _)| *x).fold(f32::MAX, f32::min);

        let unscaled_width = max_x(&unscaled.vertices) - min_x(&unscaled.vertices);
        let scaled_width = max_x(&scaled.vertices) - min_x(&scaled.vertices);
        let unscaled_ring_width =
            max_x(&unscaled.border_vertices) - max_x(&unscaled.border_inner_vertices);
        let scaled_ring_width =
            max_x(&scaled.border_vertices) - max_x(&scaled.border_inner_vertices);

        assert!((scaled_width - unscaled_width * 2.0).abs() < 0.05);
        assert!((scaled_ring_width - unscaled_ring_width * 2.0).abs() < 0.05);
    }

    #[test]
    fn construction_grid_start_light_matches_light_manager_defaults() {
        let polygon = construction_grid_start_light(&ConstructionGrid {
            rows: Vec::new(),
            base_x: 1.0,
            base_y: 2.0,
            grid_width: 6,
            grid_height: 4,
            x_min: 0,
            cell_style: ConstructionGridCellStyle::Default,
        });

        assert!((polygon.vertices[0].0 - 5.5).abs() < 1e-6);
        assert!((polygon.vertices[0].1 - 3.5).abs() < 1e-6);
        assert!((polygon.border_vertices[0].0 - 6.0).abs() < 1e-6);
        assert!((polygon.border_vertices[0].1 - 3.5).abs() < 1e-6);
    }

    fn prefab(name: &str, position: Vec3, raw_text: &str) -> PrefabInstance {
        prefab_with_scale(
            name,
            position,
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            raw_text,
        )
    }

    fn prefab_with_scale(
        name: &str,
        position: Vec3,
        scale: Vec3,
        raw_text: &str,
    ) -> PrefabInstance {
        PrefabInstance {
            name: name.to_string(),
            position,
            prefab_index: 0,
            rotation: Vec3::default(),
            scale,
            data_type: DataType::None,
            terrain_data: None,
            override_data: Some(PrefabOverrideData {
                raw_text: raw_text.to_string(),
                raw_bytes: raw_text.as_bytes().to_vec(),
            }),
            parent: None,
        }
    }
}
