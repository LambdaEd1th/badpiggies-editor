use super::get_theme;

#[test]
fn maya_cave2dark_sky_fill_uses_own_group_and_fill_color() {
    let Some(theme) = get_theme("MayaCave2Dark") else {
        panic!("missing MayaCave2Dark theme");
    };
    let Some(sprite) = theme
        .sprites
        .iter()
        .find(|sprite| sprite.name == "Background_Sky_Fill1")
    else {
        panic!("missing Background_Sky_Fill1");
    };

    assert_eq!(sprite.parent_group, "Background_Sky_Fill1");
    assert_eq!(sprite.fill_color, Some([0x04, 0x0b, 0x12]));
    assert!(sprite.atlas.is_none());
}

#[test]
fn maya_cave2dark_fg_uses_sheet_02() {
    let Some(theme) = get_theme("MayaCave2Dark") else {
        panic!("missing MayaCave2Dark theme");
    };

    for sprite in theme.sprites.iter().filter(|sprite| {
        sprite.parent_group == "FGLayer" && matches!(sprite.name.as_str(), "Fill2" | "Pillars01")
    }) {
        assert_eq!(
            sprite.atlas.as_deref(),
            Some("Background_Maya_Sheet_02.png"),
            "unexpected atlas for {}",
            sprite.name
        );
    }
}

#[test]
fn maya_temple_uses_expected_maya_sheets() {
    let Some(theme) = get_theme("MayaTemple") else {
        panic!("missing MayaTemple theme");
    };

    for sprite in theme.sprites.iter().filter(|sprite| {
        sprite.parent_group == "FGLayer"
            && matches!(
                sprite.name.as_str(),
                "Background_Maya_Temple_FG"
                    | "Background_Maya_Temple_FG_Fill"
                    | "Background_Maya_Temple_Near_Base"
                    | "Background_Maya_Temple_Near_Top"
            )
    }) {
        assert_eq!(
            sprite.atlas.as_deref(),
            Some("Background_Maya_Sheet_05.png"),
            "unexpected FG atlas for {}",
            sprite.name
        );
    }

    for sprite in theme.sprites.iter().filter(|sprite| {
        sprite.parent_group == "BGLayerNearBottom"
            && matches!(
                sprite.name.as_str(),
                "Background_Maya_Temple_Near_01"
                    | "Background_Maya_Temple_Near_02"
                    | "Background_Maya_Temple_Near_03"
                    | "Background_Maya_Temple_Near_04"
            )
    }) {
        assert_eq!(
            sprite.atlas.as_deref(),
            Some("Background_Maya_Sheet_04.png"),
            "unexpected near-bottom atlas for {}",
            sprite.name
        );
    }
}
