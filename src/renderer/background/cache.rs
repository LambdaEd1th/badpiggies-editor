//! Background layer caches and GPU state types.

use std::collections::{HashMap, HashSet};

use std::sync::Arc;

use crate::data::bg_data::{self};

use super::super::bg_shader;

pub struct BgLayerCache {
    /// Sprite index → (block_width, parallax_speed) for tiled sprite groups.
    pub(super) tile_info: HashMap<usize, (f32, f32)>,
    /// Set of sprite indices that are fill-extended singletons.
    pub(super) singleton_set: HashSet<usize>,
    /// Pre-lowercased sprite names (avoids per-frame String allocation).
    pub(super) name_lower: Vec<String>,
    /// Effective sprites (with overrides applied), or None if using theme defaults.
    effective_sprites: Option<Vec<bg_data::BgSprite>>,
    /// Sprite indices sorted by worldZ descending (farthest first = back-to-front).
    pub(super) sorted_indices: Vec<usize>,
}

impl BgLayerCache {
    /// Get the effective sprite slice (overrides or theme defaults).
    pub fn sprites<'a>(&'a self, theme: &'a bg_data::BgTheme) -> &'a [bg_data::BgSprite] {
        self.effective_sprites.as_deref().unwrap_or(&theme.sprites)
    }
}

pub(super) fn sprite_display_width(sprite: &bg_data::BgSprite) -> f32 {
    sprite.sprite_w * WORLD_SCALE * 2.0 * sprite.scale_x.abs()
}

fn tile_block_width(sorted: &[usize], sprites: &[bg_data::BgSprite]) -> Option<f32> {
    // Use edge-to-edge bounding for the wrap gap: the gap between the last
    // sprite's right edge and the first sprite's left edge in the next copy
    // should equal the median edge gap between adjacent sprites.  This is
    // correct regardless of whether sprites have uniform or varying display
    // widths (e.g. BGLayerNear's first sprite has a smaller scale than the
    // rest; a centre-to-centre formula would produce a 1-world-unit gap at
    // the seam, while edge-based matches the ~0-pixel internal overlap).
    let mut edge_gaps: Vec<f32> = sorted
        .windows(2)
        .map(|pair| {
            let a = &sprites[pair[0]];
            let b = &sprites[pair[1]];
            let a_right = a.world_x + sprite_display_width(a) * 0.5;
            let b_left = b.world_x - sprite_display_width(b) * 0.5;
            b_left - a_right
        })
        .collect();
    if edge_gaps.is_empty() {
        return None;
    }
    edge_gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_edge_gap = edge_gaps[edge_gaps.len() / 2];
    let first = &sprites[sorted[0]];
    let Some(last_index) = sorted.last().copied() else {
        return None;
    };
    let last = &sprites[last_index];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    Some(max_right - min_left + median_edge_gap)
}

pub(super) fn tile_group_key(
    sprite: &bg_data::BgSprite,
    name_lower: &str,
    name_count: usize,
) -> Option<String> {
    if sprite.fill_color.is_some() || sprite.sky_texture.is_some() || name_lower.contains("fill") {
        return None;
    }

    let layer_key = sprite.layer.order();
    if !sprite.parent_group.is_empty() {
        if name_count <= 1 {
            Some(format!("g:{}:{}", sprite.parent_group, layer_key))
        } else {
            // Use 0.5-unit Z granularity: multiply by 2 before rounding.
            // Plain z.round() collapses Z=5.5 and Z=6.0 to the same key (both →
            // 6), merging e.g. Lamp_01 and Background_Plateau_02 in Halloween
            // BGLayerNear and producing the wrong block_width.  Sprites that
            // belong to a single tiling strip (like Morning BGLayerForeground's
            // 14 uniquely-named trees, all at Z=−8.94) still share one key.
            let z_key = (sprite.world_z * 2.0).round() as i32;
            Some(format!("g:{}:{}:{}", sprite.parent_group, layer_key, z_key))
        }
    } else {
        let atlas_key = sprite.atlas.as_deref().unwrap_or("");
        let y_key = sprite.world_y.round() as i32;
        Some(format!("y:{}:{}:{}", y_key, atlas_key, layer_key))
    }
}

pub(super) fn bg_sprite_x_animation_offset(_name_lower: &str, _time: f64, _layer: &bg_data::BgLayer) -> f32 {
    // Unity's background prefab cloud strips are static parallax sprites.
    // Only CloudSet instances animate horizontally at runtime.
    0.0
}

pub fn build_bg_layer_cache(
    theme_name: &str,
    bg_override_text: Option<&str>,
) -> Option<BgLayerCache> {
    let theme = bg_data::get_theme(theme_name)?;

    let effective_sprites = if let Some(raw) = bg_override_text {
        // Try Transform-based overrides first (EP1-5 style)
        let overrides = bg_data::parse_bg_overrides(raw);
        if !overrides.groups.is_empty() || !overrides.sprites.is_empty() {
            Some(bg_data::apply_bg_overrides(theme, &overrides))
        } else if !theme.child_order.is_empty() {
            // Try PositionSerializer-based overrides (EP6 style)
            let overrides = bg_data::parse_position_serializer_overrides(raw, &theme.child_order);
            if !overrides.groups.is_empty() {
                Some(bg_data::apply_bg_overrides(theme, &overrides))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let sprites = effective_sprites.as_deref().unwrap_or(&theme.sprites);

    // Build tile bands: group non-fill, non-sky sprites by parent_group (or
    // fallback to atlas+round(y)+layer when no parent_group is set).
    // When parent_group is set, we group by parent_group+layer only — NO Y or
    // atlas in the key — so that all sprites in the same Unity repeating group
    // share one block_width and tile as a coherent unit (e.g. MayaTemple
    // vertical block columns must tile at the same period as the base strips).
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut parent_group_names: HashMap<(String, i32), HashSet<String>> = HashMap::new();
    let mut name_lower: Vec<String> = Vec::with_capacity(sprites.len());
    for (i, sprite) in sprites.iter().enumerate() {
        let nl = sprite.name.to_ascii_lowercase();
        name_lower.push(nl);
        if !sprite.parent_group.is_empty()
            && sprite.fill_color.is_none()
            && sprite.sky_texture.is_none()
        {
            parent_group_names
                .entry((sprite.parent_group.clone(), sprite.layer.order()))
                .or_default()
                .insert(name_lower[i].clone());
        }
    }

    for (i, sprite) in sprites.iter().enumerate() {
        if sprite.fill_color.is_some() || sprite.sky_texture.is_some() {
            continue;
        }
        if name_lower[i].contains("fill") {
            continue;
        }
        let name_count = if sprite.parent_group.is_empty() {
            0
        } else {
            parent_group_names
                .get(&(sprite.parent_group.clone(), sprite.layer.order()))
                .map(HashSet::len)
                .unwrap_or(0)
        };
        let Some(group_key) = tile_group_key(sprite, &name_lower[i], name_count) else {
            continue;
        };
        groups.entry(group_key).or_default().push(i);
    }

    let mut singleton_set: HashSet<usize> = HashSet::new();
    for indices in groups.values() {
        if indices.len() == 1 {
            let s = &sprites[indices[0]];
            let dw = s.sprite_w * WORLD_SCALE * 2.0 * s.scale_x.abs();
            if dw > 100.0 {
                singleton_set.insert(indices[0]);
            }
        }
    }

    let mut tile_info: HashMap<usize, (f32, f32)> = HashMap::new();
    for indices in groups.values() {
        if indices.len() < 2 {
            continue;
        }
        let mut sorted: Vec<usize> = indices.clone();
        sorted.sort_by(|a, b| sprites[*a].world_x.total_cmp(&sprites[*b].world_x));
        let Some(block_width) = tile_block_width(&sorted, sprites) else {
            continue;
        };
        let speed = sprites[sorted[0]].layer.parallax_speed();
        for &idx in &sorted {
            tile_info.insert(idx, (block_width, speed));
        }
    }

    // Sort sprites by worldZ descending (farthest first = back-to-front),
    // matching Unity's Transparent queue rendering order.
    // Fill sprites (e.g. z=9.9) have slightly lower Z than their companion
    // hills (e.g. z=10.0), so they naturally render AFTER (on top of) the
    // hills — covering the lower portion while hilltops remain visible.
    let s = effective_sprites.as_deref().unwrap_or(&theme.sprites);
    let mut idx: Vec<usize> = (0..s.len()).collect();
    idx.sort_by(|a, b| {
        s[*b]
            .world_z
            .partial_cmp(&s[*a].world_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Some(BgLayerCache {
        tile_info,
        singleton_set,
        name_lower,
        sorted_indices: idx,
        effective_sprites,
    })
}

/// GPU state passed by the renderer for background sprites.
pub struct BgGpuState<'a> {
    pub resources: Arc<bg_shader::BgResources>,
    pub atlas_cache: &'a mut bg_shader::BgAtlasCache,
    pub device: &'a eframe::wgpu::Device,
    pub queue: &'a eframe::wgpu::Queue,
    pub slot_counter: &'a mut u32,
}

/// World-size formula: pixelSize * 10 / 768
pub(super) const WORLD_SCALE: f32 = 10.0 / 768.0;
