//! Parsing of LitArea / PointLight prefabs and ConstructionGrid into lit-area polygons.

use crate::data::assets;
use crate::domain::prefab_asset::PrefabAssetDocument;
use crate::domain::types::*;
use crate::unity_runtime::components::{BezierCurve, BezierMesh, LevelManager, PointLightSource};
use crate::unity_runtime::scene::Scene;

use super::super::grid::ConstructionGrid;
use super::{LIT_AREA_BORDER_ALPHA, LitAreaPolygon, POINT_LIGHT_BORDER_ALPHA};

const CONSTRUCTION_GRID_START_LIGHT_BORDER_WIDTH: f32 = 0.5;

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
    let border_width = parse_border_width(prefab).unwrap_or_else(lit_area_prefab_border_width);
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

/// Parse borderWidth from a LitArea prefab's override data.
fn parse_border_width(prefab: &PrefabInstance) -> Option<f32> {
    let od = prefab.override_data.as_ref()?;
    let (scene, root) = Scene::from_override_text(&od.raw_text)?;
    let (_, mesh) = scene.get_component_of::<BezierMesh>(root)?;
    mesh.border_width
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
        construction_grid_start_light, parse_dark_level_data, parse_lit_area_bezier,
        parse_point_light,
    };
    use crate::domain::parser::parse_level;
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };
    use crate::renderer::grid::{ConstructionGrid, ConstructionGridCellStyle};
    use crate::unity_runtime::components::LevelManager;
    use crate::unity_runtime::scene::Scene;
    use std::path::Path;

    const LEVEL_MANAGER_OVERRIDE: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tBoolean m_darkLevel = True\n";

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
                LevelObject::Prefab(prefab("LevelManager", Vec3::default(), LEVEL_MANAGER_OVERRIDE)),
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
        assert_eq!(lit_areas[0].vertices.len(), lit_areas[0].border_vertices.len());
    }

    #[test]
    fn dark_sandbox_bytes_parse_as_dark_level() {
        let level_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../test_levels/assetbundles/episode_sandbox_levels_2.unity3d/Episode_6_Dark Sandbox_data.bytes",
        );
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
        let polygon = parse_lit_area_bezier(&prefab(
            "LitArea",
            Vec3::default(),
            LIT_AREA_OVERRIDE,
        ))
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

        assert_eq!(polygon.vertices, polygon.border_vertices);
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
        assert_eq!(polygon.vertices, polygon.border_vertices);
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
        PrefabInstance {
            name: name.to_string(),
            position,
            prefab_index: 0,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
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
