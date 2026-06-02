//! Runtime UI font configuration for egui.

use eframe::egui;

/// Load runtime UI fonts and register them as fallbacks for proportional + monospace.
pub(super) fn configure_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    for (name, data) in load_runtime_ui_fonts() {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );
    }

    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("ui-latin".into());
        list.push("ui-arabic".into());
        list.push("cjk".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("ui-latin".into());
        list.push("ui-arabic".into());
        list.push("cjk".into());
    }

    ctx.set_fonts(fonts);
}

fn load_runtime_ui_fonts() -> Vec<(&'static str, Vec<u8>)> {
    let mut fonts = Vec::new();

    fonts.push((
        "ui-latin",
        crate::data::runtime_assets::read_runtime_asset_bytes("fonts/NotoSans-Regular.ttf"),
    ));
    fonts.push((
        "ui-arabic",
        crate::data::runtime_assets::read_runtime_asset_bytes("fonts/NotoNaskhArabic-Regular.ttf"),
    ));

    if let Some(data) = load_system_cjk_font() {
        fonts.push(("cjk", data));
    } else {
        log::warn!("No CJK font found — Chinese text will render as squares");
    }

    fonts
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    let candidates = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    for path in &candidates {
        if let Ok(data) = std::fs::read(path) {
            log::info!("Loaded CJK font: {}", path);
            return Some(data);
        }
    }
    Some(crate::data::runtime_assets::read_runtime_asset_bytes(
        "fonts/NotoSansCJKsc-Regular.otf",
    ))
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    Some(crate::data::runtime_assets::read_runtime_asset_bytes(
        "fonts/NotoSansCJKsc-Regular.otf",
    ))
}
