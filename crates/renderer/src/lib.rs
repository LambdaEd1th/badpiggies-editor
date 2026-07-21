#![forbid(unsafe_code)]

//! The original level renderer, running directly on wgpu without a GUI toolkit.

mod gpu2d;

mod contraption_preview;

#[allow(dead_code)]
mod renderer;

mod engine;

#[cfg(test)]
mod test_support;

#[cfg(not(target_arch = "wasm32"))]
mod native_text;

pub mod domain {
    pub use badpiggies_editor_core::domain::*;
}

pub mod unity_runtime {
    pub use badpiggies_editor_core::unity_runtime::*;
}

pub mod data {
    pub use badpiggies_editor_core::data::{
        bg_data, goal_animation, icon_db, level_db, prefab_sprites, runtime_assets, sprite_db,
        unity_anim, unity_particles,
    };

    pub mod assets {
        pub use badpiggies_editor_core::data::assets::*;

        mod texture_cache;
        mod theme;

        pub use texture_cache::TextureCache;
        pub use theme::{
            detect_bg_theme_with_dark_level, get_object_color, ground_color,
            props_tint_color_for_prefab, props_tint_is_alpha_blend, should_skip_render,
            skip_props_tint, sky_top_color,
        };
    }
}

pub mod i18n {
    pub mod locale;
}

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
mod native;

pub use contraption_preview::ContraptionPreviewPayload;
pub use engine::{RendererEvent, ScenePayload, ViewPayload};

#[cfg(target_arch = "wasm32")]
pub use web::RendererHandle;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{NativeFrame, NativeRendererHandle, NativeViewport};
