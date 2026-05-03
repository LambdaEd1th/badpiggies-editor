//! CJK font configuration for egui.

use eframe::egui;

/// Load a system CJK font and register it as a fallback for proportional + monospace.
pub(super) fn configure_cjk_fonts(ctx: &egui::Context) {
    let Some(data) = load_system_cjk_font() else {
        log::warn!("No system CJK font found — Chinese text will render as squares");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".into(),
        std::sync::Arc::new(egui::FontData::from_owned(data)),
    );
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("cjk".into());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("cjk".into());
    }

    ctx.set_fonts(fonts);
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
    Some(include_bytes!("../../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}

#[cfg(target_arch = "wasm32")]
fn load_system_cjk_font() -> Option<Vec<u8>> {
    Some(include_bytes!("../../assets/fonts/NotoSansCJKsc-Regular.otf").to_vec())
}
