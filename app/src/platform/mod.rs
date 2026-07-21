pub mod log_buffer;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_renderer;
mod preferences;
pub mod processing;
pub mod runtime_assets;
#[cfg(target_arch = "wasm32")]
pub mod startup;
mod task;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use preferences::{
    DESKTOP_WINDOW_MIN_HEIGHT, DESKTOP_WINDOW_MIN_WIDTH, read_window_size_preference,
    save_window_size_preference,
};
pub use preferences::{read_theme_preference, save_theme_preference};
pub use task::sleep_ms;
