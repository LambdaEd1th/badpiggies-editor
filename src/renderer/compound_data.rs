//! Compound prefab sprite data constants — UV rects, sizes, offsets.
//!
//! Extracted from the TS editor's prefab-data.ts. These are pure data tables
//! with no logic — the rendering code lives in compounds.rs.

use crate::sprite_db::UvRect;

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

// ─── Slingshot ──────────────────────────────────────────────────────────

pub(super) const SLINGSHOT_BACK: SubSprite = SubSprite {
    atlas: "IngameAtlas.png",
    uv: UvRect {
        x: 0.0,
        y: 0.472168,
        w: 0.05273438,
        h: 0.2431641,
    },
    world_w: 108.0 * 0.4 * SCALE,
    world_h: 498.0 * 0.4 * SCALE,
    offset_x: 0.2329,
    offset_y: 0.2406,
    flip_x: false,
    flip_y: false,
};

pub(super) const SLINGSHOT_PAD: SubSprite = SubSprite {
    atlas: "IngameAtlas.png",
    uv: UvRect {
        x: 0.5292969,
        y: 0.6572266,
        w: 0.01904297,
        h: 0.03027344,
    },
    world_w: 39.0 * 0.4 * SCALE,
    world_h: 62.0 * 0.4 * SCALE,
    offset_x: -0.0064,
    offset_y: 2.2587,
    flip_x: false,
    flip_y: false,
};

pub(super) const SLINGSHOT_FRONT: SubSprite = SubSprite {
    atlas: "IngameAtlas.png",
    uv: UvRect {
        x: 0.1416016,
        y: 0.3066406,
        w: 0.04638672,
        h: 0.1445313,
    },
    world_w: 95.0 * 0.3984375 * SCALE,
    world_h: 296.0 * 0.4 * SCALE,
    offset_x: -0.472,
    offset_y: 1.361,
    flip_x: false,
    flip_y: false,
};

// ─── Fan ────────────────────────────────────────────────────────────────

pub(super) const FAN_PROPELLER: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.8891602,
        y: 0.4277344,
        w: 0.1000977,
        h: 0.02880859,
    },
    world_w: 205.0 * 0.4 * SCALE,
    world_h: 59.0 * 0.4 * SCALE,
    offset_x: 0.0,
    offset_y: 0.081,
    flip_x: false,
    flip_y: false,
};

pub(super) const FAN_ENGINE: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.8056641,
        y: 0.05810547,
        w: 0.09082031,
        h: 0.02490234,
    },
    world_w: 186.0 * 0.4 * SCALE,
    world_h: 51.0 * 0.4 * SCALE,
    offset_x: 0.0,
    offset_y: -0.352,
    flip_x: false,
    flip_y: false,
};

pub(super) const FAN_FRAME: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.6914063,
        y: 0.08544922,
        w: 0.1245117,
        h: 0.06396484,
    },
    world_w: 255.0 * 0.4 * SCALE,
    world_h: 131.0 * 0.4 * SCALE,
    offset_x: 0.0,
    offset_y: 0.0,
    flip_x: false,
    flip_y: false,
};

// ─── PressureButton ─────────────────────────────────────────────────────

pub(super) const BUTTON_BASE: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.5307617,
        y: 0.06542969,
        w: 0.08837891,
        h: 0.01513672,
    },
    world_w: 181.0 * 0.4 * SCALE,
    world_h: 31.0 * 0.4 * SCALE,
    offset_x: 0.0,
    offset_y: -0.24,
    flip_x: false,
    flip_y: false,
};

pub(super) struct ButtonBumpData {
    pub color_suffix: &'static str,
    pub uv: UvRect,
}

pub(super) const BUTTON_BUMPS: &[ButtonBumpData] = &[
    ButtonBumpData {
        color_suffix: "Blue",
        uv: UvRect {
            x: 0.2631836,
            y: 0.2124023,
            w: 0.07226563,
            h: 0.01025391,
        },
    },
    ButtonBumpData {
        color_suffix: "Red",
        uv: UvRect {
            x: 0.4238281,
            y: 0.2089844,
            w: 0.07226563,
            h: 0.01025391,
        },
    },
    ButtonBumpData {
        color_suffix: "Green",
        uv: UvRect {
            x: 0.7216797,
            y: 0.06494141,
            w: 0.07226563,
            h: 0.01025391,
        },
    },
    ButtonBumpData {
        color_suffix: "Yellow",
        uv: UvRect {
            x: 0.3745117,
            y: 0.34375,
            w: 0.07226563,
            h: 0.01025391,
        },
    },
];

pub(super) const BUTTON_BUMP_SIZE_W: f32 = 148.0 * 0.4 * SCALE;
pub(super) const BUTTON_BUMP_SIZE_H: f32 = 21.0 * 0.4 * SCALE;

// ─── ActivatedHingeDoor ─────────────────────────────────────────────────

pub(super) const DOOR_BAR: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.4082031,
        y: 0.4277344,
        w: 0.01660156,
        h: 0.2045898,
    },
    world_w: 34.0 * 0.4 * SCALE,
    world_h: 419.0 * 0.38 * SCALE,
    offset_x: 0.001,
    offset_y: -0.07235503 + -2.267,
    flip_x: false,
    flip_y: false,
};

// Ice variant bar: 419×35px, scaleX=0.4, scaleY=0.38 — separate horizontal sprite
pub(super) const DOOR_BAR_ICE: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.6503906,
        y: 0.3164063,
        w: 0.2045898,
        h: 0.01708984,
    },
    world_w: 419.0 * 0.4 * SCALE,
    world_h: 35.0 * 0.38 * SCALE,
    offset_x: 0.001,
    offset_y: -0.07235503 + -2.267,
    flip_x: false,
    flip_y: false,
};

pub(super) const DOOR_HINGE_BOTTOM: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.168457,
        y: 0.5673828,
        w: 0.01025391,
        h: 0.02783203,
    },
    world_w: 21.0 * 0.4 * SCALE,
    world_h: 57.0 * 0.4 * SCALE,
    offset_x: 0.001314,
    offset_y: -0.07235503 + -0.0206,
    flip_x: false,
    flip_y: false,
};

/// Hinge sprite pivot offset: m_pivotY = -18, m_scaleY = 0.4
/// Unity Sprite.CreateMesh shifts mesh by -2 * int(scaleY * pivotY) * 10/768
/// = -2 * int(0.4 * -18) * 10/768 = -2 * (-7) * 10/768 = 0.1823
/// TS editor uses: -18 * 0.4 * 10/768 = -0.09375 (slightly different from Unity's int truncation)
pub(super) const DOOR_HINGE_PIVOT_Y: f32 = -18.0 * 0.4 * SCALE;

pub(super) struct DoorHingeUpperData {
    pub color_suffix: &'static str,
    pub uv: UvRect,
}

pub(super) const DOOR_HINGE_UPPERS: &[DoorHingeUpperData] = &[
    DoorHingeUpperData {
        color_suffix: "Blue",
        uv: UvRect {
            x: 0.6630859,
            y: 0.7021484,
            w: 0.02587891,
            h: 0.02587891,
        },
    },
    DoorHingeUpperData {
        color_suffix: "Red",
        uv: UvRect {
            x: 0.6630859,
            y: 0.7578125,
            w: 0.02587891,
            h: 0.02587891,
        },
    },
    DoorHingeUpperData {
        color_suffix: "Green",
        uv: UvRect {
            x: 0.6630859,
            y: 0.7299805,
            w: 0.02587891,
            h: 0.02587891,
        },
    },
    DoorHingeUpperData {
        color_suffix: "Yellow",
        uv: UvRect {
            x: 0.6630859,
            y: 0.7856445,
            w: 0.02587891,
            h: 0.02587891,
        },
    },
];

pub(super) const DOOR_HINGE_SIZE: f32 = 53.0 * 0.4 * SCALE;

// ─── Bird face sprites ──────────────────────────────────────────────────

pub(super) struct BirdFaceData {
    pub name_prefix: &'static str,
    pub uv: UvRect,
    pub world_w: f32,
    pub world_h: f32,
    /// Pre-computed face offset = face_localPos + face_meshPivot - body_meshPivot.
    /// This gives the correct visual face-to-body relative position since the Rust
    /// editor renders the body at its transform position without mesh pivot offset.
    pub offset_x: f32,
    pub offset_y: f32,
}

pub(super) const BIRD_FACES: &[BirdFaceData] = &[
    // Bird_Red: face pivot (0, 20/768), body pivot (40/768, 0)
    // offset = (-0.06 + 0 - 40/768, -0.15 + 20/768 - 0)
    BirdFaceData {
        name_prefix: "Bird_Red",
        uv: UvRect {
            x: 0.7143555,
            y: 0.6708984,
            w: 0.03076172,
            h: 0.02441406,
        },
        world_w: 25.0 * SCALE,
        world_h: 20.0 * SCALE,
        offset_x: -0.06 - 40.0 / 768.0,
        offset_y: -0.15 + 20.0 / 768.0,
    },
    // Bird_Blue: face localPos (-0.094269, 0.004068), face pivot (0,0), body pivot (0,0)
    BirdFaceData {
        name_prefix: "Bird_Blue",
        uv: UvRect {
            x: 0.8925781,
            y: 0.5996094,
            w: 0.01953125,
            h: 0.01464844,
        },
        world_w: 16.0 * SCALE,
        world_h: 12.0 * SCALE,
        offset_x: -0.0942688,
        offset_y: 0.00406751,
    },
    // Bird_Yellow: face localPos (-0.175139, -0.266569), face pivot (0, 40/768), body pivot (0,0)
    BirdFaceData {
        name_prefix: "Bird_Yellow",
        uv: UvRect {
            x: 0.9375,
            y: 0.7265625,
            w: 0.03076172,
            h: 0.01708984,
        },
        world_w: 25.0 * SCALE,
        world_h: 14.0 * SCALE,
        offset_x: -0.175139,
        offset_y: -0.2665691 + 40.0 / 768.0,
    },
    // Bird_Black: face localPos (-0.043033, -0.009248), face pivot (0,0), body pivot (0,0)
    BirdFaceData {
        name_prefix: "Bird_Black",
        uv: UvRect {
            x: 0.7895508,
            y: 0.6162109,
            w: 0.03613281,
            h: 0.02148438,
        },
        world_w: 29.0 * SCALE,
        world_h: 17.0 * SCALE,
        offset_x: -0.04303312,
        offset_y: -0.00924778,
    },
];

// ─── Bridge step/rope ───────────────────────────────────────────────────

pub(super) const BRIDGE_STEP: SubSprite = SubSprite {
    atlas: "IngameAtlas2.png",
    uv: UvRect {
        x: 0.6679688,
        y: 0.06494141,
        w: 0.05175781,
        h: 0.01464844,
    },
    world_w: 106.0 * 0.4 * SCALE,
    world_h: 30.0 * 0.4 * SCALE,
    offset_x: 0.0,
    offset_y: 0.0,
    flip_x: false,
    flip_y: false,
};

// ─── FloatingStarBox / FloatingPartBox ──────────────────────────────────

// StarBox / DynamicPartBox sprite (same UV)
pub(super) const FLOATING_BOX: SubSprite = SubSprite {
    atlas: "IngameAtlas.png",
    uv: UvRect {
        x: 0.3325195,
        y: 0.7480469,
        w: 0.05126953,
        h: 0.05175781,
    },
    world_w: 105.0 * SCALE,
    world_h: 106.0 * SCALE,
    offset_x: 0.0,
    offset_y: 0.0,
    flip_x: false,
    flip_y: false,
};

// Balloon sprite (FloatingStarBox / FloatingPartBox share same UV)
pub(super) const FLOATING_BALLOON_UV: UvRect = UvRect {
    x: 0.3945313,
    y: 0.5200195,
    w: 0.04492188,
    h: 0.04882813,
};
pub(super) const FLOATING_BALLOON_W: f32 = 92.0 * 0.875 * SCALE;
pub(super) const FLOATING_BALLOON_H: f32 = 100.0 * 0.875 * SCALE;
