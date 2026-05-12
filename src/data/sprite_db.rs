//! Sprite database — rebuilds sprite atlas UV/sizing data from embedded Unity raw assets.
//!
//! Runtime Sprite entries come from prefab traversal plus Sprites.bytes and
//! spritemapping.bytes. Unmanaged decoration entries come from prefab YAML.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;

/// Resolved sprite info ready for rendering.
#[derive(Debug, Clone)]
pub struct SpriteInfo {
    /// Atlas filename (e.g. "IngameAtlas.png").
    pub atlas: String,
    /// Normalized UV rect [0..1].
    pub uv: UvRect,
    /// Half-extent in world units.
    pub world_w: f32,
    pub world_h: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

const SPRITES_BYTES_ASSET: &str = "unity/resources/guisystem/Sprites.bytes";
const SPRITE_MAPPING_ASSET: &str = "unity/resources/guisystem/spritemapping.bytes";
const PREFAB_MANIFEST_ASSET: &str = "unity/prefabs/manifest.txt";
const PREFAB_DIR_ASSET: &str = "unity/prefabs";
const UNMANAGED_ATLAS: &str = "Props_Generic_Sheet_01.png";

/// World-size formula: pixelSize * prefabScale * 10 / 768
const WORLD_SCALE: f32 = 10.0 / 768.0;

#[derive(Debug, Clone)]
struct RuntimeSpriteMeta {
    material_id: String,
    width: f32,
    height: f32,
    uv: UvRect,
}

#[derive(Debug, Clone)]
struct GameObjectInfo {
    active: bool,
}

#[derive(Debug, Clone)]
struct TransformInfo {
    game_object_id: String,
    father: String,
    children: Vec<String>,
}

#[derive(Debug, Clone)]
struct RendererInfo {
    material_guid: String,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct RuntimeSpriteComponent {
    game_object_id: String,
    sprite_id: String,
    scale_x: f32,
    scale_y: f32,
}

#[derive(Debug, Clone)]
struct UnmanagedSpriteComponent {
    uv: UvRect,
    world_w: f32,
    world_h: f32,
}

#[derive(Default)]
struct ParsedPrefab {
    game_objects: HashMap<String, GameObjectInfo>,
    transforms: HashMap<String, TransformInfo>,
    renderers: HashMap<String, RendererInfo>,
    runtime_sprites: Vec<RuntimeSpriteComponent>,
    unmanaged_sprites: HashMap<String, UnmanagedSpriteComponent>,
}

static SPRITE_DB: OnceLock<HashMap<String, SpriteInfo>> = OnceLock::new();

fn build_db() -> HashMap<String, SpriteInfo> {
    let runtime_sprites = load_runtime_sprites();
    let Some(manifest) = read_embedded_text(PREFAB_MANIFEST_ASSET) else {
        log::error!(
            "Failed to read embedded prefab manifest for sprite database: {}",
            PREFAB_MANIFEST_ASSET
        );
        return HashMap::new();
    };

    let mut map = HashMap::new();
    for filename in manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if !filename.ends_with(".prefab") {
            continue;
        }
        let Some(name) = filename.strip_suffix(".prefab") else {
            continue;
        };
        let asset_path = format!("{}/{}", PREFAB_DIR_ASSET, filename);
        let Some(text) = read_embedded_text(&asset_path) else {
            log::warn!(
                "Missing embedded prefab for sprite database: {}",
                asset_path
            );
            continue;
        };

        let parsed = parse_prefab(&text);
        let info = find_runtime_sprite_info(name, &parsed, &runtime_sprites)
            .or_else(|| find_unmanaged_sprite_info(&parsed));
        if let Some(info) = info {
            map.insert(name.to_string(), info);
        }
    }

    map
}

fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_asset(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

fn load_runtime_sprites() -> HashMap<String, RuntimeSpriteMeta> {
    let mut sprite_data = HashMap::new();

    let Some(text) = read_embedded_text(SPRITES_BYTES_ASSET) else {
        log::error!(
            "Failed to read embedded {} for sprite database",
            SPRITES_BYTES_ASSET
        );
        return HashMap::new();
    };

    for line in text.lines() {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.len() < 14 {
            continue;
        }
        let Some(width) = fields[11].parse().ok() else {
            continue;
        };
        let Some(height) = fields[12].parse().ok() else {
            continue;
        };
        sprite_data.insert(
            fields[0].to_string(),
            RuntimeSpriteMeta {
                material_id: fields[2].to_string(),
                width,
                height,
                uv: UvRect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
            },
        );
    }

    let Some(text) = read_embedded_text(SPRITE_MAPPING_ASSET) else {
        log::error!(
            "Failed to read embedded {} for sprite database",
            SPRITE_MAPPING_ASSET
        );
        return HashMap::new();
    };

    for line in text.lines() {
        let fields: Vec<&str> = line.trim().split('\t').collect();
        if fields.len() < 5 {
            continue;
        }
        let Some(entry) = sprite_data.get_mut(fields[0]) else {
            continue;
        };
        let Some(x) = fields[1].parse().ok() else {
            continue;
        };
        let Some(y) = fields[2].parse().ok() else {
            continue;
        };
        let Some(w) = fields[3].parse().ok() else {
            continue;
        };
        let Some(h) = fields[4].parse().ok() else {
            continue;
        };
        entry.uv = UvRect { x, y, w, h };
    }

    sprite_data
        .into_iter()
        .filter(|(_, entry)| entry.uv.w > 0.0 && entry.uv.h > 0.0)
        .collect()
}

fn find_runtime_sprite_info(
    prefab_name: &str,
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
) -> Option<SpriteInfo> {
    if let Some(sprite_id) = preferred_runtime_sprite_id(prefab_name)
        && let Some(component) = parsed
            .runtime_sprites
            .iter()
            .find(|component| component.sprite_id == sprite_id)
        && let Some(info) =
            runtime_sprite_info_from_component(prefab_name, parsed, runtime_sprites, component)
    {
        return Some(info);
    }

    for component in &parsed.runtime_sprites {
        if let Some(info) =
            runtime_sprite_info_from_component(prefab_name, parsed, runtime_sprites, component)
        {
            return Some(info);
        }
    }

    None
}

fn runtime_sprite_info_from_component(
    prefab_name: &str,
    parsed: &ParsedPrefab,
    runtime_sprites: &HashMap<String, RuntimeSpriteMeta>,
    component: &RuntimeSpriteComponent,
) -> Option<SpriteInfo> {
    let meta = runtime_sprites.get(&component.sprite_id)?;
    let atlas = runtime_atlas_for(prefab_name, &meta.material_id).or_else(|| {
        parsed
            .renderers
            .get(&component.game_object_id)
            .and_then(|renderer| atlas_for_material_guid(&renderer.material_guid))
    })?;
    Some(SpriteInfo {
        atlas: atlas.to_string(),
        uv: meta.uv,
        world_w: meta.width * component.scale_x * WORLD_SCALE,
        world_h: meta.height * component.scale_y * WORLD_SCALE,
    })
}

fn find_unmanaged_sprite_info(parsed: &ParsedPrefab) -> Option<SpriteInfo> {
    let root_transform_id = parsed
        .transforms
        .iter()
        .find(|(_, transform)| transform.father == "0")
        .map(|(id, _)| id.clone())?;
    find_unmanaged_sprite_info_at(&root_transform_id, parsed)
}

fn find_unmanaged_sprite_info_at(transform_id: &str, parsed: &ParsedPrefab) -> Option<SpriteInfo> {
    let transform = parsed.transforms.get(transform_id)?;
    let game_object = parsed.game_objects.get(&transform.game_object_id)?;
    if !game_object.active {
        return None;
    }

    if let Some(renderer) = parsed.renderers.get(&transform.game_object_id)
        && renderer.enabled
        && let Some(component) = parsed.unmanaged_sprites.get(&transform.game_object_id)
    {
        return Some(SpriteInfo {
            atlas: UNMANAGED_ATLAS.to_string(),
            uv: component.uv,
            world_w: component.world_w,
            world_h: component.world_h,
        });
    }

    for child_id in &transform.children {
        if let Some(info) = find_unmanaged_sprite_info_at(child_id, parsed) {
            return Some(info);
        }
    }

    None
}

fn parse_prefab(text: &str) -> ParsedPrefab {
    let mut parsed = ParsedPrefab::default();

    for doc in text.split("--- ").skip(1) {
        let Some(header) = doc.lines().next().map(str::trim) else {
            continue;
        };
        let Some((type_id, file_id)) = parse_doc_header(header) else {
            continue;
        };

        match type_id {
            1 => parse_game_object(doc, &file_id, &mut parsed.game_objects),
            4 => parse_transform(doc, &file_id, &mut parsed.transforms),
            23 => parse_renderer(doc, &mut parsed.renderers),
            114 => parse_mono_behaviour(doc, &mut parsed),
            _ => {}
        }
    }

    parsed
}

fn parse_doc_header(header: &str) -> Option<(u32, String)> {
    let mut parts = header.split_whitespace();
    let type_part = parts.next()?.strip_prefix("!u!")?;
    let file_part = parts.next()?.strip_prefix('&')?;
    Some((type_part.parse().ok()?, file_part.to_string()))
}

fn parse_game_object(doc: &str, file_id: &str, game_objects: &mut HashMap<String, GameObjectInfo>) {
    let mut active = true;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_IsActive:") {
            active = value.trim() != "0";
        }
    }

    game_objects.insert(file_id.to_string(), GameObjectInfo { active });
}

fn parse_transform(doc: &str, file_id: &str, transforms: &mut HashMap<String, TransformInfo>) {
    let mut game_object_id = None;
    let mut father = String::from("0");
    let mut children = Vec::new();
    let mut in_children = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_Father:") {
            father = parse_file_id(value).unwrap_or_else(|| String::from("0"));
            in_children = false;
            continue;
        }
        if trimmed.starts_with("m_Children:") {
            in_children = !trimmed.contains("[]");
            continue;
        }
        if in_children {
            if trimmed.starts_with('-') {
                if let Some(child_id) = parse_file_id(trimmed) {
                    children.push(child_id);
                }
                continue;
            }
            if trimmed.starts_with("m_") {
                in_children = false;
            }
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };

    transforms.insert(
        file_id.to_string(),
        TransformInfo {
            game_object_id,
            father,
            children,
        },
    );
}

fn parse_renderer(doc: &str, renderers: &mut HashMap<String, RendererInfo>) {
    let mut game_object_id = None;
    let mut enabled = true;
    let mut material_guid = String::new();
    let mut in_materials = false;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_Enabled:") {
            enabled = value.trim() != "0";
            continue;
        }
        if trimmed.starts_with("m_Materials:") {
            in_materials = true;
            continue;
        }
        if in_materials {
            if let Some(guid) = parse_guid(trimmed) {
                material_guid = guid;
                in_materials = false;
                continue;
            }
            if trimmed.starts_with("m_") {
                in_materials = false;
            }
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };
    renderers.insert(
        game_object_id,
        RendererInfo {
            material_guid,
            enabled,
        },
    );
}

fn parse_mono_behaviour(doc: &str, parsed: &mut ParsedPrefab) {
    let mut game_object_id = None;

    let mut sprite_id = None;
    let mut scale_x = None;
    let mut scale_y = None;

    let mut uv_x = None;
    let mut uv_y = None;
    let mut grid_w = None;
    let mut grid_h = None;
    let mut sprite_w = None;
    let mut sprite_h = None;
    let mut subdiv_x = None;
    let mut subdiv_y = None;

    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("m_GameObject:") {
            game_object_id = parse_file_id(value);
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_id:") {
            sprite_id = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleX:") {
            scale_x = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_scaleY:") {
            scale_y = value.trim().parse().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_UVx:") {
            uv_x = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_UVy:") {
            uv_y = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_width:") {
            grid_w = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_height:") {
            grid_h = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_spriteWidth:") {
            sprite_w = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_spriteHeight:") {
            sprite_h = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_atlasGridSubdivisions:") {
            let parsed_value = value.trim().parse::<f32>().ok();
            subdiv_x = parsed_value;
            subdiv_y = parsed_value;
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_subdivisionsX:") {
            subdiv_x = value.trim().parse::<f32>().ok();
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("m_subdivisionsY:") {
            subdiv_y = value.trim().parse::<f32>().ok();
        }
    }

    let Some(game_object_id) = game_object_id else {
        return;
    };

    if let (Some(sprite_id), Some(scale_x), Some(scale_y)) = (sprite_id, scale_x, scale_y) {
        parsed.runtime_sprites.push(RuntimeSpriteComponent {
            game_object_id,
            sprite_id,
            scale_x,
            scale_y,
        });
        return;
    }

    let (Some(uv_x), Some(uv_y), Some(grid_w), Some(grid_h), Some(sprite_w), Some(sprite_h)) =
        (uv_x, uv_y, grid_w, grid_h, sprite_w, sprite_h)
    else {
        return;
    };
    let subdiv_x = subdiv_x.unwrap_or(0.0);
    let subdiv_y = subdiv_y.unwrap_or(subdiv_x);
    if subdiv_x <= 0.0 || subdiv_y <= 0.0 {
        return;
    }

    parsed.unmanaged_sprites.insert(
        game_object_id,
        UnmanagedSpriteComponent {
            uv: UvRect {
                x: uv_x / subdiv_x,
                y: uv_y / subdiv_y,
                w: grid_w / subdiv_x,
                h: grid_h / subdiv_y,
            },
            world_w: sprite_w * WORLD_SCALE,
            world_h: sprite_h * WORLD_SCALE,
        },
    );
}

fn parse_file_id(text: &str) -> Option<String> {
    let rest = text[text.find("fileID:")? + "fileID:".len()..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    let file_id = &rest[..end];
    (!file_id.is_empty()).then(|| file_id.to_string())
}

fn parse_guid(text: &str) -> Option<String> {
    let rest = text[text.find("guid:")? + "guid:".len()..].trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    let guid = &rest[..end];
    (!guid.is_empty()).then(|| guid.to_string())
}

fn runtime_atlas_for(name: &str, material_id: &str) -> Option<&'static str> {
    match material_id {
        "04f5524815177fe408f9529a451cd50b"
        | "4f58ebaf253ff4341a0acfb2cdf671e6"
        | "84749b95c69414bd28c174439810c2b0"
        | "9148d367fdd7f4e5382e2b9cdf74b461"
        | "a286b652d38de4df384036482abc0571"
        | "d96ee5bd8db944803a0071fc972963a1" => Some("Ingame_Characters_Sheet_01.png"),
        "0bc3a371695e64987907110f53db83ec"
        | "1f3758e86c0414579989dc55480b23bc"
        | "a96fc7a314a89c041bb1a95fd4c281bf"
        | "e0b533defb69748799a56d4ee3b4260b" => Some("IngameAtlas2.png"),
        "38fb36d9f174d40fe859d55deb429e95"
        | "72a903c5f189843248f4878232222af4"
        | "c2c38ca20a8d040139cb7369bf7be51e"
        | "c6cc840a754074fe88e9517644258dc2"
        | "e02abde8d05ec499e9a2ba7f4850c971"
        | "eba8c92c7583e4309be6ad3f5e17e27e"
        | "fa87a551483ef4ba690410a25612e993" => Some("IngameAtlas3.png"),
        "4f53843aa7627f441a5d9e797e1745d9"
        | "51f3931e706115d468eca7f64035a4df"
        | "7844d45e898ea1441a473a80684bf4c4"
        | "89bbb403e054f204395edd3b8fea8241"
        | "bb7f816eb0cbb9f4584de6251fbc6eec"
        | "f6f09bead2bfb8c47957022817a31c85"
        | "f8cf0aa9b5c55c3469100b3a7044d86e" => Some("IngameAtlas.png"),
        "bfa953ce5fc274b6faa59deca6579361" | "f300c561f75e74380a11f80d4d2647f3" => {
            Some("Ingame_Sheet_04.png")
        }
        "1dc9819db44b840edb8cdec9ef7b80c2" => {
            if name == "LevelRowUnlockPanel" {
                Some("Ingame_Sheet_04.png")
            } else {
                Some("Ingame_Characters_Sheet_01.png")
            }
        }
        "20936c462fac24dbb967c450f9cb0cb4" => {
            if name == "GridCell" {
                Some("IngameAtlas2.png")
            } else {
                Some("Ingame_Characters_Sheet_01.png")
            }
        }
        "32e759dd981a043fa8fbcfd4997143ea" => match name {
            "DailyChallengeDialog" | "LeaderboardDialog" | "SeasonEndDialog" | "SnoutCoinShop" => {
                Some("Ingame_Sheet_04.png")
            }
            _ => Some("Ingame_Characters_Sheet_01.png"),
        },
        _ => None,
    }
}

fn preferred_runtime_sprite_id(name: &str) -> Option<&'static str> {
    match name {
        "AskAboutNotifications" => Some("ab0c6536-dfc1-46a1-8276-59280b355188"),
        "CakeRaceReplayEntry" => Some("a6cac51f-48ca-46da-b2e0-35cb3eacc819"),
        "CoinSalePopup" | "CrateCrazePopup" => Some("d37f6015-afdb-484e-b57f-451218f82ac2"),
        "ConfirmationErrorDialog"
        | "NoFreeSlotsPopup"
        | "RewardPopup"
        | "SandboxUnlock"
        | "VideoNotFoundDialog" => Some("690f29d0-ee21-4724-b083-71eb5e27e6ac"),
        "DailyChallengeDialog" | "SnoutCoinShop" => Some("ef7ae3f3-3a36-4b57-b209-f630d4837795"),
        "LeaderboardDialog" | "SeasonEndDialog" => Some("c41d8f89-5141-453b-8bdb-e42dde37860e"),
        "LeaderboardEntry" | "SingleLeaderboardEntry" => {
            Some("1d802ff7-c5a1-45ef-a084-97f81f37f0c8")
        }
        "LevelRowUnlockPanel" => Some("3f47c76b-3891-4685-adae-029a5e655dc5"),
        "PurchasePiggyPackIAP" | "WatchSnoutCoinAd" => Some("ab0c6536-dfc1-46a1-8276-59280b355188"),
        "ResourceBar" => Some("eea6164b-a556-4787-9420-d82b390e6675"),
        "ScrapButton" => Some("913d3f55-e5fe-49f7-b072-ad18875d9ce0"),
        "SnoutButton" => Some("dfb4e969-93e2-4d7d-969b-29732cc266c7"),
        "WorkshopIntroduction" => Some("f4bb39c9-0562-4c34-bc01-b66ce7c4edc2"),
        _ => None,
    }
}

fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
    let prefix = material_guid.get(..8).unwrap_or(material_guid);
    match prefix {
        "ce5a9931" | "d645821c" | "125eb5b4" | "0e790fab" | "353dd850" => Some("IngameAtlas.png"),
        "211b2b9c" | "aca6a4c6" | "765e60c2" | "4ab535f3" | "4eeb62bc" => Some("IngameAtlas2.png"),
        "2a21c011" | "ad767d84" | "7192b13e" | "a6f51d97" | "7975d66d" => Some("IngameAtlas3.png"),
        _ => None,
    }
}

/// Get the sprite database (lazily initialized).
pub fn sprite_db() -> &'static HashMap<String, SpriteInfo> {
    SPRITE_DB.get_or_init(build_db)
}

/// Look up sprite info by name, with normalization fallbacks.
pub fn get_sprite_info(name: &str) -> Option<&'static SpriteInfo> {
    let db = sprite_db();

    // Direct lookup
    if let Some(s) = db.get(name) {
        return Some(s);
    }

    // Strip " (N)" duplicates: "Bottle (2)" → "Bottle"
    if let Some(base) = name.split(" (").next()
        && base != name
        && let Some(s) = db.get(base)
    {
        return Some(s);
    }

    // Strip trailing digits: "StarBox01" → "StarBox"
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != name
        && !trimmed.is_empty()
        && let Some(s) = db.get(trimmed)
    {
        return Some(s);
    }

    // Strip "_001" style suffixes
    if let Some(pos) = name.rfind('_') {
        let suffix = &name[pos + 1..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            let base = &name[..pos];
            if let Some(s) = db.get(base) {
                return Some(s);
            }
        }
    }

    // Common runtime/prefab alias: "Bird_Black" -> "Bird_Black_01"
    let suffixed = format!("{name}_01");
    if let Some(s) = db.get(&suffixed) {
        return Some(s);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{WORLD_SCALE, get_sprite_info};

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn box_icon_uses_character_sheet_alias() {
        let sprite = get_sprite_info("BoxIcon").expect("missing BoxIcon sprite info");
        assert_eq!(sprite.atlas, "Ingame_Characters_Sheet_01.png");
        assert_close(sprite.uv.x, 0.6630859);
        assert_close(sprite.uv.y, 0.6743164);
        assert_close(sprite.uv.w, 0.02587891);
        assert_close(sprite.uv.h, 0.02587891);
        assert_close(sprite.world_w, 53.0 * WORLD_SCALE);
        assert_close(sprite.world_h, 53.0 * WORLD_SCALE);
    }

    #[test]
    fn goal_area_prefers_runtime_sprite_over_unmanaged_fallback() {
        let sprite =
            get_sprite_info("GoalArea_MM_Gold").expect("missing GoalArea_MM_Gold sprite info");
        assert_eq!(sprite.atlas, "IngameAtlas2.png");
        assert_close(sprite.uv.x, 0.5419922);
        assert_close(sprite.uv.y, 0.7719727);
        assert_close(sprite.uv.w, 0.02929688);
        assert_close(sprite.uv.h, 0.06054688);
        assert_close(sprite.world_w, 60.0 * 0.4 * WORLD_SCALE);
        assert_close(sprite.world_h, 124.0 * 0.4 * WORLD_SCALE);
    }

    #[test]
    fn level_row_unlock_panel_uses_background_runtime_sprite() {
        let sprite = get_sprite_info("LevelRowUnlockPanel")
            .expect("missing LevelRowUnlockPanel sprite info");
        assert_eq!(sprite.atlas, "Ingame_Sheet_04.png");
        assert_close(sprite.uv.x, 0.7270508);
        assert_close(sprite.uv.y, 0.3481445);
        assert_close(sprite.uv.w, 0.05908203);
        assert_close(sprite.uv.h, 0.05908203);
        assert_close(sprite.world_w, 121.0 * 0.85 * WORLD_SCALE);
        assert_close(sprite.world_h, 121.0 * 0.85 * WORLD_SCALE);
    }

    #[test]
    fn mushroom_1_uses_unmanaged_grid_data() {
        let sprite = get_sprite_info("Mushroom_1").expect("missing Mushroom_1 sprite info");
        assert_eq!(sprite.atlas, "Props_Generic_Sheet_01.png");
        assert_close(sprite.uv.x, 0.0);
        assert_close(sprite.uv.y, 0.5);
        assert_close(sprite.uv.w, 0.125);
        assert_close(sprite.uv.h, 0.125);
        assert_close(sprite.world_w, 38.0 * WORLD_SCALE);
        assert_close(sprite.world_h, 38.0 * WORLD_SCALE);
    }

    #[test]
    fn bird_black_alias_falls_back_to_bird_black_01() {
        let alias = get_sprite_info("Bird_Black").expect("missing Bird_Black alias");
        let direct = get_sprite_info("Bird_Black_01").expect("missing Bird_Black_01 sprite info");
        assert_eq!(alias.atlas, direct.atlas);
        assert_close(alias.uv.x, direct.uv.x);
        assert_close(alias.uv.y, direct.uv.y);
        assert_close(alias.uv.w, direct.uv.w);
        assert_close(alias.uv.h, direct.uv.h);
        assert_close(alias.world_w, direct.world_w);
        assert_close(alias.world_h, direct.world_h);
    }
}
