//! Override parsers for compound prefab components (Bridge, Fan).
//!
//! Parses tab-indented ObjectDeserializer format to extract field values.

pub(super) struct BridgeOverrides {
    pub end_point_x: Option<f32>,
    pub end_point_y: Option<f32>,
    pub step_length: Option<f32>,
    pub step_gap: Option<f32>,
}

/// Parse Bridge component overrides from tab-indented ObjectDeserializer format.
pub(super) fn parse_bridge_overrides(raw_text: Option<&str>) -> BridgeOverrides {
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

pub(super) struct FanOverrides {
    pub target_force: Option<f32>,
    pub start_time: Option<f32>,
    pub on_time: Option<f32>,
    pub off_time: Option<f32>,
    pub delayed_start: Option<f32>,
    pub always_on: Option<bool>,
}

/// Parse Fan component overrides from tab-indented ObjectDeserializer format.
pub(super) fn parse_fan_overrides(raw_text: Option<&str>) -> FanOverrides {
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
