#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    const UNITY_SHADER_PORTS: &[(&str, &str)] = &[
        ("Depth MaskMaskOverlay.shader", "depth_mask__maskoverlay.wgsl"),
        ("Depth MaskMaskOverlayNV.shader", "depth_mask__maskoverlaynv.wgsl"),
        ("Depth MaskUnlit Transparent (CG).shader", "depth_mask__unlit_transparent_cg.wgsl"),
        ("Depth MaskUnlit Transparent ZAlways.shader", "depth_mask__unlit_transparent_zalways.wgsl"),
        ("Photoshot Effect.shader", "photoshot_effect.wgsl"),
        ("SpineSkeleton.shader", "spine__skeleton.wgsl"),
        ("_CustomDailyChallengeMask.shader", "_custom__dailychallengemask.wgsl"),
        (
            "_CustomPreAlpha_Unlit_ColorTransparent_Geometry.shader",
            "_custom__prealpha_unlit_colortransparent_geometry.wgsl",
        ),
        (
            "_CustomPreAlpha_Unlit_ColorTransparent_GeometryZ.shader",
            "_custom__prealpha_unlit_colortransparent_geometryz.wgsl",
        ),
        (
            "_CustomPreAlpha_Unlit_ColorTransparent_Geometry_Gray.shader",
            "_custom__prealpha_unlit_colortransparent_geometry_gray.wgsl",
        ),
        (
            "_CustomPreAlpha_Unlit_ColorTransparent_Geometry_Shiny.shader",
            "_custom__prealpha_unlit_colortransparent_geometry_shiny.wgsl",
        ),
        ("_CustomSilhouetteShader.shader", "_custom__silhouetteshader.wgsl"),
        ("_CustomText Shader With Z Test.shader", "_custom__text_shader_with_z_test.wgsl"),
        ("_CustomUnlit_Alpha8Bit_Color.shader", "_custom__unlit_alpha8bit_color.wgsl"),
        (
            "_CustomUnlit_ColorTransparent_Geometry.shader",
            "_custom__unlit_colortransparent_geometry.wgsl",
        ),
        ("_CustomUnlit_Color_Geometry.shader", "_custom__unlit_color_geometry.wgsl"),
        ("_CustomUnlit_Monochrome.shader", "_custom__unlit_monochrome.wgsl"),
        ("_CustomUtility_ZWrite.shader", "_custom__utility_zwrite.wgsl"),
    ];

    const RUNTIME_SHADER_ASSETS: &[&str] = &[
        "_custom__unlit_colortransparent_geometry__sprite.wgsl",
        "_custom__unlit_color_geometry__terrain_fill.wgsl",
        "e2d__curve.wgsl",
        "unlit__transparent_cutout__sprite.wgsl",
        "depth_mask__unlit_transparent_cg__runtime.wgsl",
        "depth_mask__maskoverlay__runtime.wgsl",
        "depth_mask__maskoverlaynv__runtime.wgsl",
    ];

    fn shader_source_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../Assets/Shader")
    }

    fn wgsl_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("editor_assets/shader")
    }

    #[test]
    fn every_embedded_unity_shader_has_a_wgsl_port_file() {
        let unity_shader_count = fs::read_dir(shader_source_dir())
            .expect("expected Assets/Shader directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("shader"))
            .count();

        assert_eq!(UNITY_SHADER_PORTS.len(), unity_shader_count);

        for (shader_name, wgsl_name) in UNITY_SHADER_PORTS {
            let shader_path = shader_source_dir().join(shader_name);
            assert!(shader_path.exists(), "missing Unity shader source {}", shader_name);

            let wgsl_path = wgsl_dir().join(wgsl_name);
            assert!(wgsl_path.exists(), "missing WGSL port {} for {}", wgsl_name, shader_name);

            let source = fs::read_to_string(&wgsl_path)
                .unwrap_or_else(|_| panic!("failed to read WGSL port {}", wgsl_name));
            assert!(source.contains("@vertex"), "{} is missing a vertex entry point", wgsl_name);
            assert!(source.contains("@fragment"), "{} is missing a fragment entry point", wgsl_name);
        }
    }

    #[test]
    fn runtime_shader_sources_live_in_editor_assets_shader() {
        for wgsl_name in RUNTIME_SHADER_ASSETS {
            let wgsl_path = wgsl_dir().join(wgsl_name);
            assert!(wgsl_path.exists(), "missing runtime WGSL asset {}", wgsl_name);

            let source = fs::read_to_string(&wgsl_path)
                .unwrap_or_else(|_| panic!("failed to read runtime WGSL asset {}", wgsl_name));
            assert!(source.contains("@vertex"), "{} is missing a vertex entry point", wgsl_name);
            assert!(source.contains("@fragment"), "{} is missing a fragment entry point", wgsl_name);
        }
    }
}