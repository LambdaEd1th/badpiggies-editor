#![forbid(unsafe_code)]

//! The original level renderer, running directly on wgpu without a GUI toolkit.

#[cfg(target_arch = "wasm32")]
mod gpu2d;

#[cfg(target_arch = "wasm32")]
mod contraption_preview;

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
mod renderer;

#[cfg(target_arch = "wasm32")]
pub mod domain {
    pub use badpiggies_editor_core::domain::*;
}

#[cfg(target_arch = "wasm32")]
pub mod unity_runtime {
    pub use badpiggies_editor_core::unity_runtime::*;
}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
pub mod i18n {
    pub mod locale;
}

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::RendererHandle;
