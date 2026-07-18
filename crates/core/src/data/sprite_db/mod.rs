//! Sprite database — rebuilds sprite atlas UV/sizing data from embedded Unity raw assets.
//!
//! Runtime Sprite entries come from prefab traversal plus Sprites.bytes and
//! spritemapping.bytes. Unmanaged decoration entries come from prefab YAML.

mod atlas;
mod builder;
mod parse;
mod runtime;
mod types;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::data::assets;

pub use types::{SpriteInfo, UvRect};

const PREFAB_DIR_ASSET: &str = "Assets/Prefab/";

/// World-size formula: pixelSize * prefabScale * 10 / 768
pub(super) const WORLD_SCALE: f32 = 10.0 / 768.0;

static SPRITE_DB: OnceLock<HashMap<String, SpriteInfo>> = OnceLock::new();
static RUNTIME_SPRITES: OnceLock<HashMap<String, types::RuntimeSpriteMeta>> = OnceLock::new();

struct SpriteSource {
    asset_path: String,
    info: OnceLock<Option<SpriteInfo>>,
}

fn runtime_sprites() -> &'static HashMap<String, types::RuntimeSpriteMeta> {
    RUNTIME_SPRITES.get_or_init(runtime::load_runtime_sprites)
}

/// Get the sprite database (lazily initialized).
pub fn sprite_db() -> &'static HashMap<String, SpriteInfo> {
    SPRITE_DB.get_or_init(build_db)
}

/// Look up sprite info by name, with normalization fallbacks.
pub fn get_sprite_info(name: &str) -> Option<&'static SpriteInfo> {
    // Direct lookup
    if let Some(s) = get_exact_sprite_info(name) {
        return Some(s);
    }

    // Strip " (N)" duplicates: "Bottle (2)" → "Bottle"
    if let Some(base) = name.split(" (").next()
        && base != name
        && let Some(s) = get_exact_sprite_info(base)
    {
        return Some(s);
    }

    // Strip trailing digits: "StarBox01" → "StarBox"
    let trimmed = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if trimmed != name
        && !trimmed.is_empty()
        && let Some(s) = get_exact_sprite_info(trimmed)
    {
        return Some(s);
    }

    // Strip "_001" style suffixes
    if let Some(pos) = name.rfind('_') {
        let suffix = &name[pos + 1..];
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            let base = &name[..pos];
            if let Some(s) = get_exact_sprite_info(base) {
                return Some(s);
            }
        }
    }

    // Common runtime/prefab alias: "Bird_Black" -> "Bird_Black_01"
    let suffixed = format!("{name}_01");
    if let Some(s) = get_exact_sprite_info(&suffixed) {
        return Some(s);
    }

    None
}

pub fn runtime_sprite_dimensions(sprite_id: &str) -> Option<(UvRect, f32, f32, String)> {
    runtime_sprites()
        .get(sprite_id)
        .map(|meta| (meta.uv, meta.width, meta.height, meta.material_id.clone()))
}

pub fn runtime_sprite_pivot(sprite_id: &str) -> Option<(f32, f32)> {
    runtime_sprites()
        .get(sprite_id)
        .map(|meta| (meta.pivot_x, meta.pivot_y))
}

fn build_db() -> HashMap<String, SpriteInfo> {
    sprite_sources()
        .iter()
        .filter_map(|(name, source)| {
            source
                .info
                .get_or_init(|| build_sprite_info(name, &source.asset_path))
                .as_ref()
                .map(|info| (name.clone(), info.clone()))
        })
        .collect()
}

fn sprite_sources() -> &'static HashMap<String, SpriteSource> {
    static SOURCES: OnceLock<HashMap<String, SpriteSource>> = OnceLock::new();

    SOURCES.get_or_init(|| {
        assets::list_pathnames(PREFAB_DIR_ASSET, ".prefab")
            .into_iter()
            .filter_map(|asset_path| {
                let filename = asset_path.strip_prefix(PREFAB_DIR_ASSET)?;
                let name = filename.strip_suffix(".prefab")?.to_string();
                Some((
                    name,
                    SpriteSource {
                        asset_path,
                        info: OnceLock::new(),
                    },
                ))
            })
            .collect()
    })
}

fn get_exact_sprite_info(name: &str) -> Option<&'static SpriteInfo> {
    let source = sprite_sources().get(name)?;
    source
        .info
        .get_or_init(|| build_sprite_info(name, &source.asset_path))
        .as_ref()
}

fn build_sprite_info(name: &str, asset_path: &str) -> Option<SpriteInfo> {
    let runtime_sprites = runtime_sprites();
    let Some(text) = read_embedded_text(asset_path) else {
        log::warn!("Missing embedded prefab for sprite database: {asset_path}");
        return None;
    };

    let parsed = parse::parse_prefab(&text);
    builder::find_runtime_sprite_info(&parsed, runtime_sprites)
        .or_else(|| builder::find_unmanaged_sprite_info(name, &parsed))
}

pub(super) fn read_embedded_text(path: &str) -> Option<String> {
    let bytes = assets::read_pathname(path)?;
    Some(String::from_utf8_lossy(bytes.as_ref()).into_owned())
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
    fn box_icon_uses_prefab_backed_ingame_atlas2() {
        let sprite = get_sprite_info("BoxIcon").expect("missing BoxIcon sprite info");
        assert_eq!(sprite.atlas, "IngameAtlas2.png");
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
    fn goal_area_mesh_prefabs_do_not_reuse_achievement_sprite() {
        assert!(get_sprite_info("GoalArea_01").is_none());
        assert!(get_sprite_info("GoalArea_02").is_none());
        assert!(get_sprite_info("GoalArea_StarLevel").is_none());
    }

    #[test]
    fn level_row_unlock_panel_uses_renderer_backed_menu_atlas() {
        let sprite = get_sprite_info("LevelRowUnlockPanel")
            .expect("missing LevelRowUnlockPanel sprite info");
        assert_eq!(sprite.atlas, "MenuAtlas.png");
        assert_close(sprite.uv.x, 0.7270508);
        assert_close(sprite.uv.y, 0.3481445);
        assert_close(sprite.uv.w, 0.05908203);
        assert_close(sprite.uv.h, 0.05908203);
        assert_close(sprite.world_w, 121.0 * 0.85 * WORLD_SCALE);
        assert_close(sprite.world_h, 121.0 * 0.85 * WORLD_SCALE);
    }

    #[test]
    fn daily_challenge_dialog_uses_renderer_backed_menu_atlas() {
        let sprite = get_sprite_info("DailyChallengeDialog")
            .expect("missing DailyChallengeDialog sprite info");
        assert_eq!(sprite.atlas, "MenuAtlas.png");
    }

    #[test]
    fn purchase_piggy_pack_iap_uses_renderer_backed_ingame_atlas2() {
        let sprite = get_sprite_info("PurchasePiggyPackIAP")
            .expect("missing PurchasePiggyPackIAP sprite info");
        assert_eq!(sprite.atlas, "IngameAtlas2.png");
    }

    #[test]
    fn ask_about_notifications_prefers_close_button_sprite() {
        let sprite = get_sprite_info("AskAboutNotifications")
            .expect("missing AskAboutNotifications sprite info");
        assert_eq!(sprite.atlas, "IngameAtlas2.png");
        assert_close(sprite.world_w, 0.7877604);
    }

    #[test]
    fn resource_bar_prefers_level_icon_sprite() {
        let sprite = get_sprite_info("ResourceBar").expect("missing ResourceBar sprite info");
        assert_eq!(sprite.atlas, "MenuAtlas2.png");
        assert_close(sprite.world_w, 0.6380208);
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

    #[test]
    fn star_boxes_and_tnt_box_have_sprite_info() {
        assert!(get_sprite_info("StarBox").is_some());
        assert!(get_sprite_info("DynamicStarBox").is_some());
        assert!(get_sprite_info("TNT_Box").is_some());
    }
}
