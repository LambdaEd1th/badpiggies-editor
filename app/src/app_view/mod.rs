pub mod canvas;

use dioxus::prelude::*;

use crate::components::EditorShell;
use crate::editor_state::{CanvasPointerState, EditorState};

#[cfg(target_arch = "wasm32")]
const APP_CSS: Asset = asset!(
    "/assets/dioxus.css",
    AssetOptions::css().with_static_head(true)
);
#[cfg(not(target_arch = "wasm32"))]
const APP_CSS: &str = "/assets/dioxus.css";

#[cfg(target_arch = "wasm32")]
pub(crate) const APP_ASSETS: Asset = asset!("/assets", AssetOptions::folder());

#[cfg(not(target_arch = "wasm32"))]
fn install_native_asset_handler() {
    use std::path::{Component, Path};

    use dioxus::desktop::wry::http::{Response, StatusCode};

    let root = crate::platform::runtime_assets::runtime_assets_root().ok();
    dioxus::desktop::use_asset_handler("assets", move |request, responder| {
        let relative = request
            .uri()
            .path()
            .strip_prefix("/assets/")
            .unwrap_or_default();
        let is_safe = !relative.is_empty()
            && Path::new(relative)
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        let path = root.as_ref().filter(|_| is_safe).map(|root| {
            crate::platform::runtime_assets::runtime_asset_path(root.as_path(), relative)
        });
        let response = path
            .and_then(|path| {
                std::fs::read(&path).ok().map(|body| {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", native_asset_content_type(&path))
                        .header("Access-Control-Allow-Origin", "*")
                        .body(body)
                        .expect("native asset response must be valid")
                })
            })
            .unwrap_or_else(|| {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(b"Not Found".to_vec())
                    .expect("native 404 response must be valid")
            });
        responder.respond(response);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn native_asset_content_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("wgsl" | "ftl" | "txt") => "text/plain; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        _ => "application/octet-stream",
    }
}

#[allow(non_snake_case)]
pub fn App() -> Element {
    #[cfg(not(target_arch = "wasm32"))]
    install_native_asset_handler();
    let _editor_state = use_context_provider(|| Signal::new(EditorState::default()));
    use_context_provider(|| Signal::new(CanvasPointerState::default()));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let renderer = crate::platform::native_renderer::use_native_renderer();
        use_context_provider(|| renderer.clone());
        let appearance_renderer = renderer.clone();
        use_effect(move || {
            appearance_renderer.set_theme(_editor_state.read().theme);
        });
        use_effect(|| {
            document::eval("document.documentElement.classList.add('native-wgpu-host');");
        });
    }
    #[cfg(target_arch = "wasm32")]
    use_effect(crate::platform::startup::finish);
    rsx! {
        document::Title { "Bad Piggies Editor" }
        document::Link { rel: "stylesheet", href: APP_CSS }
        EditorShell {}
    }
}
