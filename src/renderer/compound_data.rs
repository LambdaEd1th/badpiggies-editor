//! Compound prefab sprite data — UV rects, sizes, offsets.
//!
//! Every constant in this module is derived at first access from the embedded
//! Unity prefab assets (`Assets/Prefab/*.prefab` + `sprites.bytes` +
//! `spritemapping.bytes` + atlas-material guid mapping). There is no
//! hand-extracted data here — if a prefab changes upstream and the embedded
//! assets are refreshed, the rendered values update automatically.
//!
//! All values use `LazyLock`, so each prefab is parsed at most once on first
//! access. The cost is paid lazily on first use of each compound and never
//! again. A smoke test in `prefab_derivation_tests` forces every constant to
//! initialize so any missing prefab / malformed asset surfaces immediately.

use std::sync::LazyLock;

use crate::data::sprite_db::UvRect;

mod derive;

pub(super) const SCALE: f32 = 10.0 / 768.0;

pub(super) struct SubSprite {
    pub atlas: &'static str,
    pub uv: UvRect,
    pub world_w: f32,
    pub world_h: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub flip_x: bool,
    pub flip_y: bool,
}

// ─── Fan ────────────────────────────────────────────────────────────────

pub(super) static FAN_PROPELLER: LazyLock<SubSprite> =
    LazyLock::new(|| derive::derive_by_name(&derive::load_prefab("Fan"), "Fan_Sprite"));

pub(super) static FAN_ENGINE: LazyLock<SubSprite> =
    LazyLock::new(|| derive::derive_by_name(&derive::load_prefab("Fan"), "Engine_Sprite"));

pub(super) static FAN_FRAME: LazyLock<SubSprite> =
    LazyLock::new(|| derive::derive_by_name(&derive::load_prefab("Fan"), "Frame_Sprite"));

// ─── Bird face sprites ──────────────────────────────────────────────────

pub(super) struct BirdFaceData {
    pub name_prefix: &'static str,
    pub atlas: &'static str,
    pub uv: UvRect,
    pub world_w: f32,
    pub world_h: f32,
    /// Face offset = (face_cumulative_pos − body_cumulative_pos)
    /// + (face_pivot − body_pivot) × PIVOT_WORLD.
    ///
    /// The pivot correction comes from the runtime atlas data (`sprites.bytes`).
    pub offset_x: f32,
    pub offset_y: f32,
}

pub(super) static BIRD_FACES: LazyLock<Vec<BirdFaceData>> = LazyLock::new(|| {
    [
        ("Bird_Red", "Bird_Red_01"),
        ("Bird_Blue", "Bird_Blue_01"),
        ("Bird_Yellow", "Bird_Yellow_01"),
        ("Bird_Black", "Bird_Black_01"),
    ]
    .into_iter()
    .map(|(prefix, prefab)| {
        let doc = derive::load_prefab(prefab);
        let face = derive::derive_by_path(&doc, "Visualization/Face");
        let body = derive::derive_by_path(&doc, "Visualization/Body");
        let (face_pivot_x, face_pivot_y) = derive::pivot_world_by_path(&doc, "Visualization/Face");
        let (body_pivot_x, body_pivot_y) = derive::pivot_world_by_path(&doc, "Visualization/Body");
        BirdFaceData {
            name_prefix: prefix,
            atlas: face.atlas,
            uv: face.uv,
            world_w: face.world_w,
            world_h: face.world_h,
            offset_x: (face.offset_x - body.offset_x) + (face_pivot_x - body_pivot_x),
            offset_y: (face.offset_y - body.offset_y) + (face_pivot_y - body_pivot_y),
        }
    })
    .collect()
});

// ─── Bridge step/rope ───────────────────────────────────────────────────

pub(super) static BRIDGE_STEP: LazyLock<SubSprite> =
    LazyLock::new(|| derive::derive_by_name(&derive::load_prefab("Step"), "Graphics"));

// ─── FloatingStarBox / FloatingPartBox ──────────────────────────────────

pub(super) static FLOATING_BOX: LazyLock<SubSprite> =
    LazyLock::new(|| derive::derive_root(&derive::load_prefab("FloatingStarBox"), "FloatingStarBox"));

pub(super) static FLOATING_BALLOON: LazyLock<SubSprite> = LazyLock::new(|| {
    derive::derive_with_cumulative_scale(
        &derive::load_prefab("FloatingStarBox"),
        "Balloons/BalloonsSprite",
    )
});

pub(super) static FLOATING_STAR_BALLOON_DISTANCE: LazyLock<f32> = LazyLock::new(|| {
    derive::distance_xy_by_path(&derive::load_prefab("FloatingStarBox"), "Balloons")
});

pub(super) static FLOATING_PART_BALLOON_DISTANCE: LazyLock<f32> = LazyLock::new(|| {
    derive::distance_xy_by_path(&derive::load_prefab("FloatingPartBox"), "Balloons")
});

pub(super) static FLOATING_STAR_ROPE_BALLOON_ANCHOR_LOCAL: LazyLock<(f32, f32)> =
    LazyLock::new(|| {
        derive::rope_anchor_points_by_path(&derive::load_prefab("FloatingStarBox"), "Balloons").0
    });

pub(super) static FLOATING_PART_ROPE_BALLOON_ANCHOR_LOCAL: LazyLock<(f32, f32)> =
    LazyLock::new(|| {
        derive::rope_anchor_points_by_path(&derive::load_prefab("FloatingPartBox"), "Balloons").0
    });

pub(super) static FLOATING_STAR_ROPE_BOX_ANCHOR_LOCAL: LazyLock<(f32, f32)> = LazyLock::new(|| {
    derive::rope_anchor_points_by_path(&derive::load_prefab("FloatingStarBox"), "Balloons").1
});

pub(super) static FLOATING_PART_ROPE_BOX_ANCHOR_LOCAL: LazyLock<(f32, f32)> = LazyLock::new(|| {
    derive::rope_anchor_points_by_path(&derive::load_prefab("FloatingPartBox"), "Balloons").1
});

#[cfg(test)]
mod prefab_derivation_tests;
