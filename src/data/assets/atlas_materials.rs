use std::collections::HashMap;
use std::sync::OnceLock;

use super::read_asset_text;

const ATLAS_MATERIALS_ASSET: &str = "unity/resources/utility/atlasmaterials.prefab";
const ATLAS_FILES: [&str; 3] = ["IngameAtlas.png", "IngameAtlas2.png", "IngameAtlas3.png"];

fn atlas_by_material_guid() -> &'static HashMap<String, String> {
    static INSTANCE: OnceLock<HashMap<String, String>> = OnceLock::new();
    INSTANCE.get_or_init(build_atlas_by_material_guid)
}

fn build_atlas_by_material_guid() -> HashMap<String, String> {
    let Some(text) = read_asset_text(ATLAS_MATERIALS_ASSET) else {
        log::error!("Failed to read {ATLAS_MATERIALS_ASSET} for atlas material mapping");
        return HashMap::new();
    };

    let mut guids_by_section: HashMap<&'static str, Vec<String>> = HashMap::new();
    let mut current_section: Option<&'static str> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        current_section = match trimmed {
            "normalMaterials:" => Some("normalMaterials"),
            "dimmedRenderQueueMaterials:" => Some("dimmedRenderQueueMaterials"),
            "renderQueueMaterials:" => Some("renderQueueMaterials"),
            "partQueueZMaterials:" => Some("partQueueZMaterials"),
            "grayMaterials:" => Some("grayMaterials"),
            _ if trimmed.ends_with(':') => None,
            _ => current_section,
        };

        if !trimmed.starts_with("- ") {
            continue;
        }
        let Some(section) = current_section else {
            continue;
        };
        let Some(guid) = extract_guid(trimmed) else {
            continue;
        };
        guids_by_section
            .entry(section)
            .or_default()
            .push(guid.to_string());
    }

    let mut atlas_by_guid = HashMap::new();
    for section in [
        "normalMaterials",
        "dimmedRenderQueueMaterials",
        "renderQueueMaterials",
        "partQueueZMaterials",
        "grayMaterials",
    ] {
        let Some(guids) = guids_by_section.get(section) else {
            continue;
        };
        for (index, guid) in guids.iter().take(ATLAS_FILES.len()).enumerate() {
            atlas_by_guid.insert(guid_prefix(guid).to_string(), ATLAS_FILES[index].to_string());
        }
    }

    atlas_by_guid
}

fn extract_guid(line: &str) -> Option<&str> {
    let start = line.find("guid: ")? + "guid: ".len();
    let rest = &line[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    let guid = &rest[..end];
    (!guid.is_empty()).then_some(guid)
}

fn guid_prefix(material_guid: &str) -> &str {
    material_guid.get(..8).unwrap_or(material_guid)
}

pub fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    atlas_by_material_guid()
        .get(guid_prefix(material_guid))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::atlas_for_material_guid;

    #[test]
    fn atlasmaterials_prefab_drives_ingame_atlas_mapping() {
        assert_eq!(
            atlas_for_material_guid("ce5a9931cec8f4b84741e1391306eb66"),
            Some("IngameAtlas.png")
        );
        assert_eq!(
            atlas_for_material_guid("4ab535f334efc6a54d4951c79cf5a28a"),
            Some("IngameAtlas2.png")
        );
        assert_eq!(
            atlas_for_material_guid("7975d66d65c10f9843645cf0df4004c6"),
            Some("IngameAtlas3.png")
        );
    }
}
