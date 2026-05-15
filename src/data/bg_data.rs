//! Background theme data — parses embedded Unity background prefabs at runtime.
//!
//! Each theme has multiple sprite entries organized into parallax layers.

mod overrides;
mod parse;
mod tables;
mod theme;
mod types;

pub use overrides::{apply_bg_overrides, parse_bg_overrides, parse_position_serializer_overrides};
pub use tables::{bg_atlas_files, sky_texture_files};
pub use theme::get_theme;
pub use types::{BgLayer, BgSprite, BgTheme};

#[cfg(test)]
mod tests;
