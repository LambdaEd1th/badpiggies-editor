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

    const MAYA_CAVE_2_DARK_OVERRIDE: &str =
        "GameObject BackgroundObject\n\tObjectReference prefab = 4";

    #[test]
    fn episode6_dark_level_keeps_its_explicit_maya_cave_background() {
        assert_eq!(
            theme_name_for_background_override("episode_6_level_5_data", MAYA_CAVE_2_DARK_OVERRIDE),
            Some("MayaCave2Dark")
        );
    }

    #[test]
    fn every_episode6_dark_level_resolves_the_maya_cave_background() {
        // This is the dark-level set serialized by the shipped EP6 bundle.
        for file_name in [
            "episode_6_level_5_data.bytes",
            "episode_6_level_6_data.bytes",
            "episode_6_level_7_data.bytes",
            "episode_6_level_8_data.bytes",
            "episode_6_level_17_data.bytes",
            "episode_6_level_18_data.bytes",
            "episode_6_level_19_data.bytes",
            "episode_6_level_20_data.bytes",
            "episode_6_level_29_data.bytes",
            "episode_6_level_30_data.bytes",
            "episode_6_level_31_data.bytes",
            "episode_6_level_32_data.bytes",
            "episode_6_level_II_data.bytes",
            "episode_6_level_V_data.bytes",
            "episode_6_level_VIII_data.bytes",
        ] {
            let level_key = crate::domain::level::refs::level_key_from_filename(file_name);
            assert_eq!(
                theme_name_for_background_override(&level_key, MAYA_CAVE_2_DARK_OVERRIDE),
                Some("MayaCave2Dark"),
                "wrong explicit background for {file_name}"
            );
        }
    }
}
