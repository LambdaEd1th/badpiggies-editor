//! Background layer caches and GPU state types.

use std::collections::{HashMap, HashSet};

use std::sync::Arc;

use crate::data::bg_data::{self};

use super::super::bg_shader;

pub struct BgLayerCache {
    /// Sprite index → (block_width, parallax_speed) for tiled sprite groups.
    pub(super) tile_info: HashMap<usize, (f32, f32)>,
    /// Sprite index → seam phase offset for tiled sprite groups.
    pub(super) tile_phase: HashMap<usize, f32>,
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

const MAX_MULTI_NAME_TILE_Y_SPAN: f32 = 10.0;
const MAYA_HIGH_NEAR_CORE_MAX_LOCAL_X: f32 = 90.0;
const MAYA_HIGH_FURTHER_CORE_MIN_LOCAL_X: f32 = -58.5;
const MAYA_HIGH_FURTHER_CORE_MAX_LOCAL_X: f32 = 15.0;
const MAYA_HIGH_FURTHER_CORE_BLOCK_WIDTH: f32 = 77.4;
const MAYA_TEMPLE_PATTERN_CLUSTER_X_THRESHOLD: f32 = 4.0;
const MAYA_TEMPLE_NEAR_BOTTOM_SEAM_BIAS: f32 = 1.5;

fn is_maya_high_near_core_sprite(sprite: &bg_data::BgSprite) -> bool {
    sprite.parent_group == "BGLayerNear"
        && sprite.name == "Background_Maya_High_Near"
        && sprite.local_x <= MAYA_HIGH_NEAR_CORE_MAX_LOCAL_X
}

fn is_maya_high_further_core_sprite(sprite: &bg_data::BgSprite) -> bool {
    sprite.parent_group == "BGLayerFurther"
        && sprite.name.starts_with("Background_Maya_High_Further_")
        && !sprite.name.contains("Fill")
        && sprite.local_x >= MAYA_HIGH_FURTHER_CORE_MIN_LOCAL_X
        && sprite.local_x <= MAYA_HIGH_FURTHER_CORE_MAX_LOCAL_X
}

fn forced_tile_group_key(theme_name: &str, sprite: &bg_data::BgSprite) -> Option<String> {
    match (theme_name, sprite.parent_group.as_str()) {
        ("MayaHigh", "BGLayerNear") if is_maya_high_near_core_sprite(sprite) => Some(format!(
            "forced:g:{}:{}:core8",
            sprite.parent_group,
            sprite.layer.order()
        )),
        ("MayaHigh", "BGLayerFurther") if is_maya_high_further_core_sprite(sprite) => {
            Some(format!(
                "forced:g:{}:{}:core77",
                sprite.parent_group,
                sprite.layer.order()
            ))
        }
        ("MayaTemple", "BGLayerNearBottom") => Some(format!(
            "forced:g:{}:{}:combined",
            sprite.parent_group,
            sprite.layer.order()
        )),
        _ => None,
    }
}

fn parent_group_z_key(sprite: &bg_data::BgSprite) -> i32 {
    // Use 0.5-unit Z granularity so 5.5 and 6.0 stay distinct.
    (sprite.world_z * 2.0).round() as i32
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
    let last_index = sorted.last().copied()?;
    let last = &sprites[last_index];
    let min_left = first.world_x - sprite_display_width(first) * 0.5;
    let max_right = last.world_x + sprite_display_width(last) * 0.5;
    Some(max_right - min_left + median_edge_gap)
}

fn normalize_phase_offset(mut phase: f32, block_width: f32) -> f32 {
    while phase <= -block_width * 0.5 {
        phase += block_width;
    }
    while phase > block_width * 0.5 {
        phase -= block_width;
    }
    phase
}

fn widest_gap_phase_from_bounds(bounds: &[(f32, f32)], block_width: f32) -> Option<f32> {
    if bounds.len() < 2 {
        return None;
    }

    let mut widest_gap = None;
    for pair in bounds.windows(2) {
        let gap_start = pair[0].1;
        let gap_end = pair[1].0;
        let gap = gap_end - gap_start;
        if widest_gap.is_none_or(|(best_gap, _, _)| gap > best_gap) {
            widest_gap = Some((gap, gap_start, gap_end));
        }
    }

    let (_, gap_start, gap_end) = widest_gap?;
    let desired_center = (gap_start + gap_end) * 0.5;
    let min_left = bounds.first()?.0;
    let max_right = bounds.last()?.1;
    let seam_gap = block_width - (max_right - min_left);
    let current_center = max_right + seam_gap * 0.5;
    Some(normalize_phase_offset(
        desired_center - current_center,
        block_width,
    ))
}

fn tile_block_width_clustered(
    sorted: &[usize],
    sprites: &[bg_data::BgSprite],
    x_cluster_threshold: f32,
) -> Option<f32> {
    let clusters = clustered_bounds(sorted, sprites, x_cluster_threshold);

    if clusters.len() < 2 {
        return None;
    }

    let mut gaps: Vec<f32> = clusters
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].1)
        .collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_gap = gaps[gaps.len() / 2];
    let min_left = clusters.first()?.0;
    let max_right = clusters.last()?.1;
    Some(max_right - min_left + median_gap)
}

fn clustered_bounds(
    sorted: &[usize],
    sprites: &[bg_data::BgSprite],
    x_cluster_threshold: f32,
) -> Vec<(f32, f32)> {
    let Some(first) = sorted.first().copied() else {
        return Vec::new();
    };
    let first_sprite = &sprites[first];
    let mut clusters = vec![(
        first_sprite.world_x - sprite_display_width(first_sprite) * 0.5,
        first_sprite.world_x + sprite_display_width(first_sprite) * 0.5,
    )];
    let mut prev_x = first_sprite.world_x;

    for &idx in &sorted[1..] {
        let sprite = &sprites[idx];
        let left = sprite.world_x - sprite_display_width(sprite) * 0.5;
        let right = sprite.world_x + sprite_display_width(sprite) * 0.5;

        if sprite.world_x - prev_x <= x_cluster_threshold {
            if let Some((cluster_left, cluster_right)) = clusters.last_mut() {
                *cluster_left = cluster_left.min(left);
                *cluster_right = cluster_right.max(right);
            }
        } else {
            clusters.push((left, right));
        }
        prev_x = sprite.world_x;
    }

    clusters
}

fn forced_tile_block_width(
    theme_name: &str,
    group_key: &str,
    sorted: &[usize],
    sprites: &[bg_data::BgSprite],
) -> Option<f32> {
    if theme_name == "MayaHigh"
        && group_key
            == format!(
                "forced:g:BGLayerFurther:{}:core77",
                bg_data::BgLayer::Further.order()
            )
    {
        return Some(MAYA_HIGH_FURTHER_CORE_BLOCK_WIDTH);
    }

    if theme_name == "MayaTemple"
        && group_key == format!(
            "forced:g:BGLayerNearBottom:{}:combined",
            bg_data::BgLayer::Near.order()
        )
    {
        return tile_block_width_clustered(sorted, sprites, MAYA_TEMPLE_PATTERN_CLUSTER_X_THRESHOLD)
            .or_else(|| tile_block_width(sorted, sprites));
    }

    None
}

fn forced_tile_phase_offset(
    theme_name: &str,
    group_key: &str,
    sorted: &[usize],
    sprites: &[bg_data::BgSprite],
    block_width: f32,
) -> Option<f32> {
    if theme_name == "MayaHigh"
        && group_key
            == format!(
                "forced:g:BGLayerFurther:{}:core77",
                bg_data::BgLayer::Further.order()
            )
    {
        return Some(0.0);
    }

    if theme_name == "MayaTemple"
        && group_key == format!(
            "forced:g:BGLayerNearBottom:{}:combined",
            bg_data::BgLayer::Near.order()
        )
    {
        return widest_gap_phase_from_bounds(
            &clustered_bounds(sorted, sprites, MAYA_TEMPLE_PATTERN_CLUSTER_X_THRESHOLD),
            block_width,
        )
        .map(|phase| normalize_phase_offset(phase + MAYA_TEMPLE_NEAR_BOTTOM_SEAM_BIAS, block_width));
    }

    None
}

pub(super) fn tile_group_key(
    sprite: &bg_data::BgSprite,
    name_lower: &str,
    distinct_name_count: usize,
    split_by_name: bool,
) -> Option<String> {
    if sprite.fill_color.is_some() || sprite.sky_texture.is_some() || name_lower.contains("fill") {
        return None;
    }

    let layer_key = sprite.layer.order();
    if !sprite.parent_group.is_empty() {
        if distinct_name_count <= 1 {
            Some(format!("g:{}:{}", sprite.parent_group, layer_key))
        } else {
            let z_key = parent_group_z_key(sprite);
            if split_by_name {
                Some(format!(
                    "g:{}:{}:{}:{}",
                    sprite.parent_group, layer_key, z_key, name_lower
                ))
            } else {
                // Multi-name parent groups still need Z separation, but only
                // repeated names within the same Z band split by name. That
                // keeps Halloween's Lamp/Pumpkin and Ocean's Dummy/Waves apart
                // without breaking Morning's one-off foreground tree strip.
                Some(format!("g:{}:{}:{}", sprite.parent_group, layer_key, z_key))
            }
        }
    } else {
        let atlas_key = sprite.atlas.as_deref().unwrap_or("");
        let y_key = sprite.world_y.round() as i32;
        Some(format!("y:{}:{}:{}", y_key, atlas_key, layer_key))
    }
}

pub(super) fn bg_sprite_x_animation_offset(
    _name_lower: &str,
    _time: f64,
    _layer: &bg_data::BgLayer,
) -> f32 {
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
    // Single-name parent groups still group by parent_group+layer only — NO Y
    // or atlas in the key — so that one Unity repeating strip can share one
    // block_width across nearby Z offsets (e.g. Jungle far hills). Multi-name
    // groups fall back to 0.5-Z bands, and only split by name inside a Z band
    // when repeated names would otherwise be merged incorrectly.
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    let mut parent_group_names: HashMap<(String, i32), HashSet<String>> = HashMap::new();
    let mut parent_group_z_name_counts: HashMap<(String, i32, i32), HashMap<String, usize>> =
        HashMap::new();
    let mut parent_group_z_y_extents: HashMap<(String, i32, i32), (f32, f32)> = HashMap::new();
    let mut parent_group_z_name_y_extents: HashMap<(String, i32, i32, String), (f32, f32)> =
        HashMap::new();
    let mut name_lower: Vec<String> = Vec::with_capacity(sprites.len());
    for (i, sprite) in sprites.iter().enumerate() {
        let nl = sprite.name.to_ascii_lowercase();
        name_lower.push(nl);
        if !sprite.parent_group.is_empty()
            && sprite.fill_color.is_none()
            && sprite.sky_texture.is_none()
            && !name_lower[i].contains("fill")
        {
            let parent_group = sprite.parent_group.clone();
            let layer_key = sprite.layer.order();
            parent_group_names
                .entry((parent_group.clone(), layer_key))
                .or_default()
                .insert(name_lower[i].clone());
            *parent_group_z_name_counts
                .entry((parent_group, layer_key, parent_group_z_key(sprite)))
                .or_default()
                .entry(name_lower[i].clone())
                .or_default() += 1;
            let y_extents = parent_group_z_y_extents
                .entry((sprite.parent_group.clone(), layer_key, parent_group_z_key(sprite)))
                .or_insert((sprite.world_y, sprite.world_y));
            y_extents.0 = y_extents.0.min(sprite.world_y);
            y_extents.1 = y_extents.1.max(sprite.world_y);
            let name_y_extents = parent_group_z_name_y_extents
                .entry((
                    sprite.parent_group.clone(),
                    layer_key,
                    parent_group_z_key(sprite),
                    name_lower[i].clone(),
                ))
                .or_insert((sprite.world_y, sprite.world_y));
            name_y_extents.0 = name_y_extents.0.min(sprite.world_y);
            name_y_extents.1 = name_y_extents.1.max(sprite.world_y);
        }
    }

    for (i, sprite) in sprites.iter().enumerate() {
        if sprite.fill_color.is_some() || sprite.sky_texture.is_some() {
            continue;
        }
        if name_lower[i].contains("fill") {
            continue;
        }
        if theme_name == "MayaHigh"
            && sprite.parent_group == "BGLayerNear"
            && sprite.name == "Background_Maya_High_Near"
            && !is_maya_high_near_core_sprite(sprite)
        {
            continue;
        }
        if theme_name == "MayaHigh"
            && sprite.parent_group == "BGLayerFurther"
            && !is_maya_high_further_core_sprite(sprite)
        {
            continue;
        }
        if let Some(group_key) = forced_tile_group_key(theme_name, sprite) {
            groups.entry(group_key).or_default().push(i);
            continue;
        }
        let (distinct_name_count, split_by_name) = if sprite.parent_group.is_empty() {
            (0, false)
        } else {
            let layer_key = sprite.layer.order();
            let distinct_name_count = parent_group_names
                .get(&(sprite.parent_group.clone(), layer_key))
                .map(HashSet::len)
                .unwrap_or(0);
            let split_by_name = parent_group_z_name_counts
                .get(&(
                    sprite.parent_group.clone(),
                    layer_key,
                    parent_group_z_key(sprite),
                ))
                .map(|counts| counts.len() > 1 && counts.values().any(|count| *count > 1))
                .unwrap_or(false);
            (distinct_name_count, split_by_name)
        };
        if distinct_name_count > 1 {
            let layer_key = sprite.layer.order();
            let z_key = parent_group_z_key(sprite);
            let y_span = if split_by_name {
                parent_group_z_name_y_extents
                    .get(&(
                        sprite.parent_group.clone(),
                        layer_key,
                        z_key,
                        name_lower[i].clone(),
                    ))
                    .map(|(min_y, max_y)| max_y - min_y)
                    .unwrap_or(0.0)
            } else {
                parent_group_z_y_extents
                    .get(&(sprite.parent_group.clone(), layer_key, z_key))
                    .map(|(min_y, max_y)| max_y - min_y)
                    .unwrap_or(0.0)
            };
            if y_span > MAX_MULTI_NAME_TILE_Y_SPAN {
                continue;
            }
        }
        let Some(group_key) =
            tile_group_key(sprite, &name_lower[i], distinct_name_count, split_by_name)
        else {
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
    let mut tile_phase: HashMap<usize, f32> = HashMap::new();
    for (group_key, indices) in &groups {
        if indices.len() < 2 {
            continue;
        }
        let mut sorted: Vec<usize> = indices.clone();
        sorted.sort_by(|a, b| sprites[*a].world_x.total_cmp(&sprites[*b].world_x));
        let Some(block_width) = forced_tile_block_width(theme_name, group_key, &sorted, sprites)
            .or_else(|| tile_block_width(&sorted, sprites))
        else {
            continue;
        };
        let speed = sprites[sorted[0]].layer.parallax_speed();
        let phase = forced_tile_phase_offset(theme_name, group_key, &sorted, sprites, block_width)
            .unwrap_or(0.0);
        for &idx in &sorted {
            tile_info.insert(idx, (block_width, speed));
            tile_phase.insert(idx, phase);
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
        tile_phase,
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
