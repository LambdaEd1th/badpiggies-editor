//! Embedded static data: textures, sprite/level/icon/background tables.

pub mod assets;
pub mod bg_data;
pub mod goal_animation;
pub mod icon_db;
pub mod level_db;
pub mod prefab_sprites;
pub mod runtime_assets;
pub mod sprite_db;
pub mod unity_anim;
pub mod unity_particles;

/// Prepare archive and lookup structures used by the level renderer.
///
/// Web render workers call this after becoming ready so the expensive archive
/// scan happens while the empty workspace is visible instead of after a file is
/// selected.
pub fn prepare_renderer_assets() {
    assets::prepare_runtime_asset_index();
    crate::domain::level::refs::prepare_level_lookup_tables();
}
