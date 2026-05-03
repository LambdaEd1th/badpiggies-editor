//! Tests for terrain regeneration.

#![cfg(test)]

use super::*;
use super::fill_mesh::ear_clip_triangulate;
use super::math::{count_self_intersections, signed_area};

    #[test]
    fn stripe_vertex_basic() {
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 0.0 },
                texture: 0,
            },
        ];
        let mesh = rebuild_curve_mesh(&nodes, &[0.5]);
        // Should have 6 vertices (3 pairs)
        assert_eq!(mesh.vertices.len(), 6);
        // Inner vertices should be offset in Y by strip_width (0.5)
        // Perpendicular of (1,0) is (0,-1), so inner = pos + (0,-0.5)
        assert!((mesh.vertices[1].y - (-0.5)).abs() < 0.01);
        assert!((mesh.vertices[3].y - (-0.5)).abs() < 0.01);
    }

    #[test]
    fn ear_clip_triangle() {
        let polygon = vec![
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 1.0, y: 0.0 },
            Vec2 { x: 0.5, y: 1.0 },
        ];
        let indices = ear_clip_triangulate(&polygon);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn ear_clip_square() {
        let polygon = vec![
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 1.0, y: 0.0 },
            Vec2 { x: 1.0, y: 1.0 },
            Vec2 { x: 0.0, y: 1.0 },
        ];
        let indices = ear_clip_triangulate(&polygon);
        assert_eq!(indices.len(), 6); // 2 triangles
    }

    #[test]
    fn control_png_roundtrip() {
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.0, y: 0.0 },
                texture: 1,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 0.0 },
                texture: 0,
            },
        ];
        let png_result = encode_control_png(&nodes);
        assert!(png_result.is_ok(), "control PNG should encode: {png_result:?}");
        let Ok(png) = png_result else {
            return;
        };
        let pixels_result = decode_control_png_pixels(&png);
        assert!(pixels_result.is_some(), "control PNG should decode");
        let Some(pixels) = pixels_result else {
            return;
        };
        // Node 0: R=255, G=0 → texture 0
        assert_eq!(pixels[0], 255);
        assert_eq!(pixels[1], 0);
        // Node 1: R=0, G=255 → texture 1
        assert_eq!(pixels[4], 0);
        assert_eq!(pixels[5], 255);
        // Node 2: R=255, G=0 → texture 0
        assert_eq!(pixels[8], 255);
        assert_eq!(pixels[9], 0);
    }

    /// Test earcut with the real 109-vertex closed polygon from the user's terrain.
    /// Compares total triangulated area vs polygon signed area.
    #[test]
    fn ear_clip_closed_terrain_real() {
        let polygon = vec![
            Vec2 {
                x: 3.2207842,
                y: -6.590766,
            },
            Vec2 {
                x: 3.0066977,
                y: -6.5920105,
            },
            Vec2 {
                x: 2.5506659,
                y: -6.5171385,
            },
            Vec2 {
                x: 2.2368956,
                y: -6.5573473,
            },
            Vec2 {
                x: 1.1215711,
                y: -6.5516853,
            },
            Vec2 {
                x: 1.0055976,
                y: -6.469494,
            },
            Vec2 {
                x: 1.138556,
                y: -6.268608,
            },
            Vec2 {
                x: 1.5014915,
                y: -6.001313,
            },
            Vec2 {
                x: 1.6752815,
                y: -5.134963,
            },
            Vec2 {
                x: 2.2660275,
                y: -4.855136,
            },
            Vec2 {
                x: 2.110568,
                y: -4.093384,
            },
            Vec2 {
                x: 2.312665,
                y: -3.5026374,
            },
            Vec2 {
                x: 2.7168608,
                y: -3.2849941,
            },
            Vec2 {
                x: 2.4370327,
                y: -2.942983,
            },
            Vec2 {
                x: 2.3282118,
                y: -2.4144204,
            },
            Vec2 {
                x: 2.5614014,
                y: -2.0257716,
            },
            Vec2 {
                x: 2.9189577,
                y: -1.9014039,
            },
            Vec2 {
                x: 3.2776175,
                y: -1.5149561,
            },
            Vec2 {
                x: 2.9500504,
                y: -0.93755436,
            },
            Vec2 {
                x: 2.5769472,
                y: -0.8287328,
            },
            Vec2 {
                x: 2.0639305,
                y: -0.81318676,
            },
            Vec2 {
                x: 1.8618331,
                y: -0.36235404,
            },
            Vec2 {
                x: 1.9084706,
                y: 0.104024634,
            },
            Vec2 {
                x: 2.2504816,
                y: 1.3943391,
            },
            Vec2 {
                x: 1.5509138,
                y: 1.3166093,
            },
            Vec2 {
                x: 0.82075214,
                y: 1.4809455,
            },
            Vec2 {
                x: 0.804708,
                y: 2.1716368,
            },
            Vec2 {
                x: 1.1622648,
                y: 2.3892803,
            },
            Vec2 {
                x: 0.7269783,
                y: 3.1043944,
            },
            Vec2 {
                x: 0.97571325,
                y: 3.555227,
            },
            Vec2 {
                x: 1.442092,
                y: 3.8350542,
            },
            Vec2 {
                x: 1.8151951,
                y: 5.140915,
            },
            Vec2 {
                x: 2.3437576,
                y: 5.793844,
            },
            Vec2 {
                x: 2.358778,
                y: 6.5781584,
            },
            Vec2 {
                x: 2.9655952,
                y: 6.773239,
            },
            Vec2 {
                x: 3.1055098,
                y: 7.34844,
            },
            Vec2 {
                x: 3.4008827,
                y: 7.830365,
            },
            Vec2 {
                x: 3.8983536,
                y: 7.9702787,
            },
            Vec2 {
                x: 4.349186,
                y: 7.5039005,
            },
            Vec2 {
                x: 4.7067432,
                y: 7.6282673,
            },
            Vec2 {
                x: 4.629013,
                y: 8.172377,
            },
            Vec2 {
                x: 5.297489,
                y: 8.88749,
            },
            Vec2 {
                x: 5.4915133,
                y: 9.249216,
            },
            Vec2 {
                x: 5.956813,
                y: 9.389813,
            },
            Vec2 {
                x: 6.265995,
                y: 9.188709,
            },
            Vec2 {
                x: 6.3390684,
                y: 8.794214,
            },
            Vec2 {
                x: 6.7121716,
                y: 8.716485,
            },
            Vec2 {
                x: 6.9298153,
                y: 8.421112,
            },
            Vec2 {
                x: 7.0075445,
                y: 7.752635,
            },
            Vec2 {
                x: 7.520561,
                y: 7.4261703,
            },
            Vec2 {
                x: 7.6760206,
                y: 6.664418,
            },
            Vec2 {
                x: 7.9631186,
                y: 6.330127,
            },
            Vec2 {
                x: 7.7848425,
                y: 5.8715744,
            },
            Vec2 {
                x: 8.1734915,
                y: 5.638386,
            },
            Vec2 {
                x: 8.515503,
                y: 6.0270348,
            },
            Vec2 {
                x: 9.0440645,
                y: 5.824937,
            },
            Vec2 {
                x: 9.0440645,
                y: 5.2030983,
            },
            Vec2 {
                x: 9.354984,
                y: 4.767812,
            },
            Vec2 {
                x: 8.935244,
                y: 4.441347,
            },
            Vec2 {
                x: 9.774725,
                y: 4.363617,
            },
            Vec2 {
                x: 10.318833,
                y: 4.2081575,
            },
            Vec2 {
                x: 10.52093,
                y: 3.6795948,
            },
            Vec2 {
                x: 10.809194,
                y: 3.3715112,
            },
            Vec2 {
                x: 10.413821,
                y: 3.038582,
            },
            Vec2 {
                x: 10.185775,
                y: 3.0582244,
            },
            Vec2 {
                x: 9.9146385,
                y: 2.813534,
            },
            Vec2 {
                x: 9.629618,
                y: 2.8633583,
            },
            Vec2 {
                x: 9.252022,
                y: 2.701201,
            },
            Vec2 {
                x: 9.334412,
                y: 2.501111,
            },
            Vec2 {
                x: 9.308347,
                y: 2.0317233,
            },
            Vec2 {
                x: 8.863612,
                y: 1.6105142,
            },
            Vec2 {
                x: 8.484608,
                y: 1.5097895,
            },
            Vec2 {
                x: 8.63987,
                y: 1.2855173,
            },
            Vec2 {
                x: 9.05685,
                y: 1.2356344,
            },
            Vec2 {
                x: 9.293237,
                y: 0.8488258,
            },
            Vec2 {
                x: 9.277254,
                y: 0.5859493,
            },
            Vec2 {
                x: 9.453387,
                y: 0.06870089,
            },
            Vec2 {
                x: 9.35459,
                y: -0.14535916,
            },
            Vec2 {
                x: 9.402237,
                y: -0.23703015,
            },
            Vec2 {
                x: 8.727954,
                y: -0.49190336,
            },
            Vec2 {
                x: 8.2667675,
                y: -0.5333596,
            },
            Vec2 {
                x: 7.910351,
                y: -0.6764072,
            },
            Vec2 {
                x: 8.235675,
                y: -1.5127549,
            },
            Vec2 {
                x: 8.889873,
                y: -1.6298243,
            },
            Vec2 {
                x: 8.889465,
                y: -1.928469,
            },
            Vec2 {
                x: 9.331049,
                y: -2.4003015,
            },
            Vec2 {
                x: 9.328866,
                y: -3.1641505,
            },
            Vec2 {
                x: 9.150342,
                y: -3.4498353,
            },
            Vec2 {
                x: 9.068795,
                y: -3.8293705,
            },
            Vec2 {
                x: 9.228912,
                y: -4.0434694,
            },
            Vec2 {
                x: 9.230617,
                y: -4.4975786,
            },
            Vec2 {
                x: 9.095525,
                y: -4.7898064,
            },
            Vec2 {
                x: 8.981684,
                y: -4.88396,
            },
            Vec2 {
                x: 9.0466585,
                y: -5.2832966,
            },
            Vec2 {
                x: 8.875276,
                y: -5.4213963,
            },
            Vec2 {
                x: 8.526504,
                y: -5.550169,
            },
            Vec2 {
                x: 8.295506,
                y: -5.7614822,
            },
            Vec2 {
                x: 8.162485,
                y: -5.772996,
            },
            Vec2 {
                x: 8.049504,
                y: -6.1038923,
            },
            Vec2 {
                x: 8.191641,
                y: -6.284794,
            },
            Vec2 {
                x: 8.1619425,
                y: -6.460931,
            },
            Vec2 {
                x: 7.8492203,
                y: -6.575528,
            },
            Vec2 {
                x: 7.2548304,
                y: -6.4656954,
            },
            Vec2 {
                x: 6.9899387,
                y: -6.5561457,
            },
            Vec2 {
                x: 6.595832,
                y: -6.569067,
            },
            Vec2 {
                x: 5.72363,
                y: -6.4333916,
            },
            Vec2 {
                x: 4.760975,
                y: -6.407548,
            },
            Vec2 {
                x: 4.483162,
                y: -6.5432243,
            },
            Vec2 {
                x: 3.9856825,
                y: -6.485078,
            },
        ];

        let n = polygon.len();
        assert_eq!(n, 109);

        let indices = ear_clip_triangulate(&polygon);
        let tri_count = indices.len() / 3;
        assert_eq!(
            tri_count,
            n - 2,
            "expected {} triangles, got {}",
            n - 2,
            tri_count
        );

        // Compute total triangulated area (sum of absolute triangle areas)
        let mut tri_area_sum = 0.0_f64;
        for t in 0..tri_count {
            let a = polygon[indices[t * 3] as usize];
            let b = polygon[indices[t * 3 + 1] as usize];
            let c = polygon[indices[t * 3 + 2] as usize];
            let area = ((b.x - a.x) as f64 * (c.y - a.y) as f64
                - (c.x - a.x) as f64 * (b.y - a.y) as f64)
                .abs()
                * 0.5;
            tri_area_sum += area;
        }

        // Compute polygon area via shoelace
        let poly_area = signed_area(&polygon).abs();

        let diff = (tri_area_sum - poly_area).abs();
        let rel = diff / poly_area;
        assert!(
            rel < 0.01,
            "triangulated area {tri_area_sum:.4} vs polygon area {poly_area:.4}, relative error {rel:.6}"
        );
    }

    /// Test that an open U-shaped curve survives a close→open roundtrip.
    #[test]
    fn open_close_open_roundtrip() {
        // U-shaped open curve: goes up, right, down — mimics the real terrain
        // that starts at (3.7, -18.1) and loops around
        let original_nodes = vec![
            CurveNode {
                position: Vec2 {
                    x: 3.7255,
                    y: -18.1278,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.6659,
                    y: -10.8056,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.7508,
                    y: -9.9418,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.2167,
                    y: -9.8004,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 {
                    x: 0.2559,
                    y: -9.4390,
                },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.0, y: -8.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.5, y: -6.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.5, y: -5.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.5, y: -6.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.0, y: -8.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.2, y: -10.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 3.5, y: -15.0 },
                texture: 0,
            },
        ];
        let strip_widths = [0.5, 0.1];
        let mesh0 = rebuild_curve_mesh(&original_nodes, &strip_widths);
        // Dump for debugging
        for i in 0..original_nodes.len() {
            let outer = mesh0.vertices[i * 2];
            let inner = mesh0.vertices[i * 2 + 1];
            eprintln!(
                "original[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // Close: add duplicate of first node
        let mut closed_nodes = original_nodes.clone();
        closed_nodes.push(CurveNode {
            position: original_nodes[0].position,
            texture: original_nodes[0].texture,
        });
        let mesh_closed = rebuild_curve_mesh(&closed_nodes, &strip_widths);
        eprintln!("--- closed ---");
        for i in 0..closed_nodes.len() {
            let outer = mesh_closed.vertices[i * 2];
            let inner = mesh_closed.vertices[i * 2 + 1];
            eprintln!(
                "closed[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // Open: remove last node
        let mut reopened_nodes = closed_nodes;
        reopened_nodes.pop();
        let mesh1 = rebuild_curve_mesh(&reopened_nodes, &strip_widths);
        eprintln!("--- reopened ---");
        for i in 0..reopened_nodes.len() {
            let outer = mesh1.vertices[i * 2];
            let inner = mesh1.vertices[i * 2 + 1];
            eprintln!(
                "reopened[{}] outer=({:.4},{:.4}) inner=({:.4},{:.4}) d=({:.4},{:.4})",
                i,
                outer.x,
                outer.y,
                inner.x,
                inner.y,
                inner.x - outer.x,
                inner.y - outer.y
            );
        }

        // All vertices should match the original
        assert_eq!(mesh0.vertices.len(), mesh1.vertices.len());
        for i in 0..mesh0.vertices.len() {
            let v0 = mesh0.vertices[i];
            let v1 = mesh1.vertices[i];
            assert!(
                (v0.x - v1.x).abs() < 0.001 && (v0.y - v1.y).abs() < 0.001,
                "vertex {} mismatch: ({:.4},{:.4}) vs ({:.4},{:.4})",
                i,
                v0.x,
                v0.y,
                v1.x,
                v1.y
            );
        }
    }

    /// Test that inner edges are collapsed when strip_w exceeds inter-node spacing.
    #[test]
    fn inner_collapse_dense_nodes() {
        // Nodes spaced 0.3 apart with a sharp turn — strip_w=0.5 exceeds spacing
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.3, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.6, y: 0.3 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.9, y: 0.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 1.2, y: 0.0 },
                texture: 0,
            },
        ];
        let mesh = rebuild_curve_mesh(&nodes, &[0.5]);

        // Verify no NaN/Inf in vertices
        for (i, v) in mesh.vertices.iter().enumerate() {
            assert!(v.x.is_finite(), "vertex {} x is not finite: {:?}", i, v);
            assert!(v.y.is_finite(), "vertex {} y is not finite: {:?}", i, v);
        }

        // Verify the mesh is valid
        assert_eq!(mesh.vertices.len(), 10); // 5 nodes * 2
        assert!(mesh.indices.len() >= 6, "should have at least 1 quad");
    }

    /// Test that an open terrain fill mesh is NOT just a rectangle.
    #[test]
    fn open_fill_mesh_not_rectangle() {
        // Simple open terrain: 5 nodes along a wave, boundary extending below
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 0.0, y: 2.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.0, y: 3.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 4.0, y: 1.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 6.0, y: 2.5 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 8.0, y: 2.0 },
                texture: 0,
            },
        ];
        // Boundary extends well below the curve
        let boundary = [-1.0, -3.0, 9.0, 4.0];
        let fill = rebuild_fill_mesh(&nodes, boundary);

        assert!(
            fill.vertices.len() > 4,
            "should have more than 4 verts (not just corners)"
        );
        assert!(fill.indices.len() >= 9, "should have at least 3 triangles");

        // Verify all vertices are finite
        for (i, v) in fill.vertices.iter().enumerate() {
            assert!(
                v.x.is_finite() && v.y.is_finite(),
                "fill vertex {} is not finite: ({}, {})",
                i,
                v.x,
                v.y
            );
        }

        // Verify all indices are in bounds
        for (i, &idx) in fill.indices.iter().enumerate() {
            assert!(
                (idx as usize) < fill.vertices.len(),
                "fill index {} = {} out of bounds (verts={})",
                i,
                idx,
                fill.vertices.len()
            );
        }

        // The polygon should include curve nodes (not just boundary corners)
        let has_node_at_y3 = fill.vertices.iter().any(|v| (v.y - 3.0).abs() < 0.01);
        assert!(has_node_at_y3, "fill should include curve node at y=3.0");

        // Compute triangulated area vs polygon area — they should match
        let tri_count = fill.indices.len() / 3;
        let mut tri_area = 0.0_f64;
        for t in 0..tri_count {
            let a = fill.vertices[fill.indices[t * 3] as usize];
            let b = fill.vertices[fill.indices[t * 3 + 1] as usize];
            let c = fill.vertices[fill.indices[t * 3 + 2] as usize];
            let area = ((b.x - a.x) as f64 * (c.y - a.y) as f64
                - (c.x - a.x) as f64 * (b.y - a.y) as f64)
                .abs()
                * 0.5;
            tri_area += area;
        }
        let poly_area = signed_area(&fill.vertices).abs();
        let rel_err = (tri_area - poly_area).abs() / poly_area;
        assert!(
            rel_err < 0.01,
            "fill triangulation area error: tri={:.4} poly={:.4} rel={:.6}",
            tri_area,
            poly_area,
            rel_err
        );
    }

    #[test]
    fn same_edge_fill_not_self_intersecting() {
        // Both start and end nodes project to the SAME boundary edge (bottom).
        // Previously, this caused the corner walk to add ALL 4 corners, creating
        // a self-intersecting polygon.
        let nodes = vec![
            CurveNode {
                position: Vec2 { x: 3.7, y: -18.1 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.7, y: -10.8 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 0.2, y: -9.4 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 2.1, y: -6.1 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 5.0, y: -3.0 },
                texture: 0,
            },
            CurveNode {
                position: Vec2 { x: 9.0, y: -18.1 },
                texture: 0,
            },
        ];
        let boundary = [-0.76, -18.63, 9.82, 6.63];
        let fill = rebuild_fill_mesh(&nodes, boundary);

        // The polygon should NOT have all 4 boundary corners
        let boundary_corners = [
            Vec2 { x: 9.82, y: -18.63 },
            Vec2 {
                x: -0.76,
                y: -18.63,
            },
            Vec2 { x: -0.76, y: 6.63 },
            Vec2 { x: 9.82, y: 6.63 },
        ];
        let corner_count = boundary_corners
            .iter()
            .filter(|c| {
                fill.vertices
                    .iter()
                    .any(|v| (v.x - c.x).abs() < 0.01 && (v.y - c.y).abs() < 0.01)
            })
            .count();
        assert!(
            corner_count < 4,
            "should NOT include all 4 boundary corners when both endpoints project to same edge, got {} corners",
            corner_count
        );

        // The polygon should not self-intersect
        let si = count_self_intersections(&fill.vertices);
        assert_eq!(si, 0, "fill polygon should not self-intersect");

        // Should still have valid triangulation
        assert!(fill.indices.len() >= 9, "should have valid triangulation");
        for &idx in &fill.indices {
            assert!(
                (idx as usize) < fill.vertices.len(),
                "index {} out of bounds (verts={})",
                idx,
                fill.vertices.len()
            );
        }
    }
