use crate::data::unity_anim;
use crate::domain::prefab_override::{
    OverrideNode, find_first_node_mut, parse_override_text, serialize_override_tree,
};
use crate::unity_runtime::scene::{Scene, SceneValue};

pub const GOAL_ANIMATION_OVERRIDE_NAME: &str = "bp_goalAnimation";
const GOAL_ANIMATION_IDLE_VALUE: &str = "Idle";
const GOAL_ANIMATION_VANISHING_VALUE: &str = "Vanishing";
const DEFAULT_GOAL_VANISH_HOLD: f32 = 0.35;

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
        .as_deref()
        .map(GoalAnimationState::from_override_value)
        .unwrap_or(GoalAnimationState::Idle)
}

pub fn set_goal_animation_state(raw_text: &mut String, state: GoalAnimationState) {
    let mut nodes = parse_override_text(raw_text);

    match state.override_value() {
        Some(value) => {
            if let Some(node) = find_first_node_mut(&mut nodes, &is_goal_animation_node) {
                node.value = Some(value.to_string());
            } else {
                let new_node = OverrideNode {
                    node_type: "String".to_string(),
                    name: GOAL_ANIMATION_OVERRIDE_NAME.to_string(),
                    value: Some(value.to_string()),
                    children: Vec::new(),
                };
                if let Some(component) = find_first_node_mut(&mut nodes, &is_component_node) {
                    component.children.push(new_node);
                } else {
                    nodes.push(new_node);
                }
            }
        }
        None => remove_goal_animation_nodes(&mut nodes),
    }

    *raw_text = serialize_override_tree(&nodes);
}

pub fn goal_visual_state(
    state: GoalAnimationState,
    time: f64,
    preview_seed: usize,
) -> GoalVisualState {
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
            GoalAnimationState::Vanishing => Some("\"Vanishing\""),
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

fn goal_animation_override_value(raw_text: &str) -> Option<String> {
    let (scene, _root) = Scene::from_override_text(raw_text)?;
    for (_, c) in scene.iter_components() {
        if let Some(value) = find_string_field(c.behavior.extra(), GOAL_ANIMATION_OVERRIDE_NAME) {
            return Some(value);
        }
    }
    None
}

fn find_string_field(entries: &[(String, SceneValue)], field_name: &str) -> Option<String> {
    for (name, value) in entries {
        if name == field_name
            && let SceneValue::String(s) = value
        {
            return Some(s.clone());
        }
        if let SceneValue::Generic(inner) = value
            && let Some(s) = find_string_field(inner, field_name)
        {
            return Some(s);
        }
    }
    None
}

fn is_goal_animation_node(node: &OverrideNode) -> bool {
    node.node_type == "String" && node.name == GOAL_ANIMATION_OVERRIDE_NAME
}

fn is_component_node(node: &OverrideNode) -> bool {
    node.node_type == "Component"
}

fn remove_goal_animation_nodes(nodes: &mut Vec<OverrideNode>) {
    nodes.retain(|node| !is_goal_animation_node(node));
    for node in nodes {
        remove_goal_animation_nodes(&mut node.children);
    }
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
    let start_scale_x = scale_x_curve
        .first()
        .map(|key| key.1)
        .unwrap_or(1.0)
        .max(0.001);
    let start_scale_y = scale_y_curve
        .first()
        .map(|key| key.1)
        .unwrap_or(1.0)
        .max(0.001);
    let start_alpha = alpha_curve
        .first()
        .map(|key| key.1)
        .unwrap_or(1.0)
        .max(0.001);

    GoalVisualState {
        y_offset: sample_hermite(pos_y_curve, sample_time) - start_y,
        scale_x: (sample_hermite(scale_x_curve, sample_time) / start_scale_x).max(0.0),
        scale_y: (sample_hermite(scale_y_curve, sample_time) / start_scale_y).max(0.0),
        alpha: (sample_hermite(alpha_curve, sample_time) / start_alpha).clamp(0.0, 1.0),
    }
}

fn goal_vanishing_duration() -> f32 {
    let duration = unity_anim::goal_vanishing_clip()
        .expect("GoalVanishing.anim should load from embedded assets")
        .duration;
    if duration <= 0.0 {
        panic!("GoalVanishing.anim must have positive duration");
    }
    duration
}

fn goal_vanishing_pos_y_curve() -> &'static [unity_anim::HermiteKey] {
    let curve = unity_anim::goal_vanishing_clip()
        .expect("GoalVanishing.anim should load from embedded assets")
        .root_position()
        .expect("GoalVanishing.anim must include root position curves")
        .y
        .as_slice();
    if curve.is_empty() {
        panic!("GoalVanishing.anim root position Y curve must not be empty");
    }
    curve
}

fn goal_vanishing_scale_x_curve() -> &'static [unity_anim::HermiteKey] {
    let curve = unity_anim::goal_vanishing_clip()
        .expect("GoalVanishing.anim should load from embedded assets")
        .root_scale()
        .expect("GoalVanishing.anim must include root scale curves")
        .x
        .as_slice();
    if curve.is_empty() {
        panic!("GoalVanishing.anim root scale X curve must not be empty");
    }
    curve
}

fn goal_vanishing_scale_y_curve() -> &'static [unity_anim::HermiteKey] {
    let curve = unity_anim::goal_vanishing_clip()
        .expect("GoalVanishing.anim should load from embedded assets")
        .root_scale()
        .expect("GoalVanishing.anim must include root scale curves")
        .y
        .as_slice();
    if curve.is_empty() {
        panic!("GoalVanishing.anim root scale Y curve must not be empty");
    }
    curve
}

fn goal_vanishing_alpha_curve() -> &'static [unity_anim::HermiteKey] {
    let curve = unity_anim::goal_vanishing_clip()
        .expect("GoalVanishing.anim should load from embedded assets")
        .root_float_curve("_Color.a")
        .expect("GoalVanishing.anim must include root alpha curve");
    if curve.is_empty() {
        panic!("GoalVanishing.anim root alpha curve must not be empty");
    }
    curve
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
        assert_eq!(
            parse_goal_animation_state(Some(&raw)),
            GoalAnimationState::Idle
        );
    }

    #[test]
    fn goal_animation_ast_parser_reads_and_removes_nested_override() {
        let mut raw = "Component GoalArea\n\tString bp_goalAnimation = \"Vanishing\"\n".to_string();

        assert_eq!(
            parse_goal_animation_state(Some(&raw)),
            GoalAnimationState::Vanishing
        );

        set_goal_animation_state(&mut raw, GoalAnimationState::Idle);

        assert_eq!(raw, "Component GoalArea\n");
        assert_eq!(
            parse_goal_animation_state(Some(&raw)),
            GoalAnimationState::Idle
        );
    }
}
