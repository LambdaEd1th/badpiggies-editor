use std::collections::HashMap;

use super::types::LevelData;
use crate::unity_runtime::{Scene, components::LevelManager as RuntimeLevelManager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelWarningSeverity {
    High,
    Low,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelWarningKind {
    MultipleSlingshots,
    MultipleLevelManagers,
    MissingLevelManager,
    MultipleLevelStarts,
    MissingLevelStart,
    MultipleCameraSystems,
    MissingCameraSystem,
    MultipleGameCameras,
    MultipleHudCameras,
    MultipleWorldObjects,
    MissingWorldObject,
    MissingGoalArea,
    MultipleGoalAreas,
    MultipleDessertPlaces,
    MissingDessertPlaces,
    MultipleEffectManagers,
    MultipleIngameCameras,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelWarning {
    pub kind: LevelWarningKind,
    pub object_name: &'static str,
    pub count: usize,
}

impl LevelWarning {
    pub fn message_key(self) -> &'static str {
        match self.kind {
            LevelWarningKind::MultipleSlingshots => "level_warning_multiple_slingshots",
            LevelWarningKind::MissingLevelManager => "level_warning_missing_level_manager",
            LevelWarningKind::MissingLevelStart => "level_warning_missing_level_start",
            LevelWarningKind::MultipleCameraSystems => "level_warning_multiple_camera_system",
            LevelWarningKind::MissingCameraSystem => "level_warning_missing_camera_system",
            LevelWarningKind::MultipleGameCameras => "level_warning_multiple_game_camera",
            LevelWarningKind::MultipleHudCameras => "level_warning_multiple_hud_camera",
            LevelWarningKind::MultipleWorldObjects => "level_warning_multiple_world_object",
            LevelWarningKind::MissingWorldObject => "level_warning_missing_world_object",
            LevelWarningKind::MissingGoalArea => "level_warning_missing_goal_area",
            LevelWarningKind::MultipleGoalAreas => "level_warning_multiple_goal_area",
            LevelWarningKind::MultipleDessertPlaces => "level_warning_multiple_dessert_places",
            LevelWarningKind::MissingDessertPlaces => "level_warning_missing_dessert_places",
            LevelWarningKind::MultipleLevelManagers
            | LevelWarningKind::MultipleLevelStarts
            | LevelWarningKind::MultipleEffectManagers
            | LevelWarningKind::MultipleIngameCameras => "level_warning_multiple_singleton",
        }
    }

    pub fn severity(self) -> LevelWarningSeverity {
        match self.kind {
            LevelWarningKind::MultipleDessertPlaces | LevelWarningKind::MissingDessertPlaces => {
                LevelWarningSeverity::Low
            }
            _ => LevelWarningSeverity::High,
        }
    }
}

struct KnownRiskObject {
    name: &'static str,
    kind: LevelWarningKind,
}

const KNOWN_RISK_OBJECTS: [KnownRiskObject; 4] = [
    KnownRiskObject {
        name: "Slingshot",
        kind: LevelWarningKind::MultipleSlingshots,
    },
    KnownRiskObject {
        name: "LevelManager",
        kind: LevelWarningKind::MultipleLevelManagers,
    },
    KnownRiskObject {
        name: "EffectManager",
        kind: LevelWarningKind::MultipleEffectManagers,
    },
    KnownRiskObject {
        name: "IngameCamera",
        kind: LevelWarningKind::MultipleIngameCameras,
    },
];

const WORLD_TAGGED_BACKGROUND_OBJECTS: [&str; 13] = [
    "Background_Jungle_01_SET",
    "Background_Plateau_01_SET",
    "Background_Cave_01_SET 1",
    "Background_Night_01_SET 1",
    "Background_Forest_01_SET 1",
    "Background_Halloween",
    "Background_MM_01_SET",
    "Background_MM_Temple_01_SET_01",
    "Background_MM_Cave_01_SET",
    "Background_MM_Cave_01_SET_DARK",
    "Background_MM_Cave_02_SET_DARK",
    "Background_MM_High_01_SET",
    "BackgroundObject",
];

fn level_sandbox_flag(level: &LevelData) -> Option<bool> {
    let mut saw_true = false;
    let mut saw_false = false;

    for prefab in level.objects.iter().filter_map(|object| object.as_prefab()) {
        if prefab.name != "LevelManager" {
            continue;
        }
        let Some(raw_text) = prefab
            .override_data
            .as_ref()
            .map(|data| data.raw_text.as_str())
        else {
            continue;
        };
        let Some((scene, root)) = Scene::from_override_text(raw_text) else {
            continue;
        };
        let Some((_, level_manager)) = scene.get_component_of::<RuntimeLevelManager>(root) else {
            continue;
        };

        match level_manager.sandbox {
            Some(true) => saw_true = true,
            Some(false) => saw_false = true,
            None => {}
        }
    }

    match (saw_true, saw_false) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

pub fn collect_level_warnings(level: &LevelData) -> Vec<LevelWarning> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for object in &level.objects {
        *counts.entry(object.name()).or_default() += 1;
    }

    let level_manager_count = counts.get("LevelManager").copied().unwrap_or(0);
    let level_start_count = counts.get("LevelStart").copied().unwrap_or(0);
    let camera_system_count = counts.get("CameraSystem").copied().unwrap_or(0);
    let game_camera_count = counts.get("GameCamera").copied().unwrap_or(0);
    let hud_camera_count = counts.get("HUDCamera").copied().unwrap_or(0);
    let world_object_count = level
        .objects
        .iter()
        .filter(|object| WORLD_TAGGED_BACKGROUND_OBJECTS.contains(&object.name()))
        .count();
    let goal_area_count = level
        .objects
        .iter()
        .filter(|object| object.name().starts_with("GoalArea"))
        .count();
    let dessert_places_count = counts.get("DessertPlaces").copied().unwrap_or(0);
    let sandbox_flag = level_sandbox_flag(level);

    let mut warnings = Vec::new();

    if counts.get("Slingshot").copied().unwrap_or(0) > 1 {
        warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleSlingshots,
            object_name: "Slingshot",
            count: counts["Slingshot"],
        });
    }

    match level_manager_count {
        0 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingLevelManager,
            object_name: "LevelManager",
            count: 0,
        }),
        count if count > 1 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleLevelManagers,
            object_name: "LevelManager",
            count,
        }),
        _ => {}
    }

    match level_start_count {
        0 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingLevelStart,
            object_name: "LevelStart",
            count: 0,
        }),
        count if count > 1 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleLevelStarts,
            object_name: "LevelStart",
            count,
        }),
        _ => {}
    }

    match camera_system_count {
        0 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingCameraSystem,
            object_name: "CameraSystem",
            count: 0,
        }),
        count if count > 1 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleCameraSystems,
            object_name: "CameraSystem",
            count,
        }),
        _ => {}
    }

    if camera_system_count == 1 {
        if game_camera_count > 1 {
            warnings.push(LevelWarning {
                kind: LevelWarningKind::MultipleGameCameras,
                object_name: "GameCamera",
                count: game_camera_count,
            });
        }

        if hud_camera_count > 1 {
            warnings.push(LevelWarning {
                kind: LevelWarningKind::MultipleHudCameras,
                object_name: "HUDCamera",
                count: hud_camera_count,
            });
        }
    }

    match world_object_count {
        0 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingWorldObject,
            object_name: "World-tagged background",
            count: 0,
        }),
        count if count > 1 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleWorldObjects,
            object_name: "World-tagged background",
            count,
        }),
        _ => {}
    }

    if goal_area_count == 0 && sandbox_flag == Some(false) {
        warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingGoalArea,
            object_name: "GoalArea*",
            count: 0,
        });
    }

    if goal_area_count > 1 {
        warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleGoalAreas,
            object_name: "GoalArea*",
            count: goal_area_count,
        });
    }

    warnings.extend(KNOWN_RISK_OBJECTS.iter().filter_map(|rule| {
        let count = counts.get(rule.name).copied().unwrap_or(0);
        (count > 1 && rule.name != "Slingshot" && rule.name != "LevelManager").then_some(
            LevelWarning {
                kind: rule.kind,
                object_name: rule.name,
                count,
            },
        )
    }));

    match dessert_places_count {
        0 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MissingDessertPlaces,
            object_name: "DessertPlaces",
            count: 0,
        }),
        count if count > 1 => warnings.push(LevelWarning {
            kind: LevelWarningKind::MultipleDessertPlaces,
            object_name: "DessertPlaces",
            count,
        }),
        _ => {}
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::{LevelWarning, LevelWarningKind, LevelWarningSeverity, collect_level_warnings};
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };

    fn prefab(name: &str) -> LevelObject {
        LevelObject::Prefab(PrefabInstance {
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
            override_data: None,
            parent: None,
        })
    }

    fn prefab_with_override(name: &str, raw_text: &str) -> LevelObject {
        LevelObject::Prefab(PrefabInstance {
            name: name.to_string(),
            position: Vec3::default(),
            prefab_index: 0,
            rotation: Vec3::default(),
            scale: Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            data_type: DataType::PrefabOverrides,
            terrain_data: None,
            override_data: Some(PrefabOverrideData {
                raw_text: raw_text.to_string(),
                raw_bytes: raw_text.as_bytes().to_vec(),
            }),
            parent: None,
        })
    }

    fn level_manager_prefab(sandbox: Option<bool>) -> LevelObject {
        let mut raw_text = String::from("GameObject LevelManager\n\tComponent LevelManager\n");
        if let Some(sandbox) = sandbox {
            raw_text.push_str(if sandbox {
                "\t\tBoolean m_sandbox = True\n"
            } else {
                "\t\tBoolean m_sandbox = False\n"
            });
        }
        prefab_with_override("LevelManager", &raw_text)
    }

    #[test]
    fn warns_for_multiple_slingshots() {
        let level = LevelData {
            objects: vec![
                prefab("Slingshot"),
                prefab("Slingshot"),
                prefab("Pig"),
                prefab("DessertPlaces"),
                prefab("LevelManager"),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MultipleSlingshots,
                object_name: "Slingshot",
                count: 2,
            }]
        );
    }

    #[test]
    fn does_not_warn_for_missing_top_level_camera_children_with_single_camera_system() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5],
        };

        assert!(collect_level_warnings(&level).is_empty());
    }

    #[test]
    fn warns_for_missing_required_scene_objects() {
        let level = LevelData {
            objects: vec![prefab("Pig"), prefab("DessertPlaces")],
            roots: vec![0, 1],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![
                LevelWarning {
                    kind: LevelWarningKind::MissingLevelManager,
                    object_name: "LevelManager",
                    count: 0,
                },
                LevelWarning {
                    kind: LevelWarningKind::MissingLevelStart,
                    object_name: "LevelStart",
                    count: 0,
                },
                LevelWarning {
                    kind: LevelWarningKind::MissingCameraSystem,
                    object_name: "CameraSystem",
                    count: 0,
                },
                LevelWarning {
                    kind: LevelWarningKind::MissingWorldObject,
                    object_name: "World-tagged background",
                    count: 0,
                },
            ]
        );
    }

    #[test]
    fn warns_for_missing_world_object() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MissingWorldObject,
                object_name: "World-tagged background",
                count: 0,
            }]
        );
    }

    #[test]
    fn warns_for_multiple_world_objects() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("Background_Cave_01_SET 1"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MultipleWorldObjects,
                object_name: "World-tagged background",
                count: 2,
            }]
        );
    }

    #[test]
    fn warns_for_multiple_camera_children_with_single_camera_system() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![
                LevelWarning {
                    kind: LevelWarningKind::MultipleGameCameras,
                    object_name: "GameCamera",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleHudCameras,
                    object_name: "HUDCamera",
                    count: 2,
                },
            ]
        );
    }

    #[test]
    fn warns_for_multiple_camera_systems() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("GoalArea_01"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("CameraSystem"),
                prefab("CameraSystem"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MultipleCameraSystems,
                object_name: "CameraSystem",
                count: 2,
            }]
        );
    }

    #[test]
    fn warns_for_missing_goal_area_on_non_sandbox_levels() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("Pig"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MissingGoalArea,
                object_name: "GoalArea*",
                count: 0,
            }]
        );
    }

    #[test]
    fn skips_missing_goal_area_for_sandbox_levels() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(true)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("Pig"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7],
        };

        assert!(collect_level_warnings(&level).is_empty());
    }

    #[test]
    fn warns_for_multiple_goal_areas_by_prefix() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("DessertPlaces"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
                prefab("GoalArea_MM_Grey_Light"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MultipleGoalAreas,
                object_name: "GoalArea*",
                count: 2,
            }]
        );
    }

    #[test]
    fn warns_for_missing_dessert_places() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MissingDessertPlaces,
                object_name: "DessertPlaces",
                count: 0,
            }]
        );
    }

    #[test]
    fn warns_for_multiple_dessert_places() {
        let level = LevelData {
            objects: vec![
                level_manager_prefab(Some(false)),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("Background_Jungle_01_SET"),
                prefab("GoalArea_01"),
                prefab("DessertPlaces"),
                prefab("DessertPlaces"),
            ],
            roots: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![LevelWarning {
                kind: LevelWarningKind::MultipleDessertPlaces,
                object_name: "DessertPlaces",
                count: 2,
            }]
        );
    }

    #[test]
    fn marks_dessert_places_as_low_severity() {
        let warning = LevelWarning {
            kind: LevelWarningKind::MissingDessertPlaces,
            object_name: "DessertPlaces",
            count: 0,
        };

        assert_eq!(warning.severity(), LevelWarningSeverity::Low);
    }

    #[test]
    fn keeps_known_warning_order_stable() {
        let level = LevelData {
            objects: vec![
                prefab("EffectManager"),
                prefab("IngameCamera"),
                prefab("Slingshot"),
                prefab("LevelManager"),
                prefab("Slingshot"),
                prefab("EffectManager"),
                prefab("LevelManager"),
                prefab("IngameCamera"),
                prefab("LevelStart"),
                prefab("LevelStart"),
                prefab("CameraSystem"),
                prefab("CameraSystem"),
                prefab("GameCamera"),
                prefab("HUDCamera"),
                prefab("Background_Jungle_01_SET"),
                prefab("Background_Cave_01_SET 1"),
                prefab("GoalArea_01"),
                prefab("GoalArea_StarLevel"),
                prefab("DessertPlaces"),
                prefab("DessertPlaces"),
            ],
            roots: vec![
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ],
        };

        assert_eq!(
            collect_level_warnings(&level),
            vec![
                LevelWarning {
                    kind: LevelWarningKind::MultipleSlingshots,
                    object_name: "Slingshot",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleLevelManagers,
                    object_name: "LevelManager",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleLevelStarts,
                    object_name: "LevelStart",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleCameraSystems,
                    object_name: "CameraSystem",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleWorldObjects,
                    object_name: "World-tagged background",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleGoalAreas,
                    object_name: "GoalArea*",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleEffectManagers,
                    object_name: "EffectManager",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleIngameCameras,
                    object_name: "IngameCamera",
                    count: 2,
                },
                LevelWarning {
                    kind: LevelWarningKind::MultipleDessertPlaces,
                    object_name: "DessertPlaces",
                    count: 2,
                },
            ]
        );
    }
}
