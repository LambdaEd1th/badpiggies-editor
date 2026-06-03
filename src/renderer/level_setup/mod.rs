//! Level setup: loading level data into the renderer, building draw data and GPU resources.

mod set_level;

use std::sync::Arc;

use crate::data::assets;
use crate::domain::types::{LevelData, LevelObject, ObjectIndex, Vec2, Vec3};
use crate::unity_runtime::components::{
    Camera as UnityCamera, LevelManager, Transform as UnityTransform,
};
use crate::unity_runtime::scene::Scene;

use super::bg_shader;
use super::dark_mask_shader;
use super::edge_shader;
use super::fill_shader;
use super::opaque_shader;
use super::sprite_shader;
use super::{Camera, LevelRenderer, PreviewPlaybackState, TerrainDrawMode};

impl LevelRenderer {
    pub fn new(render_state: Option<&egui_wgpu::RenderState>) -> Self {
        // Initialize wgpu edge shader pipeline if render state available
        let (wgpu_device, wgpu_queue, edge_resources) = match render_state {
            Some(rs) => {
                log::info!(
                    "wgpu render state available, target_format={:?}",
                    rs.target_format
                );
                let resources = edge_shader::init_edge_resources(&rs.device, rs.target_format);
                (
                    Some(rs.device.clone()),
                    Some(rs.queue.clone()),
                    Some(Arc::new(resources)),
                )
            }
            None => {
                log::warn!("No wgpu render state — edge shader disabled");
                (None, None, None)
            }
        };
        let opaque_resources = render_state.map(|rs| {
            Arc::new(opaque_shader::init_opaque_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        let bg_resources = render_state
            .map(|rs| Arc::new(bg_shader::init_bg_resources(&rs.device, rs.target_format)));
        let sprite_resources = render_state.map(|rs| {
            Arc::new(sprite_shader::init_sprite_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        let fill_resources = render_state.map(|rs| {
            Arc::new(fill_shader::init_fill_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        let dark_mask_resources = render_state.map(|rs| {
            Arc::new(dark_mask_shader::init_dark_mask_resources(
                &rs.device,
                rs.target_format,
            ))
        });
        Self {
            camera: Camera::default(),
            world_positions: Vec::new(),
            terrain_data: Vec::new(),
            sprite_data: Vec::new(),
            bg_theme: None,
            bg_override_text: None,
            bg_layer_cache: None,
            construction_grid: None,
            show_grid_overlay: true,
            level_key: String::new(),
            tex_cache: assets::TextureCache::new(),
            panning: false,
            clicked_object: None,
            clicked_with_cmd: false,
            mouse_world: None,
            time: 0.0,
            preview_playback_state: PreviewPlaybackState::Play,
            dragging: None,
            node_dragging: None,
            drag_result: None,
            rotation_drag_result: None,
            scale_drag_result: None,
            node_drag_result: None,
            node_edit_action: None,
            box_select_result: None,
            context_action: None,
            context_selected_object: None,
            context_menu_world_pos: None,
            context_menu_indices: Vec::new(),
            context_menu_node: None,
            suppress_context_menu_this_frame: false,
            box_select_start: None,
            draw_terrain_result: None,
            terrain_preset_shape: None,
            terrain_draw_has_collider: true,
            terrain_preset_drag_start: None,
            terrain_curve_segments: 24,
            terrain_draw_mode: TerrainDrawMode::default(),
            terrain_draw_texture_index: 1,
            draw_terrain_points: Vec::new(),
            draw_terrain_active: false,
            terrain_draw_continuation_anchor: None,
            bounds_dragging: None,
            bounds_drag_result: None,
            bounds_hovered_handle: None,
            pending_drag_offset: None,
            pending_transform_preview: None,
            show_bg: true,
            show_ground: false,
            show_grid: true,
            dark_level: false,
            show_dark_overlay: true,
            contraption_has_night_vision: false,
            night_vision_enabled: false,
            camera_limits: None,
            show_level_bounds: false,
            show_terrain_tris: false,
            lit_area_polygons: Vec::new(),
            fan_emitters: Vec::new(),
            fan_particles: Vec::new(),
            wind_areas: Vec::new(),
            wind_particles: Vec::new(),
            wind_spawn_accum: Vec::new(),
            zzz_particles: Vec::new(),
            zzz_emit_accum: Vec::new(),
            bird_positions: Vec::new(),
            attached_effect_emitters: Vec::new(),
            attached_effect_particles: Vec::new(),
            cloud_instances: Vec::new(),
            wgpu_device,
            wgpu_queue,
            edge_resources,
            edge_gpu_meshes: Arc::new(Vec::new()),
            edge_gpu_mesh_index: Vec::new(),
            bg_resources,
            bg_atlas_cache: bg_shader::BgAtlasCache::new(),
            bg_slot_counter: 0,
            opaque_resources,
            opaque_atlas: None,
            opaque_batch: None,
            opaque_sprite_map: Vec::new(),
            sprite_resources,
            sprite_atlas_cache: sprite_shader::SpriteAtlasCache::new(),
            sprite_slot_counter: 0,
            fill_resources,
            dark_mask_resources,
            fill_texture_cache: fill_shader::FillTextureCache::new(),
            fill_gpu_meshes: Vec::new(),
            fill_slot_counter: 0,
            dark_mask_slot_counter: 0,
            hovered_terrain_node: None,
            hovered_rotation_handle: None,
            hovered_scale_handle: None,
            clicked_empty: false,
            dark_overlay_mesh: None,
            dark_overlay_mesh_gpu: None,
            dark_overlay_light: None,
            dark_overlay_light_gpu: None,
            dark_overlay_ring: None,
            dark_overlay_ring_gpu: None,
            dark_overlay_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_live_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_stable_frames: 0,
        }
    }

    /// Create a new renderer that shares GPU pipeline resources with this one.
    /// Used when opening a new tab — pipeline/device/queue are Arc-shared,
    /// while per-level caches start empty.
    pub fn clone_for_new_tab(&self) -> Self {
        Self {
            camera: Camera::default(),
            world_positions: Vec::new(),
            terrain_data: Vec::new(),
            sprite_data: Vec::new(),
            bg_theme: None,
            bg_override_text: None,
            bg_layer_cache: None,
            construction_grid: None,
            show_grid_overlay: self.show_grid_overlay,
            level_key: String::new(),
            tex_cache: assets::TextureCache::new(),
            panning: false,
            clicked_object: None,
            clicked_with_cmd: false,
            mouse_world: None,
            time: 0.0,
            preview_playback_state: PreviewPlaybackState::Play,
            dragging: None,
            node_dragging: None,
            drag_result: None,
            rotation_drag_result: None,
            scale_drag_result: None,
            node_drag_result: None,
            node_edit_action: None,
            box_select_result: None,
            context_action: None,
            context_selected_object: None,
            context_menu_world_pos: None,
            context_menu_indices: Vec::new(),
            context_menu_node: None,
            suppress_context_menu_this_frame: false,
            box_select_start: None,
            draw_terrain_result: None,
            terrain_preset_shape: None,
            terrain_draw_has_collider: self.terrain_draw_has_collider,
            terrain_preset_drag_start: None,
            terrain_curve_segments: self.terrain_curve_segments,
            terrain_draw_mode: self.terrain_draw_mode,
            terrain_draw_texture_index: self.terrain_draw_texture_index,
            draw_terrain_points: Vec::new(),
            draw_terrain_active: false,
            terrain_draw_continuation_anchor: self.terrain_draw_continuation_anchor,
            bounds_dragging: None,
            bounds_drag_result: None,
            bounds_hovered_handle: None,
            pending_drag_offset: None,
            pending_transform_preview: None,
            show_bg: self.show_bg,
            show_ground: self.show_ground,
            show_grid: self.show_grid,
            dark_level: false,
            show_dark_overlay: true,
            contraption_has_night_vision: false,
            night_vision_enabled: false,
            camera_limits: None,
            show_level_bounds: self.show_level_bounds,
            show_terrain_tris: self.show_terrain_tris,
            lit_area_polygons: Vec::new(),
            fan_emitters: Vec::new(),
            fan_particles: Vec::new(),
            wind_areas: Vec::new(),
            wind_particles: Vec::new(),
            wind_spawn_accum: Vec::new(),
            zzz_particles: Vec::new(),
            zzz_emit_accum: Vec::new(),
            bird_positions: Vec::new(),
            attached_effect_emitters: Vec::new(),
            attached_effect_particles: Vec::new(),
            cloud_instances: Vec::new(),
            wgpu_device: self.wgpu_device.clone(),
            wgpu_queue: self.wgpu_queue.clone(),
            edge_resources: self.edge_resources.clone(),
            edge_gpu_meshes: Arc::new(Vec::new()),
            edge_gpu_mesh_index: Vec::new(),
            bg_resources: self.bg_resources.clone(),
            bg_atlas_cache: bg_shader::BgAtlasCache::new(),
            bg_slot_counter: 0,
            opaque_resources: self.opaque_resources.clone(),
            opaque_atlas: None,
            opaque_batch: None,
            opaque_sprite_map: Vec::new(),
            sprite_resources: self.sprite_resources.clone(),
            sprite_atlas_cache: sprite_shader::SpriteAtlasCache::new(),
            sprite_slot_counter: 0,
            fill_resources: self.fill_resources.clone(),
            dark_mask_resources: self.dark_mask_resources.clone(),
            fill_texture_cache: fill_shader::FillTextureCache::new(),
            fill_gpu_meshes: Vec::new(),
            fill_slot_counter: 0,
            dark_mask_slot_counter: 0,
            hovered_terrain_node: None,
            hovered_rotation_handle: None,
            hovered_scale_handle: None,
            clicked_empty: false,
            dark_overlay_mesh: None,
            dark_overlay_mesh_gpu: None,
            dark_overlay_light: None,
            dark_overlay_light_gpu: None,
            dark_overlay_ring: None,
            dark_overlay_ring_gpu: None,
            dark_overlay_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_live_key: (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            dark_overlay_stable_frames: 0,
        }
    }

    /// Rebuild cached data when a new level is loaded.
    pub fn fit_to_level(&mut self) {
        if let Some([tl_x, tl_y, w, h]) = self.camera_limits {
            let padding = 5.0;
            self.camera.center = Vec2 {
                x: tl_x + w * 0.5,
                y: tl_y - h * 0.5,
            };

            let range_x = w + padding * 2.0;
            let range_y = h + padding * 2.0;
            let range = range_x.max(range_y).max(1.0);
            self.camera.zoom = (600.0 / range).clamp(5.0, 200.0);
            return;
        }

        if self.world_positions.is_empty() {
            return;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for &(_, pos) in &self.world_positions {
            min_x = min_x.min(pos.x);
            max_x = max_x.max(pos.x);
            min_y = min_y.min(pos.y);
            max_y = max_y.max(pos.y);
        }

        let padding = 5.0;
        self.camera.center = Vec2 {
            x: (min_x + max_x) / 2.0,
            y: (min_y + max_y) / 2.0,
        };

        let range_x = (max_x - min_x) + padding * 2.0;
        let range_y = (max_y - min_y) + padding * 2.0;
        let range = range_x.max(range_y).max(1.0);
        self.camera.zoom = (600.0 / range).clamp(5.0, 200.0);
    }
}

pub(super) fn load_raw_rgba(asset_key: &str) -> Option<(Vec<u8>, u32, u32)> {
    let data = crate::data::assets::read_pathname(asset_key)?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    // Flip vertically: image crate stores top-to-bottom, but glTexImage2D places
    // row 0 at V=0 (bottom). Flipping matches Three.js flipY=true / Unity convention
    // so that V=1 (outer/surface) maps to the top of the image (green grass).
    let flipped = image::imageops::flip_vertical(&img);
    let w = flipped.width();
    let h = flipped.height();
    Some((flipped.into_raw(), w, h))
}

/// Compute the world position of an object by walking up the parent chain.
/// Binary stores world-space positions (LevelLoader.cs uses transform.position, not localPosition).
pub(super) fn compute_world_position(level: &LevelData, idx: ObjectIndex) -> Vec3 {
    level.objects[idx].position()
}

/// Search the flat object arena for a BackgroundObject with override data.
///
/// Accepts both `Component UnityEngine.Transform` overrides (EP1-5 style)
/// and `PositionSerializer` / `childLocalPositions` (EP6 style).
pub(super) fn find_bg_override_text(objects: &[LevelObject]) -> Option<String> {
    for obj in objects {
        if let LevelObject::Prefab(inst) = obj
            && inst.name.contains("Background")
            && let Some(ref od) = inst.override_data
            && background_override_has_transform_or_serializer(&od.raw_text)
        {
            return Some(od.raw_text.clone());
        }
    }
    None
}

pub(super) fn find_bg_root_position(objects: &[LevelObject]) -> Option<Vec3> {
    for obj in objects {
        if let LevelObject::Prefab(inst) = obj
            && inst.name.contains("Background")
            && let Some(ref od) = inst.override_data
            && background_override_has_transform_or_serializer(&od.raw_text)
        {
            return Some(inst.position);
        }
    }
    None
}

pub(super) fn parse_authored_camera(level: &LevelData) -> Option<(Vec2, f32)> {
    for obj in &level.objects {
        if let LevelObject::Prefab(prefab) = obj
            && prefab.name == "CameraSystem"
            && let Some(ref od) = prefab.override_data
            && let Some((scene, root)) = Scene::from_override_text(&od.raw_text)
        {
            let camera_owner = scene.find_child(root, "GameCamera").unwrap_or(root);
            let Some((_, camera)) = scene.get_component_of::<UnityCamera>(camera_owner) else {
                continue;
            };
            if camera.orthographic_size <= 0.0 {
                continue;
            }

            let mut center = Vec2 {
                x: prefab.position.x,
                y: prefab.position.y,
            };
            if camera_owner != root
                && let Some((_, transform)) = scene.get_component_of::<UnityTransform>(camera_owner)
                && let Some(local) = transform.local_position
            {
                center.x += local.x;
                center.y += local.y;
            }

            return Some((center, 600.0 / (camera.orthographic_size * 2.0)));
        }
    }
    None
}

/// Parse `m_cameraLimits` from LevelManager override data.
/// Returns `[topLeft.x, topLeft.y, size.x, size.y]` or `None` if not overridden.
pub(super) fn parse_camera_limits(level: &LevelData) -> Option<[f32; 4]> {
    for obj in &level.objects {
        if let LevelObject::Prefab(p) = obj
            && p.name == "LevelManager"
            && let Some(ref od) = p.override_data
            && let Some((scene, root)) = Scene::from_override_text(&od.raw_text)
            && let Some((_, lm)) = scene.get_component_of::<LevelManager>(root)
            && let Some(vals) = lm.camera_limits
            && vals[2] > 0.0
            && vals[3] > 0.0
        {
            return Some(vals);
        }
    }
    None
}

fn background_override_has_transform_or_serializer(raw_text: &str) -> bool {
    let Some((scene, _root)) = Scene::from_override_text(raw_text) else {
        return false;
    };
    scene.iter_components().any(|(_, c)| {
        let suffix = c.behavior.component_suffix();
        suffix == "Transform"
            || suffix == "PositionSerializer"
            || c.behavior
                .extra()
                .iter()
                .any(|(name, _)| name == "childLocalPositions")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LevelRenderer, find_bg_override_text, find_bg_root_position, parse_authored_camera,
        parse_camera_limits,
    };
    use crate::domain::parser::parse_level;
    use crate::domain::types::{
        DataType, LevelData, LevelObject, ParentObject, PrefabInstance, PrefabOverrideData, Vec3,
    };

    const LEVEL_MANAGER_OVERRIDE: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tBoolean m_darkLevel = True\n\t\tGeneric m_cameraLimits\n\t\t\tVector2 topLeft\n\t\t\t\tFloat x = -202.43\n\t\t\t\tFloat y = 28.3\n\t\t\tVector2 size\n\t\t\t\tFloat x = 96.5\n\t\t\t\tFloat y = 49.7\n";

    const CAMERA_SYSTEM_OVERRIDE: &str = "GameObject CameraSystem\n\tComponent Camera\n\t\tBoolean orthographic = True\n\t\tFloat orthographic size = 9.430499\n";

    const NESTED_CAMERA_SYSTEM_OVERRIDE: &str = "GameObject CameraSystem\n\tGameObject GameCamera\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = -92.76147\n\t\t\t\tFloat y = 51.59889\n\t\tComponent UnityEngine.Camera\n\t\t\tBoolean orthographic = True\n\t\t\tFloat orthographic size = 9.430499\n";

    const BG_OVERRIDE: &str = "GameObject Background_Cave_01_SET 1\n\tComponent PositionSerializer\n\t\tArray childLocalPositions\n\t\t\tArraySize size = 1\n";

    #[test]
    fn parses_camera_limits_from_level_manager_override_ast() {
        let level = LevelData {
            objects: vec![LevelObject::Prefab(prefab(
                "LevelManager",
                LEVEL_MANAGER_OVERRIDE,
            ))],
            roots: vec![0],
        };

        assert_eq!(
            parse_camera_limits(&level),
            Some([-202.43, 28.3, 96.5, 49.7])
        );
    }

    #[test]
    fn parses_authored_camera_from_camera_system_override_ast() {
        let mut camera = prefab("CameraSystem", CAMERA_SYSTEM_OVERRIDE);
        camera.position = Vec3 {
            x: 12.5,
            y: -3.75,
            z: 0.0,
        };
        let level = LevelData {
            objects: vec![LevelObject::Prefab(camera)],
            roots: vec![0],
        };

        let Some((center, zoom)) = parse_authored_camera(&level) else {
            panic!("expected CameraSystem override to produce an authored camera");
        };
        assert!((center.x - 12.5).abs() < 0.001);
        assert!((center.y + 3.75).abs() < 0.001);
        assert!((zoom - (600.0 / (9.430499 * 2.0))).abs() < 0.001);
    }

    #[test]
    fn parses_authored_camera_from_nested_game_camera_override_ast() {
        let mut camera = prefab("CameraSystem", NESTED_CAMERA_SYSTEM_OVERRIDE);
        camera.position = Vec3 {
            x: 32.8404,
            y: -2.0766,
            z: -15.0,
        };
        let level = LevelData {
            objects: vec![LevelObject::Prefab(camera)],
            roots: vec![0],
        };

        let Some((center, zoom)) = parse_authored_camera(&level) else {
            panic!("expected nested GameCamera override to produce an authored camera");
        };
        assert!((center.x - (32.8404 - 92.76147)).abs() < 0.001);
        assert!((center.y - (-2.0766 + 51.59889)).abs() < 0.001);
        assert!((zoom - (600.0 / (9.430499 * 2.0))).abs() < 0.001);
    }

    #[test]
    fn level27_parses_authored_camera_system_view() {
        let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels/assetbundles/episode_1_levels.unity3d/Level_27_data.bytes");
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        let level = parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

        let Some((_, zoom)) = parse_authored_camera(&level) else {
            panic!("expected Level_27 CameraSystem to provide an authored camera view");
        };
        assert!(
            zoom > 20.0,
            "expected authored Level_27 zoom to be much tighter than bounds-fit, got {zoom}"
        );
    }

    #[test]
    fn sandbox_level_parses_authored_camera_center_from_game_camera_transform() {
        let level_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels/assetbundles/episode_sandbox_levels_2.unity3d/Level_Sandbox_01_data.bytes");
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        let level = parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()));

        let Some((center, zoom)) = parse_authored_camera(&level) else {
            panic!("expected Level_Sandbox_01 CameraSystem to provide an authored camera view");
        };
        assert!(
            (center.x - -59.92106).abs() < 0.01,
            "expected sandbox camera center x to include nested GameCamera transform, got {}",
            center.x
        );
        assert!(
            (center.y - 49.52229).abs() < 0.01,
            "expected sandbox camera center y to include nested GameCamera transform, got {}",
            center.y
        );
        assert!((zoom - (600.0 / (9.430499 * 2.0))).abs() < 0.001);
    }

    #[test]
    fn finds_background_override_via_position_serializer_ast() {
        let level = LevelData {
            objects: vec![
                LevelObject::Prefab(prefab("Background_Cave_01_SET", BG_OVERRIDE)),
                LevelObject::Parent(ParentObject {
                    name: "Other".to_string(),
                    position: Vec3::default(),
                    children: vec![],
                    parent: None,
                }),
            ],
            roots: vec![0, 1],
        };

        assert_eq!(
            find_bg_override_text(&level.objects),
            Some(BG_OVERRIDE.to_string())
        );
    }

    #[test]
    fn finds_background_root_position_from_matching_prefab() {
        let mut bg = prefab("BackgroundObject", BG_OVERRIDE);
        bg.position = Vec3 {
            x: -172.66,
            y: -0.21,
            z: 0.0,
        };
        let level = LevelData {
            objects: vec![LevelObject::Prefab(bg)],
            roots: vec![0],
        };

        let position = find_bg_root_position(&level.objects)
            .expect("background root position should be detected");
        assert!((position.x + 172.66).abs() < f32::EPSILON);
        assert!((position.y + 0.21).abs() < f32::EPSILON);
        assert!(position.z.abs() < f32::EPSILON);
    }

    #[test]
    fn fit_to_level_prefers_camera_limits_over_object_bounds() {
        let mut renderer = LevelRenderer::new(None);
        renderer.world_positions = vec![
            (
                0,
                Vec3 {
                    x: -500.0,
                    y: -200.0,
                    z: 0.0,
                },
            ),
            (
                1,
                Vec3 {
                    x: 400.0,
                    y: 300.0,
                    z: 0.0,
                },
            ),
        ];
        renderer.camera_limits = Some([-20.0, 30.0, 80.0, 40.0]);

        renderer.fit_to_level();

        assert!((renderer.camera.center.x - 20.0).abs() < 0.001);
        assert!((renderer.camera.center.y - 10.0).abs() < 0.001);
        assert!((renderer.camera.zoom - (600.0 / 90.0)).abs() < 0.001);
    }

    fn prefab(name: &str, raw_text: &str) -> PrefabInstance {
        PrefabInstance {
            name: name.to_string(),
            position: Vec3::default(),
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
                raw_bytes: raw_text.as_bytes().to_vec(),
                raw_text: raw_text.to_string(),
            }),
            parent: None,
        }
    }
}
