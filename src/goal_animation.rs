use crate::data::unity_anim;

pub const GOAL_ANIMATION_OVERRIDE_NAME: &str = "bp_goalAnimation";
const GOAL_ANIMATION_IDLE_VALUE: &str = "Idle";
const GOAL_ANIMATION_VANISHING_VALUE: &str = "Vanishing";
const DEFAULT_GOAL_VANISH_DURATION: f32 = 1.0;
const DEFAULT_GOAL_VANISH_HOLD: f32 = 0.35;
const DEFAULT_GOAL_VANISH_POS_Y: &[unity_anim::HermiteKey] = &[
    (0.0, -0.01172495, -2.38242, -2.38242),
    (0.5, -1.202935, 0.415809, 0.415809),
    (1.0, 13.51, 29.42587, 29.42587),
];
const DEFAULT_GOAL_VANISH_SCALE_X: &[unity_anim::HermiteKey] = &[
    (0.0, 1.3, -2.1, -2.1),
    (0.5, 0.25, 0.2071749, 0.2071749),
    (1.0, 15.0, 29.5, 29.5),
];
const DEFAULT_GOAL_VANISH_SCALE_Y: &[unity_anim::HermiteKey] = &[
    (0.0, 0.6500001, 1.7, 1.7),
    (0.5, 1.5, -0.6500001, -0.6500001),
    (1.0, 0.0, -3.0, -3.0),
];
const DEFAULT_GOAL_VANISH_ALPHA: &[unity_anim::HermiteKey] = &[
    (0.0, 1.0, -0.4367118, -0.4367118),
    (0.6166667, 0.7306944, -0.6272243, -0.6272243),
    (1.0, 0.0, -1.906159, -1.906159),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalAnimationState {
    Idle,
    Vanishing,
}

#[derive(Debug, Clone, Copy)]
pub struct GoalVisualState {
    pub y_offset: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub alpha: f32,
}

pub fn parse_goal_animation_state(raw_text: Option<&str>) -> GoalAnimationState {
    raw_text
        .and_then(goal_animation_override_value)
        .map(GoalAnimationState::from_override_value)
        .unwrap_or(GoalAnimationState::Idle)
}

pub fn set_goal_animation_state(raw_text: &mut String, state: GoalAnimationState) {
    let mut result = Vec::new();
    let mut replaced = false;
    let value = state.override_value();

    for line in raw_text.lines() {
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        if trimmed.starts_with("String ")
            && trimmed
                .strip_prefix("String ")
                .is_some_and(|rest| rest.starts_with(&format!("{GOAL_ANIMATION_OVERRIDE_NAME} = ")))
        {
            replaced = true;
            if let Some(value) = value {
                let indent_len = line.len() - line.trim_start_matches('\t').len();
                let indent = "\t".repeat(indent_len);
                result.push(format!(
                    "{indent}String {GOAL_ANIMATION_OVERRIDE_NAME} = {value}"
                ));
            }
            continue;
        }
        result.push(line.to_string());
    }

    if !replaced && let Some(value) = value {
        result.push(format!(
            "String {GOAL_ANIMATION_OVERRIDE_NAME} = {value}"
        ));
    }

    *raw_text = result.join("\n");
    if !raw_text.is_empty() {
        raw_text.push('\n');
    }
}

pub fn goal_visual_state(state: GoalAnimationState, time: f64, preview_seed: usize) -> GoalVisualState {
    match state {
        GoalAnimationState::Idle => GoalVisualState {
            y_offset: (time * 3.0).sin() as f32 * 0.25,
            scale_x: 1.0,
            scale_y: 1.0,
            alpha: 1.0,
        },
        GoalAnimationState::Vanishing => goal_vanishing_visual_state(time, preview_seed),
    }
}

impl GoalAnimationState {
    pub fn label(self) -> &'static str {
        match self {
            GoalAnimationState::Idle => GOAL_ANIMATION_IDLE_VALUE,
            GoalAnimationState::Vanishing => GOAL_ANIMATION_VANISHING_VALUE,
        }
    }

    fn override_value(self) -> Option<&'static str> {
        match self {
            GoalAnimationState::Idle => None,
            GoalAnimationState::Vanishing => Some(GOAL_ANIMATION_VANISHING_VALUE),
        }
    }

    fn from_override_value(value: &str) -> Self {
        if value.eq_ignore_ascii_case(GOAL_ANIMATION_VANISHING_VALUE) {
            GoalAnimationState::Vanishing
        } else {
            GoalAnimationState::Idle
        }
    }
}

fn goal_animation_override_value(raw_text: &str) -> Option<&str> {
    raw_text.lines().find_map(|line| {
        let trimmed = line.trim_start_matches('\u{feff}').trim();
        trimmed
            .strip_prefix(&format!("String {GOAL_ANIMATION_OVERRIDE_NAME} = "))
            .map(str::trim)
    })
}

fn goal_vanishing_visual_state(time: f64, preview_seed: usize) -> GoalVisualState {
    let duration = goal_vanishing_duration();
    let cycle = duration + DEFAULT_GOAL_VANISH_HOLD;
    let phase = if cycle > 0.0 {
        (preview_seed % 17) as f32 / 17.0 * cycle
    } else {
        0.0
    };
    let sample_time = if cycle > 0.0 {
        ((time as f32) + phase).rem_euclid(cycle).min(duration)
    } else {
        0.0
    };

    let pos_y_curve = goal_vanishing_pos_y_curve();
    let scale_x_curve = goal_vanishing_scale_x_curve();
    let scale_y_curve = goal_vanishing_scale_y_curve();
    let alpha_curve = goal_vanishing_alpha_curve();
    let start_y = pos_y_curve.first().map(|key| key.1).unwrap_or(0.0);
    let start_scale_x = scale_x_curve.first().map(|key| key.1).unwrap_or(1.0).max(0.001);
    let start_scale_y = scale_y_curve.first().map(|key| key.1).unwrap_or(1.0).max(0.001);
    let start_alpha = alpha_curve.first().map(|key| key.1).unwrap_or(1.0).max(0.001);

    GoalVisualState {
        y_offset: sample_hermite(pos_y_curve, sample_time) - start_y,
        scale_x: (sample_hermite(scale_x_curve, sample_time) / start_scale_x).max(0.0),
        scale_y: (sample_hermite(scale_y_curve, sample_time) / start_scale_y).max(0.0),
        alpha: (sample_hermite(alpha_curve, sample_time) / start_alpha).clamp(0.0, 1.0),
    }
}

fn goal_vanishing_duration() -> f32 {
    unity_anim::goal_vanishing_clip()
        .map(|clip| clip.duration)
        .filter(|duration| *duration > 0.0)
        .unwrap_or(DEFAULT_GOAL_VANISH_DURATION)
}

fn goal_vanishing_pos_y_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::goal_vanishing_clip()
        .and_then(|clip| clip.root_position())
        .map(|curve| curve.y.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_GOAL_VANISH_POS_Y)
}

fn goal_vanishing_scale_x_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::goal_vanishing_clip()
        .and_then(|clip| clip.root_scale())
        .map(|curve| curve.x.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_GOAL_VANISH_SCALE_X)
}

fn goal_vanishing_scale_y_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::goal_vanishing_clip()
        .and_then(|clip| clip.root_scale())
        .map(|curve| curve.y.as_slice())
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_GOAL_VANISH_SCALE_Y)
}

fn goal_vanishing_alpha_curve() -> &'static [unity_anim::HermiteKey] {
    unity_anim::goal_vanishing_clip()
        .and_then(|clip| clip.root_float_curve("_Color.a"))
        .filter(|curve| !curve.is_empty())
        .unwrap_or(DEFAULT_GOAL_VANISH_ALPHA)
}

fn sample_hermite(keys: &[unity_anim::HermiteKey], time: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    if time <= keys[0].0 {
        return keys[0].1;
    }

    for window in keys.windows(2) {
        let [(t0, v0, _, out_slope), (t1, v1, in_slope, _)] = window else {
            continue;
        };
        if time > *t1 {
            continue;
        }

        let dt = *t1 - *t0;
        if dt.abs() <= f32::EPSILON {
            return *v1;
        }

        let u = ((time - *t0) / dt).clamp(0.0, 1.0);
        let u2 = u * u;
        let u3 = u2 * u;
        let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
        let h10 = u3 - 2.0 * u2 + u;
        let h01 = -2.0 * u3 + 3.0 * u2;
        let h11 = u3 - u2;
        return h00 * *v0 + h10 * dt * *out_slope + h01 * *v1 + h11 * dt * *in_slope;
    }

    keys.last().map(|key| key.1).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn goal_vanishing_preview_starts_from_rest_pose() {
        let visual = goal_visual_state(GoalAnimationState::Vanishing, 0.0, 0);
        assert_close(visual.y_offset, 0.0);
        assert_close(visual.scale_x, 1.0);
        assert_close(visual.scale_y, 1.0);
        assert_close(visual.alpha, 1.0);
    }

    #[test]
    fn goal_animation_override_round_trips() {
        let mut raw = "Component GoalArea\n".to_string();
        set_goal_animation_state(&mut raw, GoalAnimationState::Vanishing);
        assert_eq!(
            parse_goal_animation_state(Some(&raw)),
            GoalAnimationState::Vanishing
        );

        set_goal_animation_state(&mut raw, GoalAnimationState::Idle);
        assert_eq!(parse_goal_animation_state(Some(&raw)), GoalAnimationState::Idle);
    }
}