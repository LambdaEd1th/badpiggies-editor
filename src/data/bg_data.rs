//! Background theme data — parses embedded Unity background prefabs at runtime.
//!
//! Each theme has multiple sprite entries organized into parallax layers.

mod overrides;
mod parse;
mod tables;
mod theme;
mod types;

use std::collections::HashMap;
use std::sync::OnceLock;

pub use overrides::{apply_bg_overrides, parse_bg_overrides, parse_position_serializer_overrides};
pub use tables::{bg_atlas_files, sky_texture_files};
pub use theme::get_theme;
pub use types::{BgLayer, BgSprite, BgTheme};

pub fn atlas_for_material_guid(material_guid: &str) -> Option<&'static str> {
	static ATLAS_BY_GUID: OnceLock<HashMap<String, String>> = OnceLock::new();

	let prefix = parse::guid_prefix(material_guid);
	let atlas_by_guid = ATLAS_BY_GUID.get_or_init(|| {
		parse::load_textureloader_materials()
			.into_iter()
			.map(|(guid, asset_name)| (guid, parse::asset_filename(&asset_name)))
			.collect()
	});

	atlas_by_guid
		.get(prefix)
		.map(String::as_str)
		.or_else(|| tables::supplemental_atlas_for_material(prefix))
}

#[cfg(test)]
mod tests;
