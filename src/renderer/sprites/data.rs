//! Sprite draw data structures and builder.

use eframe::egui;

use crate::data::assets;
use crate::data::sprite_db;
use crate::domain::types::*;
use crate::goal_animation::{GoalAnimationState, parse_goal_animation_state};

use super::{bird_sleep_duration, dessert_y_offset};

const GOAL_AREA_HALF_WIDTH: f32 = 1.328125 * 0.5;
const GOAL_AREA_HALF_HEIGHT: f32 = 2.65625 * 0.5;

pub struct SpriteDrawData {
    /// World position.
    pub world_pos: Vec3,
    /// Display color.
    pub color: egui::Color32,
    /// Half-width and half-height in world units.
    pub half_size: (f32, f32),
    /// Instance scale (signed, for flip detection).
    pub scale: (f32, f32),
    /// Object name for labeling.
    pub name: String,
    /// Object index in the level arena.
    pub index: ObjectIndex,
    /// Whether this is a terrain object (skip — terrain renders separately).
    pub is_terrain: bool,
    /// Atlas filename if sprite was found in database.
    pub atlas: Option<String>,
    /// UV rect if sprite was found in database.
    pub uv: Option<sprite_db::UvRect>,
    /// Whether this sprite is hidden (not rendered and not hit-testable unless selected).
    pub is_hidden: bool,
    /// Parent container index (for showing siblings when parent is selected).
    pub parent: Option<ObjectIndex>,
    /// Raw override text for compound rendering (Bridge/Fan parsing).
    pub override_text: Option<String>,
    /// Z-axis rotation in radians.
    pub rotation: f32,
    /// Bird sleep animation phase offset (random per bird, 0..clip duration).
    pub bird_phase: f32,
    /// Pre-computed lowercase name (avoids per-frame String allocation).
    pub name_lower: String,
    /// GoalArea animation mode derived from override data.
    pub goal_animation_state: GoalAnimationState,
}

/// Build sprite draw data for a prefab instance.
/// `name_override` is the resolved name from level-refs (if different from `prefab.name`).
pub fn build_sprite(
    prefab: &PrefabInstance,
    world_pos: Vec3,
    index: ObjectIndex,
    name_override: Option<&str>,
) -> SpriteDrawData {
    let is_terrain = prefab.terrain_data.is_some();
    let sprite_name = name_override.unwrap_or(&prefab.name);

    // Dessert picker: resolve DessertPlace to a specific dessert variant
    let dessert_resolved;
    let is_dessert_place = sprite_name.contains("DessertPlace") || sprite_name.contains("Desserts");
    let sprite_name = if is_dessert_place {
        let hash = (index as u32).wrapping_mul(2654435761);
        let is_golden = hash % 100 == 50;
        dessert_resolved = if is_golden {
            "GoldenCake".to_string()
        } else {
            const REGULAR: &[&str] = &[
                "StrawberryCake",
                "Cupcake",
                "VanillaCakeSlice",
                "CreamyBun",
                "IcecreamBalls",
            ];
            REGULAR[(hash as usize) % REGULAR.len()].to_string()
        };
        dessert_resolved.as_str()
    } else {
        sprite_name
    };

    // DessertPlace Y offset: shift along local up (accounting for rotation)
    let world_pos = if is_dessert_place {
        let y_off = dessert_y_offset(sprite_name);
        let rot_z = prefab.rotation.z.to_radians();
        Vec3 {
            x: world_pos.x - rot_z.sin() * y_off,
            y: world_pos.y + rot_z.cos() * y_off,
            z: world_pos.z,
        }
    } else {
        world_pos
    };

    let color = assets::get_object_color(sprite_name, prefab.prefab_index);

    let sx = prefab.scale.x.abs().max(0.01);
    let sy = prefab.scale.y.abs().max(0.01);

    // Try to get real sprite size from database
    let sprite_info = sprite_db::get_sprite_info(sprite_name);
    let (half_w, half_h, atlas, uv) = if let Some(info) = sprite_info {
        // world_w/world_h are half-extents; scale by instance scale
        (
            info.world_w * sx,
            info.world_h * sy,
            Some(info.atlas.clone()),
            Some(info.uv),
        )
    } else if sprite_name.starts_with("GoalArea") {
        (
            GOAL_AREA_HALF_WIDTH * sx,
            GOAL_AREA_HALF_HEIGHT * sy,
            None,
            None,
        )
    } else {
        // Fallback: 0.3 world units half-extent
        (0.3 * sx, 0.3 * sy, None, None)
    };

    let has_atlas = atlas.is_some();

    SpriteDrawData {
        world_pos,
        color,
        half_size: (half_w, half_h),
        scale: (prefab.scale.x, prefab.scale.y),
        name: sprite_name.to_string(),
        index,
        is_terrain,
        atlas,
        uv,
        is_hidden: is_dessert_place
            || sprite_name.starts_with("WindArea")
            || (!has_atlas && assets::should_skip_render(sprite_name)),
        parent: prefab.parent,
        override_text: prefab.override_data.as_ref().map(|od| od.raw_text.clone()),
        rotation: prefab.rotation.z.to_radians(),
        bird_phase: if sprite_name.starts_with("Bird_") && !sprite_name.starts_with("BirdCompass") {
            // Deterministic random phase per bird based on position
            let seed = (world_pos.x * 1000.0) as u32 ^ (world_pos.y * 1000.0) as u32;
            let duration_ms = (bird_sleep_duration() * 1000.0).round().max(1.0) as u32;
            (seed % duration_ms) as f32 / 1000.0
        } else {
            0.0
        },
        name_lower: sprite_name.to_ascii_lowercase(),
        goal_animation_state: parse_goal_animation_state(
            prefab.override_data.as_ref().map(|od| od.raw_text.as_str()),
        ),
    }
}

/// Options for `draw_sprite`.
pub struct SpriteDrawOpts {
    pub is_selected: bool,
    pub time: f64,
    pub tex_id: Option<egui::TextureId>,
    pub atlas_size: Option<[usize; 2]>,
    pub fan_angle: Option<f32>,
    pub opaque_rendered: bool,
}
