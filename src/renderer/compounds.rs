//! Compound prefab rendering — multi-sprite objects (slingshot, fan, door, button, bird).
//!
//! These prefabs are rendered as groups of sub-sprites with relative positions.
//! Sprite data constants are extracted from the TS editor's prefab-data.ts.

use eframe::egui;

use crate::sprite_db::UvRect;

use super::{CompoundTransform, DrawCtx};

/// Sub-sprite data for compound prefab rendering.
struct SubSprite {
    atlas: &'static str,
    uv: UvRect,
    world_w: f32,
    world_h: f32,
    offset_x: f32,
    offset_y: f32,
    flip_x: bool,
    flip_y: bool,
}

const SCALE: f32 = 10.0 / 768.0;

struct QuadDraw<'a> {
    atlas: &'a str,
    uv: &'a UvRect,
    half_w: f32,
    half_h: f32,
    world_x: f32,
    world_y: f32,
    flip_x: bool,
    flip_y: bool,
    rotation_z: f32,
}

// ─── Slingshot ──────────────────────────────────────────────────────────

const SLINGSHOT_BACK: SubSprite = SubSprite {
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

const SLINGSHOT_PAD: SubSprite = SubSprite {
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

const SLINGSHOT_FRONT: SubSprite = SubSprite {
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

const FAN_PROPELLER: SubSprite = SubSprite {
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

const FAN_ENGINE: SubSprite = SubSprite {
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

const FAN_FRAME: SubSprite = SubSprite {
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

const BUTTON_BASE: SubSprite = SubSprite {
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

struct ButtonBumpData {
    color_suffix: &'static str,
    uv: UvRect,
}

const BUTTON_BUMPS: &[ButtonBumpData] = &[
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

const BUTTON_BUMP_SIZE_W: f32 = 148.0 * 0.4 * SCALE;
const BUTTON_BUMP_SIZE_H: f32 = 21.0 * 0.4 * SCALE;

// ─── ActivatedHingeDoor ─────────────────────────────────────────────────

const DOOR_BAR: SubSprite = SubSprite {
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
const DOOR_BAR_ICE: SubSprite = SubSprite {
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

const DOOR_HINGE_BOTTOM: SubSprite = SubSprite {
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
const DOOR_HINGE_PIVOT_Y: f32 = -18.0 * 0.4 * SCALE;

struct DoorHingeUpperData {
    color_suffix: &'static str,
    uv: UvRect,
}

const DOOR_HINGE_UPPERS: &[DoorHingeUpperData] = &[
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

const DOOR_HINGE_SIZE: f32 = 53.0 * 0.4 * SCALE;

// ─── Bird face sprites ──────────────────────────────────────────────────

struct BirdFaceData {
    name_prefix: &'static str,
    uv: UvRect,
    world_w: f32,
    world_h: f32,
    /// Pre-computed face offset = face_localPos + face_meshPivot - body_meshPivot.
    /// This gives the correct visual face-to-body relative position since the Rust
    /// editor renders the body at its transform position without mesh pivot offset.
    offset_x: f32,
    offset_y: f32,
}

const BIRD_FACES: &[BirdFaceData] = &[
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
    // Bird_Blue: face pivot (0, 0), body pivot (0, 0)
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
        offset_x: -0.06,
        offset_y: -0.15,
    },
    // Bird_Yellow: face pivot (0, 40/768), body pivot (0, 0)
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
        offset_x: -0.06,
        offset_y: -0.15 + 40.0 / 768.0,
    },
];

// ─── Bridge step/rope ───────────────────────────────────────────────────

const BRIDGE_STEP: SubSprite = SubSprite {
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
const FLOATING_BOX: SubSprite = SubSprite {
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
const FLOATING_BALLOON_UV: UvRect = UvRect {
    x: 0.3945313,
    y: 0.5200195,
    w: 0.04492188,
    h: 0.04882813,
};
const FLOATING_BALLOON_W: f32 = 92.0 * 0.875 * SCALE;
const FLOATING_BALLOON_H: f32 = 100.0 * 0.875 * SCALE;

// ─── Public API ─────────────────────────────────────────────────────────

/// Public wrapper for fan override parsing (used by mod.rs for state machine init).
pub fn parse_fan_override_public(raw_text: Option<&str>) -> FanOverridesPublic {
    let ovr = parse_fan_overrides(raw_text);
    FanOverridesPublic {
        start_time: ovr.start_time,
        on_time: ovr.on_time,
        off_time: ovr.off_time,
        delayed_start: ovr.delayed_start,
        always_on: ovr.always_on,
    }
}

/// Parsed fan override values (public subset).
pub struct FanOverridesPublic {
    pub start_time: Option<f32>,
    pub on_time: Option<f32>,
    pub off_time: Option<f32>,
    pub delayed_start: Option<f32>,
    pub always_on: Option<bool>,
}

/// Draw a compound prefab's sub-sprites.
/// Returns true if a compound was drawn (caller should skip normal sprite rendering).
pub fn draw_compound(
    ctx: &DrawCtx<'_>,
    name: &str,
    xf: CompoundTransform,
    time: f64,
    override_text: Option<&str>,
) -> bool {
    if name == "Slingshot" {
        draw_sub_sprites_rotated(
            ctx,
            &[&SLINGSHOT_BACK, &SLINGSHOT_PAD, &SLINGSHOT_FRONT],
            xf,
        );
        return true;
    }

    if name == "Fan" {
        // Unity Z-order: propeller (Z=0, back) → engine (Z=-0.05) → frame (Z=-0.1, front)
        // Draw propeller first with foreshortening animation
        let angle = (time * 10.472) as f32;
        let foreshorten = angle.cos().abs().max(0.05);
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        let lx = FAN_PROPELLER.offset_x * xf.scale_x;
        let ly = FAN_PROPELLER.offset_y * xf.scale_y;
        let px = xf.world_x + lx * cos_r - ly * sin_r;
        let py = xf.world_y + lx * sin_r + ly * cos_r;
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: FAN_PROPELLER.atlas,
                uv: &FAN_PROPELLER.uv,
                half_w: FAN_PROPELLER.world_w * xf.scale_x.abs() * foreshorten,
                half_h: FAN_PROPELLER.world_h * xf.scale_y.abs(),
                world_x: px,
                world_y: py,
                flip_x: FAN_PROPELLER.flip_x != (xf.scale_x < 0.0),
                flip_y: FAN_PROPELLER.flip_y != (xf.scale_y < 0.0),
                rotation_z: xf.rotation_z,
            },
        );
        // Then engine (middle) and frame (front)
        draw_sub_sprites_rotated(ctx, &[&FAN_ENGINE, &FAN_FRAME], xf);
        return true; // skip root sprite (propeller already drawn)
    }

    if name.starts_with("PressureButton") {
        let color = name.strip_prefix("PressureButton").unwrap_or("");
        draw_sub_sprites_rotated(ctx, &[&BUTTON_BASE], xf);
        // Draw color-specific bump
        if let Some(bump) = BUTTON_BUMPS.iter().find(|b| b.color_suffix == color) {
            let cos_r = xf.rotation_z.cos();
            let sin_r = xf.rotation_z.sin();
            let lx = 0.0_f32 * xf.scale_x;
            let ly = -0.012 * xf.scale_y;
            let bx = xf.world_x + lx * cos_r - ly * sin_r;
            let by = xf.world_y + lx * sin_r + ly * cos_r;
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: "IngameAtlas2.png",
                    uv: &bump.uv,
                    half_w: BUTTON_BUMP_SIZE_W * xf.scale_x.abs(),
                    half_h: BUTTON_BUMP_SIZE_H * xf.scale_y.abs(),
                    world_x: bx,
                    world_y: by,
                    flip_x: false,
                    flip_y: false,
                    rotation_z: xf.rotation_z,
                },
            );
        }
        return true; // root has no visual — skip it
    }

    if name.starts_with("ActivatedHingeDoor") {
        let suffix = name.strip_prefix("ActivatedHingeDoor").unwrap_or("");
        let is_ice = suffix.ends_with("_Ice");
        let color = suffix.trim_end_matches("_Ice");
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();

        // Bar: Ice uses a separate horizontal sprite, normal uses the vertical bar
        let bar = if is_ice { &DOOR_BAR_ICE } else { &DOOR_BAR };
        let bar_rot = if is_ice {
            xf.rotation_z + (-std::f32::consts::FRAC_PI_2)
        } else {
            xf.rotation_z
        };
        {
            let lx = bar.offset_x * xf.scale_x;
            let ly = bar.offset_y * xf.scale_y;
            let bx = xf.world_x + lx * cos_r - ly * sin_r;
            let by = xf.world_y + lx * sin_r + ly * cos_r;
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: bar.atlas,
                    uv: &bar.uv,
                    half_w: bar.world_w * xf.scale_x.abs(),
                    half_h: bar.world_h * xf.scale_y.abs(),
                    world_x: bx,
                    world_y: by,
                    flip_x: bar.flip_x != (xf.scale_x < 0.0),
                    flip_y: bar.flip_y != (xf.scale_y < 0.0),
                    rotation_z: bar_rot,
                },
            );
        }
        // Lower hinge: 180° rotation, pivot Y offset shifts sprite upward
        {
            let lx = DOOR_HINGE_BOTTOM.offset_x * xf.scale_x;
            let ly = (DOOR_HINGE_BOTTOM.offset_y + DOOR_HINGE_PIVOT_Y) * xf.scale_y;
            let hx = xf.world_x + lx * cos_r - ly * sin_r;
            let hy = xf.world_y + lx * sin_r + ly * cos_r;
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: DOOR_HINGE_BOTTOM.atlas,
                    uv: &DOOR_HINGE_BOTTOM.uv,
                    half_w: DOOR_HINGE_BOTTOM.world_w * xf.scale_x.abs(),
                    half_h: DOOR_HINGE_BOTTOM.world_h * xf.scale_y.abs(),
                    world_x: hx,
                    world_y: hy,
                    flip_x: DOOR_HINGE_BOTTOM.flip_x != (xf.scale_x < 0.0),
                    flip_y: DOOR_HINGE_BOTTOM.flip_y != (xf.scale_y < 0.0),
                    rotation_z: xf.rotation_z + std::f32::consts::PI,
                },
            );
        }
        // Upper hinge: color-specific, Y-flipped (prefab scaleY=-1), pivot offset
        if let Some(hinge) = DOOR_HINGE_UPPERS.iter().find(|h| h.color_suffix == color) {
            let lx = 0.0_f32;
            let ly = (0.123 + DOOR_HINGE_PIVOT_Y) * xf.scale_y;
            let ux = xf.world_x + lx * cos_r - ly * sin_r;
            let uy = xf.world_y + lx * sin_r + ly * cos_r;
            let flip_y_val = xf.scale_y >= 0.0; // prefab scaleY=-1 baked: flip when parent NOT flipped
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: "IngameAtlas2.png",
                    uv: &hinge.uv,
                    half_w: DOOR_HINGE_SIZE * xf.scale_x.abs(),
                    half_h: DOOR_HINGE_SIZE * xf.scale_y.abs(),
                    world_x: ux,
                    world_y: uy,
                    flip_x: xf.scale_x < 0.0,
                    flip_y: flip_y_val,
                    rotation_z: xf.rotation_z,
                },
            );
        }
        return true; // skip root sprite — Unity root has no visual component
    }

    if name.starts_with("Bird_") && !name.starts_with("BirdCompass") {
        // Face is drawn by draw_bird_face() AFTER the body sprite, so it renders in front.
        return false; // still draw root bird sprite (body)
    }

    if name == "Bridge" {
        // Parse override data for bridge parameters
        let ovr = parse_bridge_overrides(override_text);
        let step_length = ovr.step_length.unwrap_or(1.0);
        let step_gap = ovr.step_gap.unwrap_or(0.2);
        let endpoint_x = ovr.end_point_x.unwrap_or(2.561546);
        let endpoint_y = ovr.end_point_y.unwrap_or(0.0);
        let dist = (endpoint_x * endpoint_x + endpoint_y * endpoint_y).sqrt();
        let stride = step_length + step_gap;
        let step_count = (dist / stride).floor() as i32;
        let angle = endpoint_y.atan2(endpoint_x);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        for i in 0..step_count {
            let along = (i as f32 + 0.5) * stride;
            let lx = along * cos_a;
            let ly = along * sin_a;
            draw_uv_quad_rotated(
                ctx,
                QuadDraw {
                    atlas: BRIDGE_STEP.atlas,
                    uv: &BRIDGE_STEP.uv,
                    half_w: BRIDGE_STEP.world_w * xf.scale_x.abs(),
                    half_h: BRIDGE_STEP.world_h * xf.scale_y.abs(),
                    world_x: xf.world_x + lx * xf.scale_x,
                    world_y: xf.world_y + ly * xf.scale_y,
                    flip_x: false,
                    flip_y: false,
                    rotation_z: angle,
                },
            );
            // Rope between steps
            if i > 0 {
                let prev_along = (i as f32 - 0.5) * stride;
                let half_step = step_length * 0.5;
                let rope_start_d = prev_along + half_step;
                let rope_end_d = along - half_step;
                let p0 = ctx.camera.world_to_screen(
                    crate::types::Vec2 {
                        x: xf.world_x + rope_start_d * cos_a * xf.scale_x,
                        y: xf.world_y + rope_start_d * sin_a * xf.scale_y,
                    },
                    ctx.canvas_center,
                );
                let p1 = ctx.camera.world_to_screen(
                    crate::types::Vec2 {
                        x: xf.world_x + rope_end_d * cos_a * xf.scale_x,
                        y: xf.world_y + rope_end_d * sin_a * xf.scale_y,
                    },
                    ctx.canvas_center,
                );
                ctx.painter.line_segment(
                    [p0, p1],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(0x8B, 0x73, 0x55)),
                );
            }
        }
        // First rope: origin → first step left edge
        if step_count > 0 {
            let half_step = step_length * 0.5;
            let p0 = ctx.camera.world_to_screen(
                crate::types::Vec2 {
                    x: xf.world_x,
                    y: xf.world_y,
                },
                ctx.canvas_center,
            );
            let p1 = ctx.camera.world_to_screen(
                crate::types::Vec2 {
                    x: xf.world_x + (0.5 * stride - half_step) * cos_a * xf.scale_x,
                    y: xf.world_y + (0.5 * stride - half_step) * sin_a * xf.scale_y,
                },
                ctx.canvas_center,
            );
            ctx.painter.line_segment(
                [p0, p1],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(0x8B, 0x73, 0x55)),
            );
            // Last rope: last step right edge → endpoint
            let last_right = ((step_count - 1) as f32 + 0.5) * stride + half_step;
            let p0 = ctx.camera.world_to_screen(
                crate::types::Vec2 {
                    x: xf.world_x + last_right * cos_a * xf.scale_x,
                    y: xf.world_y + last_right * sin_a * xf.scale_y,
                },
                ctx.canvas_center,
            );
            let p1 = ctx.camera.world_to_screen(
                crate::types::Vec2 {
                    x: xf.world_x + endpoint_x * xf.scale_x,
                    y: xf.world_y + endpoint_y * xf.scale_y,
                },
                ctx.canvas_center,
            );
            ctx.painter.line_segment(
                [p0, p1],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(0x8B, 0x73, 0x55)),
            );
        }
        return true;
    }

    if name.starts_with("FloatingStarBox") || name.starts_with("FloatingPartBox") {
        let is_part_box = name.starts_with("FloatingPartBox");
        let sx = xf.scale_x.abs();
        let sy = xf.scale_y.abs();

        // Unity SpringJoint (spring=100, damper=10) makes both box and balloon oscillate.
        // Balloon bobs more (lighter, receives upForce), box bobs less (heavier end).
        let phase = (time * 1.8).sin() as f32;
        let box_bob = 0.06 * phase * sy;
        let balloon_bob = 0.15 * phase * sy;
        let box_y = xf.world_y + box_bob;

        // 1. Box sprite at root position (bobs gently)
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: FLOATING_BOX.atlas,
                uv: &FLOATING_BOX.uv,
                half_w: FLOATING_BOX.world_w * sx,
                half_h: FLOATING_BOX.world_h * sy,
                world_x: xf.world_x,
                world_y: box_y,
                flip_x: false,
                flip_y: false,
                rotation_z: xf.rotation_z,
            },
        );

        // 2. Balloon above at physics equilibrium distance + bobbing
        let balloon_dist = if is_part_box { 3.725 } else { 3.749 } * sx;
        let balloon_y = box_y + balloon_dist + (balloon_bob - box_bob);
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: "IngameAtlas.png",
                uv: &FLOATING_BALLOON_UV,
                half_w: FLOATING_BALLOON_W * sx,
                half_h: FLOATING_BALLOON_H * sy,
                world_x: xf.world_x,
                world_y: balloon_y,
                flip_x: false,
                flip_y: false,
                rotation_z: 0.0,
            },
        );

        // 3. Rope: black line from rotated box anchor to balloon bottom
        //    Unity LineRenderer width = 0.05 world units
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        let rbx = (1.25 * cos_r - 1.25 * sin_r) * sx;
        let rby = (1.25 * sin_r + 1.25 * cos_r) * sy;
        let rtx = 0.0;
        let rty = balloon_dist + (balloon_bob - box_bob) - 0.875 * sy;
        let rope_bot = ctx.camera.world_to_screen(
            crate::types::Vec2 {
                x: xf.world_x + rbx,
                y: box_y + rby,
            },
            ctx.canvas_center,
        );
        let rope_top = ctx.camera.world_to_screen(
            crate::types::Vec2 {
                x: xf.world_x + rtx,
                y: box_y + rty,
            },
            ctx.canvas_center,
        );
        let rope_width = (0.05 * ctx.camera.zoom).max(1.0);
        ctx.painter.line_segment(
            [rope_bot, rope_top],
            egui::Stroke::new(rope_width, egui::Color32::BLACK),
        );

        return true; // skip root sprite (balloon is drawn above)
    }

    false
}

/// Draw Bird face sprite AFTER the body has been rendered, so it appears in front.
/// `world_y` (in xf) should already include the sleep bob offset.
/// `breath_sx`/`breath_sy` are the hermite-evaluated scale factors from the vizGroup.
pub fn draw_bird_face(
    ctx: &DrawCtx<'_>,
    name: &str,
    xf: CompoundTransform,
    breath_sx: f32,
    breath_sy: f32,
) {
    // Strip trailing _01 etc to match face lookup
    let base_name = name.trim_end_matches(|c: char| c == '_' || c.is_ascii_digit());
    let base_name = if base_name.is_empty() {
        name
    } else {
        base_name
    };

    if let Some(face) = BIRD_FACES
        .iter()
        .find(|f| base_name.starts_with(f.name_prefix))
    {
        let cos_r = xf.rotation_z.cos();
        let sin_r = xf.rotation_z.sin();
        // Face offset in visualization-local space (breathing scales the viz group).
        // Per-bird offset accounts for both face mesh pivot and body mesh pivot.
        let lx = face.offset_x * breath_sx * xf.scale_x;
        let ly = face.offset_y * breath_sy * xf.scale_y;
        let fx = xf.world_x + lx * cos_r - ly * sin_r;
        let fy = xf.world_y + lx * sin_r + ly * cos_r;
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: "IngameAtlas.png",
                uv: &face.uv,
                half_w: face.world_w * xf.scale_x.abs() * breath_sx,
                half_h: face.world_h * xf.scale_y.abs() * breath_sy,
                world_x: fx,
                world_y: fy,
                flip_x: xf.scale_x < 0.0,
                flip_y: xf.scale_y < 0.0,
                rotation_z: xf.rotation_z,
            },
        );
    }
}

// ─── Internal helpers ───────────────────────────────────────────────────

fn draw_sub_sprites_rotated(ctx: &DrawCtx, sprites: &[&SubSprite], xf: CompoundTransform) {
    let cos_r = xf.rotation_z.cos();
    let sin_r = xf.rotation_z.sin();
    for s in sprites {
        let lx = s.offset_x * xf.scale_x;
        let ly = s.offset_y * xf.scale_y;
        let sx = xf.world_x + lx * cos_r - ly * sin_r;
        let sy = xf.world_y + lx * sin_r + ly * cos_r;
        let flip_x = s.flip_x != (xf.scale_x < 0.0);
        let flip_y = s.flip_y != (xf.scale_y < 0.0);
        draw_uv_quad_rotated(
            ctx,
            QuadDraw {
                atlas: s.atlas,
                uv: &s.uv,
                half_w: s.world_w * xf.scale_x.abs(),
                half_h: s.world_h * xf.scale_y.abs(),
                world_x: sx,
                world_y: sy,
                flip_x,
                flip_y,
                rotation_z: xf.rotation_z,
            },
        );
    }
}

fn draw_uv_quad_rotated(ctx: &DrawCtx, q: QuadDraw<'_>) {
    let tex_id = match ctx.tex_cache.get(q.atlas) {
        Some(id) => id,
        None => return,
    };

    let center = ctx.camera.world_to_screen(
        crate::types::Vec2 {
            x: q.world_x,
            y: q.world_y,
        },
        ctx.canvas_center,
    );

    let hw = q.half_w * ctx.camera.zoom;
    let hh = q.half_h * ctx.camera.zoom;

    // Frustum cull
    let margin = hw.max(hh);
    if center.x + margin < ctx.canvas_rect.left()
        || center.x - margin > ctx.canvas_rect.right()
        || center.y + margin < ctx.canvas_rect.top()
        || center.y - margin > ctx.canvas_rect.bottom()
    {
        return;
    }

    let (u0, u1) = if q.flip_x {
        (q.uv.x + q.uv.w, q.uv.x)
    } else {
        (q.uv.x, q.uv.x + q.uv.w)
    };
    // UV Y flip: Unity V=0 at bottom, egui V=0 at top
    let (v0, v1) = if q.flip_y {
        (1.0 - q.uv.y, 1.0 - q.uv.y - q.uv.h)
    } else {
        (1.0 - q.uv.y - q.uv.h, 1.0 - q.uv.y)
    };

    let mut mesh = egui::Mesh::with_texture(tex_id);
    if q.rotation_z.abs() > 0.001 {
        let cos_r = q.rotation_z.cos();
        let sin_r = q.rotation_z.sin();
        // Screen Y is inverted (down = positive), negate sin for rotation
        let rot = |dx: f32, dy: f32| -> egui::Pos2 {
            egui::pos2(
                center.x + dx * cos_r + dy * sin_r,
                center.y - dx * sin_r + dy * cos_r,
            )
        };
        let tl = rot(-hw, -hh);
        let tr = rot(hw, -hh);
        let br = rot(hw, hh);
        let bl = rot(-hw, hh);
        let white = egui::Color32::WHITE;
        let i = mesh.vertices.len() as u32;
        mesh.vertices.push(egui::epaint::Vertex {
            pos: tl,
            uv: egui::pos2(u0, v0),
            color: white,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: tr,
            uv: egui::pos2(u1, v0),
            color: white,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: br,
            uv: egui::pos2(u1, v1),
            color: white,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: bl,
            uv: egui::pos2(u0, v1),
            color: white,
        });
        mesh.indices
            .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    } else {
        let rect = egui::Rect::from_center_size(center, egui::vec2(hw * 2.0, hh * 2.0));
        let uv_rect = egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1));
        mesh.add_rect_with_uv(rect, uv_rect, egui::Color32::WHITE);
    }
    ctx.painter.add(egui::Shape::mesh(mesh));
}

// ─── Override parsers ───────────────────────────────────────────────────

struct BridgeOverrides {
    end_point_x: Option<f32>,
    end_point_y: Option<f32>,
    step_length: Option<f32>,
    step_gap: Option<f32>,
}

/// Parse Bridge component overrides from tab-indented ObjectDeserializer format.
fn parse_bridge_overrides(raw_text: Option<&str>) -> BridgeOverrides {
    let mut result = BridgeOverrides {
        end_point_x: None,
        end_point_y: None,
        step_length: None,
        step_gap: None,
    };
    let text = match raw_text {
        Some(t) => t,
        None => return result,
    };

    let mut in_bridge_component = false;
    let mut in_endpoint = false;
    let mut in_transform = false;
    let mut in_local_position = false;

    for line in text.lines() {
        let stripped = line.trim_start_matches('\t');
        let depth = line.len() - stripped.len();
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if depth == 0 {
            in_bridge_component = false;
            in_endpoint = false;
            in_transform = false;
            in_local_position = false;
            continue;
        }
        if depth == 1 {
            in_local_position = false;
            if trimmed.starts_with("Component Bridge") {
                in_bridge_component = true;
                in_endpoint = false;
                in_transform = false;
            } else if trimmed == "GameObject EndPoint" {
                in_endpoint = true;
                in_bridge_component = false;
                in_transform = false;
            } else if trimmed.starts_with("GameObject ") {
                in_endpoint = false;
                in_bridge_component = false;
                in_transform = false;
            } else {
                in_bridge_component = false;
                in_transform = false;
            }
            continue;
        }
        if depth == 2 {
            if in_bridge_component {
                if let Some(val) = parse_float_field(trimmed, "stepLength") {
                    result.step_length = Some(val);
                } else if let Some(val) = parse_float_field(trimmed, "stepGap") {
                    result.step_gap = Some(val);
                }
            } else if in_endpoint {
                in_transform = trimmed == "Component UnityEngine.Transform";
                in_local_position = false;
            }
            continue;
        }
        if depth == 3 && in_endpoint && in_transform {
            in_local_position = trimmed == "Vector3 m_LocalPosition";
            continue;
        }
        if depth == 4 && in_endpoint && in_local_position {
            if let Some(val) = parse_float_field(trimmed, "x") {
                result.end_point_x = Some(val);
            } else if let Some(val) = parse_float_field(trimmed, "y") {
                result.end_point_y = Some(val);
            }
        }
    }
    result
}

struct FanOverrides {
    target_force: Option<f32>,
    start_time: Option<f32>,
    on_time: Option<f32>,
    off_time: Option<f32>,
    delayed_start: Option<f32>,
    always_on: Option<bool>,
}

/// Parse Fan component overrides from tab-indented ObjectDeserializer format.
fn parse_fan_overrides(raw_text: Option<&str>) -> FanOverrides {
    let mut result = FanOverrides {
        target_force: None,
        start_time: None,
        on_time: None,
        off_time: None,
        delayed_start: None,
        always_on: None,
    };
    let text = match raw_text {
        Some(t) => t,
        None => return result,
    };

    let mut in_fan_component = false;
    let mut current_child_go: Option<String> = None;

    for line in text.lines() {
        let stripped = line.trim_start_matches('\t');
        let depth = line.len() - stripped.len();
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if depth == 0 {
            current_child_go = None;
            in_fan_component = false;
            continue;
        }
        if depth == 1 {
            if trimmed.starts_with("Component Fan") && !trimmed.contains('.') {
                current_child_go = None;
                in_fan_component = true;
            } else if let Some(rest) = trimmed.strip_prefix("GameObject ") {
                current_child_go = Some(rest.to_string());
                in_fan_component = false;
            } else {
                in_fan_component = false;
            }
            continue;
        }
        if depth == 2 && in_fan_component && current_child_go.is_none() {
            if let Some(val) = parse_float_field(trimmed, "targetForce") {
                result.target_force = Some(val);
            } else if let Some(val) = parse_float_field(trimmed, "startTime") {
                result.start_time = Some(val);
            } else if let Some(val) = parse_float_field(trimmed, "onTime") {
                result.on_time = Some(val);
            } else if let Some(val) = parse_float_field(trimmed, "offTime") {
                result.off_time = Some(val);
            } else if let Some(val) = parse_float_field(trimmed, "delayedStart") {
                result.delayed_start = Some(val);
            } else if let Some(val) = parse_bool_field(trimmed, "alwaysOn") {
                result.always_on = Some(val);
            }
        }
    }
    result
}

/// Parse "Float fieldName = value" or "Float fieldName=value", return value if field matches.
fn parse_float_field(trimmed: &str, field: &str) -> Option<f32> {
    // Pattern: "Float fieldName = value" or "Float fieldName=value"
    let prefix = format!("Float {}", field);
    if !trimmed.starts_with(&prefix) {
        return None;
    }
    let rest = &trimmed[prefix.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    rest.trim().parse::<f32>().ok()
}

/// Parse "Boolean fieldName = value".
fn parse_bool_field(trimmed: &str, field: &str) -> Option<bool> {
    let prefix = format!("Boolean {}", field);
    if !trimmed.starts_with(&prefix) {
        return None;
    }
    let rest = &trimmed[prefix.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    let val = rest.trim();
    Some(val.eq_ignore_ascii_case("true"))
}
