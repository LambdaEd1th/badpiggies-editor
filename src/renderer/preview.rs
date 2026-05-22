use std::sync::Arc;

use crate::domain::types::LevelData;

use super::particles::{reset_fan_emitter_for_build, start_fan_emitter_for_play};
use super::{LevelRenderer, PreviewPlaybackState, sprite_shader};

impl LevelRenderer {
    fn sync_night_vision_state(&mut self) {
        self.contraption_has_night_vision = self.dark_level;
        if !self.dark_level {
            self.night_vision_enabled = false;
        }
    }

    pub fn preview_playback_state(&self) -> PreviewPlaybackState {
        self.preview_playback_state
    }

    pub fn reload_level_preserving_preview_state(&mut self, level: &LevelData) {
        let camera = self.camera.clone();
        let preview_playback_state = self.preview_playback_state;
        let was_dark_level = self.dark_level;
        let night_vision_enabled = self.night_vision_enabled;

        self.set_level(level);
        self.camera = camera;
        self.preview_playback_state = preview_playback_state;
        if was_dark_level && self.dark_level {
            self.night_vision_enabled = night_vision_enabled;
        }
        self.sync_night_vision_state();

        if preview_playback_state == PreviewPlaybackState::Build {
            self.reset_runtime_preview();
        }
    }

    pub fn set_preview_playback_state(&mut self, state: PreviewPlaybackState) {
        if self.preview_playback_state == state {
            return;
        }

        let was_build = self.preview_playback_state == PreviewPlaybackState::Build;
        match state {
            PreviewPlaybackState::Build => {
                self.reset_runtime_preview();
            }
            PreviewPlaybackState::Play | PreviewPlaybackState::Pause if was_build => {
                self.start_runtime_preview();
            }
            PreviewPlaybackState::Play | PreviewPlaybackState::Pause => {}
        }

        self.preview_playback_state = state;
        self.sync_night_vision_state();
    }

    fn reset_runtime_preview(&mut self) {
        for emitter in &mut self.fan_emitters {
            reset_fan_emitter_for_build(emitter);
        }
        self.fan_particles.clear();
        self.wind_particles.clear();
        self.wind_spawn_accum = vec![
            0.0;
            self.wind_areas.len() * crate::renderer::particles::wind_area_particle_system_count()
        ];
        self.zzz_particles.clear();
        self.zzz_emit_accum = vec![0.0; self.bird_positions.len()];
        self.attached_effect_particles.clear();
        for emitter in &mut self.attached_effect_emitters {
            emitter.system_time.fill(0.0);
            emitter.spawn_accum.fill(0.0);
        }
    }

    pub(super) fn start_runtime_preview(&mut self) {
        for emitter in &mut self.fan_emitters {
            start_fan_emitter_for_play(emitter);
        }
        self.fan_particles.clear();
        self.zzz_particles.clear();
        self.zzz_emit_accum = vec![0.0; self.bird_positions.len()];
        self.seed_attached_effect_particles();
        self.seed_wind_particles();
    }

    /// Set the level-refs key (derived from filename) for prefab name overrides.
    pub fn set_level_key(&mut self, filename: &str) {
        self.level_key = crate::domain::level::refs::level_key_from_filename(filename);
    }

    /// Whether the current level is a dark level.
    pub fn is_dark_level(&self) -> bool {
        self.dark_level
    }

    /// Whether the night-vision dark overlay variant is enabled.
    pub fn night_vision_enabled(&self) -> bool {
        self.night_vision_enabled
    }

    /// Toggle the night-vision dark overlay variant for dark levels.
    pub fn set_night_vision_enabled(&mut self, enabled: bool) {
        self.night_vision_enabled = enabled && self.dark_level;
        self.sync_night_vision_state();
    }

    /// Shared transparent sprite shader resources, if the current backend has wgpu.
    pub fn preview_sprite_resources(&self) -> Option<Arc<sprite_shader::SpriteResources>> {
        self.sprite_resources.clone()
    }

    /// Load or fetch a GPU sprite atlas for save preview rendering.
    pub fn preview_sprite_atlas(
        &mut self,
        filename: &str,
    ) -> Option<Arc<sprite_shader::SpriteAtlasGpu>> {
        let (Some(resources), Some(device), Some(queue)) = (
            self.sprite_resources.as_ref(),
            self.wgpu_device.as_ref(),
            self.wgpu_queue.as_ref(),
        ) else {
            return None;
        };
        self.sprite_atlas_cache
            .get_or_load(device, queue, resources, filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{
        DataType, LevelData, LevelObject, PrefabInstance, PrefabOverrideData, Vec3,
    };

    const LEVEL_MANAGER_DARK_OVERRIDE: &str =
        "GameObject LevelManager\n\tComponent LevelManager\n\t\tBoolean m_darkLevel = True\n";

    fn dark_level() -> LevelData {
        LevelData {
            objects: vec![LevelObject::Prefab(PrefabInstance {
                name: "LevelManager".to_string(),
                position: Vec3::default(),
                prefab_index: 0,
                rotation: Vec3::default(),
                scale: Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
                data_type: DataType::PrefabOverrides,
                terrain_data: None,
                override_data: Some(PrefabOverrideData {
                    raw_text: LEVEL_MANAGER_DARK_OVERRIDE.to_string(),
                    raw_bytes: Vec::new(),
                }),
                parent: None,
            })],
            roots: vec![0],
        }
    }

    #[test]
    fn dark_levels_keep_night_vision_enabled_in_build_and_runtime() {
        let level = dark_level();
        let mut renderer = LevelRenderer::new(None);
        renderer.set_level(&level);

        assert!(renderer.contraption_has_night_vision);
        assert!(renderer.night_vision_enabled);

        renderer.set_preview_playback_state(PreviewPlaybackState::Play);
        assert!(renderer.contraption_has_night_vision);
        assert!(renderer.night_vision_enabled);

        renderer.set_preview_playback_state(PreviewPlaybackState::Build);
        assert!(renderer.contraption_has_night_vision);
        assert!(renderer.night_vision_enabled);
    }

    #[test]
    fn dark_level_night_vision_toggle_persists_across_preview_states() {
        let level = dark_level();
        let mut renderer = LevelRenderer::new(None);
        renderer.set_level(&level);

        renderer.set_night_vision_enabled(false);
        assert!(renderer.contraption_has_night_vision);
        assert!(!renderer.night_vision_enabled);

        renderer.set_preview_playback_state(PreviewPlaybackState::Play);
        assert!(renderer.contraption_has_night_vision);
        assert!(!renderer.night_vision_enabled);

        renderer.set_preview_playback_state(PreviewPlaybackState::Build);
        assert!(renderer.contraption_has_night_vision);
        assert!(!renderer.night_vision_enabled);

        renderer.set_night_vision_enabled(true);
        assert!(renderer.night_vision_enabled);
    }

    #[test]
    fn non_dark_levels_keep_night_vision_disabled() {
        let mut renderer = LevelRenderer::new(None);

        renderer.set_preview_playback_state(PreviewPlaybackState::Play);

        assert!(!renderer.contraption_has_night_vision);
        assert!(!renderer.night_vision_enabled);
    }

    #[test]
    fn reload_preserves_dark_level_night_vision_toggle_and_preview_state() {
        let level = dark_level();
        let mut renderer = LevelRenderer::new(None);
        renderer.set_level(&level);
        renderer.preview_playback_state = PreviewPlaybackState::Pause;
        renderer.set_night_vision_enabled(false);

        renderer.reload_level_preserving_preview_state(&level);

        assert_eq!(renderer.preview_playback_state, PreviewPlaybackState::Pause);
        assert!(renderer.contraption_has_night_vision);
        assert!(!renderer.night_vision_enabled);
    }
}
