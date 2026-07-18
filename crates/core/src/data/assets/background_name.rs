use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

const THEME_PRIORITY: &[&str] = &[
    "MayaCave2Dark",
    "MayaCaveDark",
    "MayaCave",
    "MayaTemple",
    "MayaHigh",
    "Maya",
    "Jungle",
    "Plateau",
    "Morning",
    "Night",
    "Halloween",
    "Cave",
];

pub fn theme_name_for_background_prefab(name: &str) -> Option<&'static str> {
    let normalized = normalize(name);
    aliases().get(&normalized).copied().or_else(|| {
        THEME_PRIORITY
            .iter()
            .copied()
            .find(|theme| normalize(theme) == normalized)
    })
}

/// Resolve the concrete background theme selected by a serialized
/// `BackgroundObject` prefab reference.
pub fn theme_name_for_background_override(
    level_key: &str,
    override_text: &str,
) -> Option<&'static str> {
    let reference_index = crate::domain::prefab_override::parse_override_text(override_text)
        .iter()
        .find_map(|root| {
            root.find_descendant(&|node| {
                node.node_type == "ObjectReference" && node.name == "prefab"
            })
        })
        .and_then(|node| node.value_as_i32())?;
    let prefab_name =
        crate::domain::level::refs::get_background_prefab_ref(level_key, reference_index)?;
    theme_name_for_background_prefab(prefab_name)
}

fn normalize(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator {
            normalized.push('_');
            separator = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn aliases() -> &'static HashMap<String, &'static str> {
    static ALIASES: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    ALIASES.get_or_init(|| {
        let mut aliases = HashMap::new();
        for path in super::list_pathnames("Assets/Resources/environment/background/", ".prefab") {
            let filename = path
                .strip_prefix("Assets/Resources/environment/background/")
                .unwrap_or(path.as_str());
            let Some(stem) = Path::new(filename)
                .file_stem()
                .and_then(|stem| stem.to_str())
            else {
                continue;
            };
            let Some(theme) = theme_from_stem(&normalize(stem)) else {
                continue;
            };
            aliases.insert(normalize(stem), theme);
            aliases.insert(normalize(filename), theme);
            aliases.insert(normalize(&path), theme);
        }
        aliases
    })
}

fn theme_from_stem(normalized: &str) -> Option<&'static str> {
    match normalized.strip_prefix("background_").unwrap_or(normalized) {
        "cave_01_set_1" => Some("Cave"),
        "forest_01_set_1" => Some("Morning"),
        "halloween" => Some("Halloween"),
        "jungle_01_set" => Some("Jungle"),
        "mm_01_set" => Some("Maya"),
        "mm_cave_01_set" => Some("MayaCave"),
        "mm_cave_01_set_dark" => Some("MayaCaveDark"),
        "mm_cave_02_set_dark" => Some("MayaCave2Dark"),
        "mm_high_01_set" => Some("MayaHigh"),
        "mm_temple_01_set_01" => Some("MayaTemple"),
        "night_01_set_1" => Some("Night"),
        "plateau_01_set" => Some("Plateau"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::theme_name_for_background_override;
    use crate::domain::parser::parse_level;
    use crate::domain::types::LevelObject;

    fn background_override(level: &crate::domain::types::LevelData) -> &str {
        level
            .objects
            .iter()
            .find_map(|object| match object {
                LevelObject::Prefab(prefab) if prefab.name == "BackgroundObject" => prefab
                    .override_data
                    .as_ref()
                    .map(|data| data.raw_text.as_str()),
                _ => None,
            })
            .expect("background override")
    }

    #[test]
    fn episode6_dark_level_keeps_its_explicit_maya_cave_background() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../test_levels/assetbundles/episode_6_levels.unity3d/episode_6_level_5_data.bytes",
        );
        let level = parse_level(std::fs::read(&path).expect("episode 6 level 5 fixture"))
            .expect("parse episode 6 level 5");
        let override_text = background_override(&level);

        assert_eq!(
            theme_name_for_background_override("episode_6_level_5_data", override_text),
            Some("MayaCave2Dark")
        );
    }

    #[test]
    fn every_episode6_dark_level_resolves_the_maya_cave_background() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../test_levels/assetbundles/episode_6_levels.unity3d");
        let mut paths = std::fs::read_dir(&root)
            .expect("episode 6 fixtures")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("episode_6_level_") && name.ends_with("_data.bytes")
                    })
            })
            .collect::<Vec<_>>();
        paths.sort();

        let mut dark_levels = Vec::new();
        for path in paths {
            let level = parse_level(std::fs::read(&path).expect("episode 6 fixture bytes"))
                .expect("parse episode 6 fixture");
            let is_dark = level.objects.iter().any(|object| {
                matches!(object, LevelObject::Prefab(prefab)
                    if prefab.name == "LevelManager"
                        && prefab.override_data.as_ref().is_some_and(|data|
                            data.raw_text.contains("Boolean m_darkLevel = True")))
            });
            if !is_dark {
                continue;
            }

            let file_name = path.file_name().unwrap().to_string_lossy();
            let level_key = crate::domain::level::refs::level_key_from_filename(&file_name);
            assert_eq!(
                theme_name_for_background_override(&level_key, background_override(&level)),
                Some("MayaCave2Dark"),
                "wrong explicit background for {file_name}"
            );
            dark_levels.push(file_name.into_owned());
        }

        assert_eq!(dark_levels.len(), 15, "unexpected EP6 dark-level set");
    }
}
