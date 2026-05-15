use std::sync::Arc;

use super::particles::{reset_fan_emitter_for_build, start_fan_emitter_for_play};
use super::{LevelRenderer, PreviewPlaybackState, sprite_shader};

impl LevelRenderer {
    pub fn preview_playback_state(&self) -> PreviewPlaybackState {
        self.preview_playback_state
    }

    pub fn set_preview_playback_state(&mut self, state: PreviewPlaybackState) {
        if self.preview_playback_state == state {
            return;
        }

        let was_build = self.preview_playback_state == PreviewPlaybackState::Build;
        match state {
            PreviewPlaybackState::Build => self.reset_runtime_preview(),
            PreviewPlaybackState::Play if was_build => self.start_runtime_preview(),
            PreviewPlaybackState::Play | PreviewPlaybackState::Pause => {}
        }

        self.preview_playback_state = state;
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
