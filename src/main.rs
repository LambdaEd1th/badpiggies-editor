//! Bad Piggies Level Editor — Rust/egui rewrite.
//! Supports both native desktop and WASM (GitHub Pages) targets.

mod app;
mod assets;
mod bg_data;
mod level_refs;
mod locale;
mod parser;
mod renderer;
mod sprite_db;
mod types;

use app::EditorApp;

// ── Screen size detection ────────────────────────────
/// Returns 80% of the primary monitor's logical resolution (points, not pixels).
/// Falls back to 1600×1000 on unsupported platforms or errors.
#[cfg(not(target_arch = "wasm32"))]
fn get_screen_size_80pct() -> (f32, f32) {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            // Prefer "UI Looks like: W x H" (logical points), fall back to "Resolution: W x H"
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(after) = trimmed.strip_prefix("UI Looks like:") {
                    // "UI Looks like: 1920 x 1080 @ 60Hz" — logical resolution
                    let parts: Vec<&str> = after.split_whitespace().collect();
                    if parts.len() >= 3
                        && let (Ok(lw), Ok(lh)) = (parts[0].parse::<f32>(), parts[2].parse::<f32>())
                    {
                        return (lw * 0.5, lh * 0.5);
                    }
                }
            }
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Resolution:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 4
                        && let (Ok(pw), Ok(ph)) = (parts[1].parse::<f32>(), parts[3].parse::<f32>())
                    {
                        let is_retina = trimmed.contains("Retina");
                        let scale = if is_retina { 2.0 } else { 1.0 };
                        return (pw / scale * 0.5, ph / scale * 0.5);
                    }
                }
            }
        }
    }
    (1600.0, 1000.0)
}

// ── Native entry point ───────────────────────────────
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    env_logger::init();

    // Query primary monitor size via macOS Core Graphics and use 80% for the initial window.
    // Falls back to 1600×1000 on other platforms or errors.
    let (w, h) = get_screen_size_80pct();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([w, h])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Bad Piggies Editor",
        options,
        Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
    )
}

// ── WASM entry point ─────────────────────────────────
#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;

    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("failed to get document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("failed to find canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
            )
            .await;

        // Remove loading text and show error on failure
        if let Some(loading) = document.get_element_by_id("loading_text") {
            loading.remove();
        }

        if let Err(e) = start_result {
            if let Some(body) = document.body() {
                body.set_inner_html(&format!("<p style='color:red'>应用启动失败: {e:?}</p>"));
            }
        }
    });
}
