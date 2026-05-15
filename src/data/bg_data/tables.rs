use super::types::BgLayer;

pub(super) const BG_SPRITE_SCRIPT_GUID: &str = "b011dfa16a4475b746a1372ea41fdf05";
pub(super) const BG_TEXTURELOADER_ASSET: &str = "unity/resources/textureloader.prefab";
pub(super) const BG_THEME_PREFABS: &[(&str, &str)] = &[
    ("Cave", "unity/background/background_cave_01_set 1.prefab"),
    (
        "Morning",
        "unity/background/background_forest_01_set 1.prefab",
    ),
    ("Halloween", "unity/background/background_halloween.prefab"),
    ("Jungle", "unity/background/background_jungle_01_set.prefab"),
    ("Maya", "unity/background/background_mm_01_set.prefab"),
    (
        "MayaCave",
        "unity/background/background_mm_cave_01_set.prefab",
    ),
    (
        "MayaCaveDark",
        "unity/background/background_mm_cave_01_set_dark.prefab",
    ),
    (
        "MayaCave2Dark",
        "unity/background/background_mm_cave_02_set_dark.prefab",
    ),
    (
        "MayaHigh",
        "unity/background/background_mm_high_01_set.prefab",
    ),
    (
        "MayaTemple",
        "unity/background/background_mm_temple_01_set_01.prefab",
    ),
    ("Night", "unity/background/background_night_01_set 1.prefab"),
    (
        "Plateau",
        "unity/background/background_plateau_01_set.prefab",
    ),
];

pub(super) fn classify_group_layer(tag: &str, group_name: &str) -> BgLayer {
    match tag {
        "ParallaxLayerSky" => BgLayer::Sky,
        "ParallaxLayerFixedFollowCamera" => BgLayer::Camera,
        "ParallaxLayerFurther" => BgLayer::Further,
        "ParallaxLayerFar" => BgLayer::Far,
        "ParallaxLayerNear" => BgLayer::Near,
        "ParallaxLayerForeground" => BgLayer::Foreground,
        "Ground" => BgLayer::Ground,
        _ => {
            let lower = group_name.to_ascii_lowercase();
            if lower.contains("sky") {
                BgLayer::Sky
            } else if lower.contains("further") {
                BgLayer::Further
            } else if lower.contains("foreground") || lower.starts_with("fglayer") {
                BgLayer::Foreground
            } else if lower.contains("far") {
                BgLayer::Far
            } else if lower.contains("near") {
                BgLayer::Near
            } else if lower.contains("cloud") || lower.contains("moon") || lower.contains("castle")
            {
                BgLayer::Camera
            } else {
                BgLayer::Ground
            }
        }
    }
}

pub(super) fn supplemental_atlas_for_material(guid: &str) -> Option<&'static str> {
    match guid {
        "42e57a40" => Some("Background_Maya_Sheet_03.png"),
        "38ea809d" => Some("Background_Maya_Sheet_02.png"),
        "0de59521" => Some("Background_Maya_Sheet_02.png"),
        "c650b83a" => Some("Background_Maya_Sheet_04.png"),
        "d2458d0c" => Some("Background_Maya_Sheet_05.png"),
        "ac6e41ef" => Some("Background_Maya_Sheet_04.png"),
        "8429542c" => Some("Background_Maya_Sheet_04.png"),
        "ac9d3653" => Some("Background_Maya_Sheet_04.png"),
        "543a0873" => Some("Background_Maya_Sheet_03.png"),
        "ad0893eb" => Some("Background_Maya_Sheet_02.png"),
        "18df2da6" => Some("Background_Maya_Sheet_03.png"),
        "a79aee02" => Some("Background_Maya_Sheet_03.png"),
        "141823ce" => Some("Background_Maya_Sheet_03.png"),
        _ => None,
    }
}

pub(super) fn fill_color_override(
    theme: &str,
    sprite_name: &str,
    parent_group: &str,
) -> Option<[u8; 3]> {
    match (theme, sprite_name, parent_group) {
        ("Cave", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("Cave", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("Jungle", "Background_Far_fill", "BGLayerFar") => Some([0x54, 0xaa, 0x44]),
        ("Jungle", "Background_Near_fill", "BGLayerNear") => Some([0x33, 0x88, 0x44]),
        ("Jungle", "Dummy", "Ocean") => Some([0x44, 0xaa, 0x99]),
        ("Maya", "Background_Far_fill", "BGLayerFar") => Some([0xcd, 0xab, 0x74]),
        ("Maya", "Background_Far_fill2", "BGLayerFurther") => Some([0xdd, 0xdd, 0xdd]),
        ("Maya", "Dummy", "Ocean") => Some([0x14, 0xba, 0xdc]),
        ("MayaCave", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("MayaCave", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("MayaCaveDark", "Background_Far_fill", "BGLayerFar") => Some([0x21, 0x44, 0x21]),
        ("MayaCaveDark", "Background_Near_fill", "BGLayerNear") => Some([0x11, 0x21, 0x11]),
        ("MayaCave2Dark", "Background_Sky_Fill1", "Background_Sky_Fill1") => {
            Some([0x04, 0x0b, 0x12])
        }
        ("MayaCave2Dark", "Grass_fill", "GroundLayer") => Some([0x42, 0x42, 0x29]),
        ("MayaTemple", "Background_Sky", "Background_Sky") => Some([0xfd, 0xf8, 0x7b]),
        ("Morning", "Background_Far_fill", "BGLayerFar") => Some([0x6d, 0x7e, 0x96]),
        ("Morning", "Background_Near_fill", "BGLayerNear") => Some([0x3f, 0x4b, 0x5b]),
        ("Morning", "Dummy", "Ocean") => Some([0x4f, 0x5f, 0x82]),
        ("Morning", "Fill", "BGLayerForeground") => Some([0x11, 0x11, 0x11]),
        ("Plateau", "Background_Far_fill", "BGLayerFar") => Some([0xcc, 0xaa, 0x21]),
        ("Plateau", "Background_Near_fill", "BGLayerNear") => Some([0x88, 0x77, 0x21]),
        ("Plateau", "Fill", "FGLayer") => Some([0x21, 0x44, 0x44]),
        ("Plateau", "Grass_fill", "GrassLayer") => Some([0x33, 0x77, 0x66]),
        _ => None,
    }
}

pub(super) fn alpha_blend_override(
    theme: &str,
    sprite_name: &str,
    parent_group: &str,
    layer: BgLayer,
) -> bool {
    matches!(
        (theme, sprite_name, parent_group, layer),
        (
            "Morning",
            "Background_Jungle_02",
            "BGLayerFar",
            BgLayer::Far
        ) | ("Jungle", "Background_Jungle_02", "BGLayerFar", BgLayer::Far)
            | ("Maya", "Background_Maya_01", "BGLayerFar", BgLayer::Far)
            | ("Night", "Moon", _, BgLayer::Camera)
            | ("Halloween", _, _, BgLayer::Camera)
    )
}

pub(super) fn uses_own_group_context(theme: &str, sprite_name: &str, parent_group: &str) -> bool {
    matches!(
        (theme, sprite_name, parent_group),
        ("MayaCave2Dark", "Background_Sky_Fill1", "Background_Sky")
    )
}

/// All known background atlas filenames.
pub fn bg_atlas_files() -> &'static [&'static str] {
    &[
        "Background_Cave_Sheet_01.png",
        "Background_Halloween_Sheet_01.png",
        "Background_Jungle_Sheet_01.png",
        "Background_Jungle_Sheet_02.png",
        "Background_Maya_Sheet_01.png",
        "Background_Maya_Sheet_02.png",
        "Background_Maya_Sheet_03.png",
        "Background_Maya_Sheet_04.png",
        "Background_Maya_Sheet_05.png",
        "Background_Morning_Sheet_01.png",
        "Background_Morning_Sheet_02.png",
        "Background_Night_Sheet_01.png",
        "Background_Plateaus_Sheet_01.png",
    ]
}

/// All known sky texture filenames.
pub fn sky_texture_files() -> &'static [&'static str] {
    &[
        "Halloween_Sky_Texture.png",
        "Jungle_Sky_Texture.png",
        "Maya_Backgrounds_sky.png",
        "Morning_Sky_Texture.png",
        "Night_Sky_Texture.png",
        "Plateau_Sky_Texture.png",
    ]
}
