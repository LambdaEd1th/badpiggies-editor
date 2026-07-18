use std::collections::HashMap;

use super::level::refs::{
    first_prefab_index_with_prefix, level_key_from_filename, prefab_index_for_name,
};
use super::terrain_gen::{CurveNode, regenerate_terrain};
use super::types::{
    Color, CurveTexture, DataType, LevelData, LevelObject, PrefabInstance, TerrainData,
    TerrainMesh, Vec2, Vec3,
};

const DEFAULT_TERRAIN_NAME: &str = "e2dTerrainBase";

fn terrain_template_for_level(level: &LevelData, wants_collider: bool) -> Option<&PrefabInstance> {
    let mut terrain_name_counts = HashMap::<&str, usize>::new();
    for object in &level.objects {
        let LevelObject::Prefab(prefab) = object else {
            continue;
        };
        let Some(terrain) = prefab.terrain_data.as_deref() else {
            continue;
        };
        if terrain.has_collider == wants_collider {
            *terrain_name_counts.entry(prefab.name.as_str()).or_default() += 1;
        }
    }

    let dominant_name = terrain_name_counts
        .into_iter()
        .max_by(|(name_a, count_a), (name_b, count_b)| {
            count_a.cmp(count_b).then_with(|| name_b.cmp(name_a))
        })
        .map(|(name, _)| name);

    dominant_name
        .and_then(|name| {
            level.objects.iter().find_map(|object| {
                let LevelObject::Prefab(prefab) = object else {
                    return None;
                };
                let terrain = prefab.terrain_data.as_deref()?;
                (terrain.has_collider == wants_collider && prefab.name == name).then_some(prefab)
            })
        })
        .or_else(|| {
            level.objects.iter().find_map(|object| {
                let LevelObject::Prefab(prefab) = object else {
                    return None;
                };
                let terrain = prefab.terrain_data.as_deref()?;
                (terrain.has_collider == wants_collider).then_some(prefab)
            })
        })
        .or_else(|| {
            level.objects.iter().find_map(|object| {
                let LevelObject::Prefab(prefab) = object else {
                    return None;
                };
                prefab.terrain_data.as_ref()?;
                Some(prefab)
            })
        })
}

pub fn preferred_terrain_prefab_identity(
    level: &LevelData,
    file_name: Option<&str>,
    wants_collider: bool,
) -> (String, i16) {
    if let Some(template) = terrain_template_for_level(level, wants_collider) {
        return (template.name.clone(), template.prefab_index);
    }

    let prefab_index = file_name
        .map(level_key_from_filename)
        .and_then(|level_key| {
            prefab_index_for_name(&level_key, DEFAULT_TERRAIN_NAME)
                .or_else(|| first_prefab_index_with_prefix(&level_key, "e2dTerrain"))
        })
        .unwrap_or(0);
    (DEFAULT_TERRAIN_NAME.to_string(), prefab_index)
}

pub fn build_terrain_prefab_from_local_nodes(
    level: &LevelData,
    file_name: Option<&str>,
    center: Vec2,
    local_nodes: Vec<CurveNode>,
    wants_collider: bool,
) -> PrefabInstance {
    let mut terrain = TerrainData {
        fill_texture_tile_offset_x: 0.0,
        fill_texture_tile_offset_y: 0.0,
        fill_mesh: TerrainMesh::default(),
        fill_color: Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        fill_texture_index: 0,
        curve_mesh: TerrainMesh::default(),
        curve_textures: vec![
            CurveTexture {
                texture_index: 0,
                size: Vec2 { x: 1.0, y: 0.5 },
                fixed_angle: false,
                fade_threshold: 0.0,
            },
            CurveTexture {
                texture_index: 1,
                size: Vec2 { x: 1.0, y: 0.1 },
                fixed_angle: false,
                fade_threshold: 0.0,
            },
        ],
        control_texture_count: 0,
        control_texture_data: None,
        has_collider: wants_collider,
        fill_boundary: None,
    };

    let (name, prefab_index) = preferred_terrain_prefab_identity(level, file_name, wants_collider);
    if let Some(template) = terrain_template_for_level(level, wants_collider) {
        let template = template
            .terrain_data
            .as_deref()
            .expect("terrain template must contain terrain data");
        terrain.fill_texture_tile_offset_x = template.fill_texture_tile_offset_x;
        terrain.fill_texture_tile_offset_y = template.fill_texture_tile_offset_y;
        terrain.fill_color = template.fill_color;
        terrain.fill_texture_index = template.fill_texture_index;
        if !template.curve_textures.is_empty() {
            terrain.curve_textures = template.curve_textures.clone();
        }
    }
    regenerate_terrain(&mut terrain, &local_nodes);

    PrefabInstance {
        name,
        position: Vec3 {
            x: center.x,
            y: center.y,
            z: 0.0,
        },
        prefab_index,
        rotation: Vec3::default(),
        scale: Vec3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        data_type: DataType::Terrain,
        terrain_data: Some(Box::new(terrain)),
        override_data: None,
        parent: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::level::refs::get_prefab_override;

    fn sample_terrain_data(has_collider: bool) -> TerrainData {
        TerrainData {
            fill_texture_tile_offset_x: 12.0,
            fill_texture_tile_offset_y: -4.0,
            fill_mesh: TerrainMesh::default(),
            fill_color: Color {
                r: 0.5,
                g: 0.6,
                b: 0.7,
                a: 1.0,
            },
            fill_texture_index: 32,
            curve_mesh: TerrainMesh::default(),
            curve_textures: vec![
                CurveTexture {
                    texture_index: 7,
                    size: Vec2 { x: 0.5, y: 0.5 },
                    fixed_angle: false,
                    fade_threshold: 0.25,
                },
                CurveTexture {
                    texture_index: 9,
                    size: Vec2 { x: 0.1, y: 0.1 },
                    fixed_angle: false,
                    fade_threshold: 0.0,
                },
            ],
            control_texture_count: 0,
            control_texture_data: None,
            has_collider,
            fill_boundary: None,
        }
    }

    #[test]
    fn created_terrain_reuses_the_dominant_matching_template() {
        let template = PrefabInstance {
            name: "e2dTerrainBase_MM_rock".to_string(),
            position: Vec3::default(),
            prefab_index: 12,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            data_type: DataType::Terrain,
            terrain_data: Some(Box::new(sample_terrain_data(true))),
            override_data: None,
            parent: None,
        };
        let level = LevelData {
            objects: vec![LevelObject::Prefab(template)],
            roots: vec![0],
        };

        let created = build_terrain_prefab_from_local_nodes(
            &level,
            None,
            Vec2 { x: 10.0, y: 20.0 },
            vec![
                CurveNode {
                    position: Vec2 { x: -1.0, y: 0.0 },
                    texture: 0,
                },
                CurveNode {
                    position: Vec2 { x: 1.0, y: 0.0 },
                    texture: 1,
                },
            ],
            true,
        );

        assert_eq!(created.name, "e2dTerrainBase_MM_rock");
        assert_eq!(created.prefab_index, 12);
        let terrain = created.terrain_data.expect("created terrain data");
        assert_eq!(terrain.fill_texture_index, 32);
        assert_eq!(terrain.curve_textures[1].texture_index, 9);
    }

    #[test]
    fn empty_level_uses_the_loader_terrain_prefab_index() {
        let level = LevelData::default();
        let (name, prefab_index) =
            preferred_terrain_prefab_identity(&level, Some("Level_14_data.bytes"), true);

        assert_eq!(name, DEFAULT_TERRAIN_NAME);
        assert!(
            get_prefab_override("Level_14_data", prefab_index)
                .is_some_and(|resolved| resolved.starts_with("e2dTerrain"))
        );
    }
}
