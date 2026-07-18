//! Override parsers for compound prefab components (Bridge, Fan).
//!
//! Drives the typed [`Scene`] via `ObjectDeserializer` and reads typed field
//! values back through component downcasts — mirroring Unity's runtime
//! `ApplyOverrides` + reflection pipeline.

use std::sync::OnceLock;

use crate::data::assets;
use crate::domain::prefab_asset::PrefabAssetDocument;
use crate::unity_runtime::components::{Bridge, Fan, Transform};
use crate::unity_runtime::scene::Scene;

const BRIDGE_PREFAB_ASSET: &str = "Assets/Prefab/Bridge.prefab";
const FAN_PREFAB_ASSET: &str = "Assets/Prefab/Fan.prefab";

#[derive(Debug, Clone, Copy, PartialEq)]
struct BridgePrefabDefaults {
    step_length: f32,
    step_gap: f32,
    end_point_x: f32,
    end_point_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FanPrefabDefaults {
    target_force: f32,
    start_time: f32,
    on_time: f32,
    off_time: f32,
    delayed_start: f32,
    always_on: bool,
}

pub(super) struct BridgeOverrides {
    pub end_point_x: Option<f32>,
    pub end_point_y: Option<f32>,
    pub step_length: Option<f32>,
    pub step_gap: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BridgeRuntimeProjection {
    pub step_length: f32,
    pub step_gap: f32,
    pub raw_end_point_x: f32,
    pub raw_end_point_y: f32,
    pub runtime_end_point_x: f32,
    pub runtime_end_point_y: f32,
    pub stride: f32,
    pub step_count: i32,
    pub angle: f32,
}

/// Parse Bridge component overrides via the typed [`Scene`] pipeline.
pub(super) fn parse_bridge_overrides(raw_text: Option<&str>) -> BridgeOverrides {
    let mut result = BridgeOverrides {
        end_point_x: None,
        end_point_y: None,
        step_length: None,
        step_gap: None,
    };
    let Some(text) = raw_text else {
        return result;
    };
    let Some((scene, root)) = Scene::from_override_text(text) else {
        return result;
    };

    if let Some((_, bridge)) = scene.get_component_of::<Bridge>(root) {
        result.step_length = bridge.step_length;
        result.step_gap = bridge.step_gap;
    }

    if let Some(endpoint) = scene.find_child(root, "EndPoint")
        && let Some((_, transform)) = scene.get_component_of::<Transform>(endpoint)
        && let Some(pos) = transform.local_position
    {
        result.end_point_x = Some(pos.x);
        result.end_point_y = Some(pos.y);
    }

    result
}

pub(super) fn project_bridge_runtime(raw_text: Option<&str>) -> BridgeRuntimeProjection {
    let defaults = bridge_prefab_defaults();
    let overrides = parse_bridge_overrides(raw_text);

    let step_length = overrides.step_length.unwrap_or(defaults.step_length);
    let step_gap = overrides.step_gap.unwrap_or(defaults.step_gap);
    let raw_end_point_x = overrides.end_point_x.unwrap_or(defaults.end_point_x);
    let raw_end_point_y = overrides.end_point_y.unwrap_or(defaults.end_point_y);

    build_bridge_runtime_projection(step_length, step_gap, raw_end_point_x, raw_end_point_y)
}

// (Legacy AST-based hook implementation removed: the typed `Scene` pipeline
// now drives Bridge override parsing directly. See `parse_bridge_overrides`.)
fn bridge_prefab_defaults() -> BridgePrefabDefaults {
    static DEFAULTS: OnceLock<BridgePrefabDefaults> = OnceLock::new();

    *DEFAULTS.get_or_init(load_bridge_prefab_defaults)
}

fn load_bridge_prefab_defaults() -> BridgePrefabDefaults {
    let text = assets::read_pathname_text(BRIDGE_PREFAB_ASSET)
        .unwrap_or_else(|| panic!("missing embedded prefab {BRIDGE_PREFAB_ASSET}"));
    let prefab = PrefabAssetDocument::parse(&text)
        .unwrap_or_else(|| panic!("failed to parse embedded prefab {BRIDGE_PREFAB_ASSET}"));
    let component = prefab
        .root_component("Bridge")
        .unwrap_or_else(|| panic!("missing Bridge component in {BRIDGE_PREFAB_ASSET}"));
    let step_length = component
        .field_f32("stepLength")
        .unwrap_or_else(|| panic!("missing Bridge.stepLength in {BRIDGE_PREFAB_ASSET}"));
    let step_gap = component
        .field_f32("stepGap")
        .unwrap_or_else(|| panic!("missing Bridge.stepGap in {BRIDGE_PREFAB_ASSET}"));
    let Some(end_point_position) = prefab
        .transform_by_game_object_name("EndPoint")
        .map(|transform| transform.local_pos)
    else {
        panic!("missing EndPoint transform in {BRIDGE_PREFAB_ASSET}");
    };

    BridgePrefabDefaults {
        step_length,
        step_gap,
        end_point_x: end_point_position[0],
        end_point_y: end_point_position[1],
    }
}

fn build_bridge_runtime_projection(
    step_length: f32,
    step_gap: f32,
    raw_end_point_x: f32,
    raw_end_point_y: f32,
) -> BridgeRuntimeProjection {
    let stride = step_length + step_gap;
    let raw_distance =
        (raw_end_point_x * raw_end_point_x + raw_end_point_y * raw_end_point_y).sqrt();
    let step_count = if stride > f32::EPSILON {
        (raw_distance / stride).floor().max(0.0) as i32
    } else {
        0
    };
    let angle = raw_end_point_y.atan2(raw_end_point_x);
    let runtime_distance = if step_count > 0 {
        step_count as f32 * stride + step_gap * 0.5
    } else {
        raw_distance
    };

    let (runtime_end_point_x, runtime_end_point_y) = if raw_distance > f32::EPSILON {
        (
            runtime_distance * angle.cos(),
            runtime_distance * angle.sin(),
        )
    } else {
        (raw_end_point_x, raw_end_point_y)
    };

    BridgeRuntimeProjection {
        step_length,
        step_gap,
        raw_end_point_x,
        raw_end_point_y,
        runtime_end_point_x,
        runtime_end_point_y,
        stride,
        step_count,
        angle,
    }
}

pub(super) struct FanOverrides {
    pub target_force: Option<f32>,
    pub start_time: Option<f32>,
    pub on_time: Option<f32>,
    pub off_time: Option<f32>,
    pub delayed_start: Option<f32>,
    pub always_on: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FanRuntimeConfig {
    pub target_force: f32,
    pub start_time: f32,
    pub on_time: f32,
    pub off_time: f32,
    pub delayed_start: f32,
    pub always_on: bool,
}

/// Parse Fan component overrides via the typed [`Scene`] pipeline.
pub(super) fn parse_fan_overrides(raw_text: Option<&str>) -> FanOverrides {
    let empty = FanOverrides {
        target_force: None,
        start_time: None,
        on_time: None,
        off_time: None,
        delayed_start: None,
        always_on: None,
    };
    let Some(text) = raw_text else {
        return empty;
    };
    let Some((scene, root)) = Scene::from_override_text(text) else {
        return empty;
    };
    let Some((_, fan)) = scene.get_component_of::<Fan>(root) else {
        return empty;
    };

    FanOverrides {
        target_force: fan.target_force,
        start_time: fan.start_time,
        on_time: fan.on_time,
        off_time: fan.off_time,
        delayed_start: fan.delayed_start,
        always_on: fan.always_on,
    }
}

pub(super) fn project_fan_runtime(raw_text: Option<&str>) -> FanRuntimeConfig {
    let defaults = fan_prefab_defaults();
    let overrides = parse_fan_overrides(raw_text);
    let on_time = overrides.on_time.unwrap_or(defaults.on_time);

    FanRuntimeConfig {
        target_force: overrides.target_force.unwrap_or(defaults.target_force),
        start_time: overrides.start_time.unwrap_or(defaults.start_time),
        on_time,
        off_time: overrides.off_time.unwrap_or(defaults.off_time),
        delayed_start: overrides.delayed_start.unwrap_or(defaults.delayed_start) + on_time,
        always_on: overrides.always_on.unwrap_or(defaults.always_on),
    }
}

fn fan_prefab_defaults() -> FanPrefabDefaults {
    static DEFAULTS: OnceLock<FanPrefabDefaults> = OnceLock::new();

    *DEFAULTS.get_or_init(load_fan_prefab_defaults)
}

fn load_fan_prefab_defaults() -> FanPrefabDefaults {
    let text = assets::read_pathname_text(FAN_PREFAB_ASSET)
        .unwrap_or_else(|| panic!("missing embedded prefab {FAN_PREFAB_ASSET}"));
    let prefab = PrefabAssetDocument::parse(&text)
        .unwrap_or_else(|| panic!("failed to parse embedded prefab {FAN_PREFAB_ASSET}"));
    let component = prefab
        .root_component("Fan")
        .unwrap_or_else(|| panic!("missing Fan component in {FAN_PREFAB_ASSET}"));
    let target_force = component
        .field_f32("targetForce")
        .unwrap_or_else(|| panic!("missing Fan.targetForce in {FAN_PREFAB_ASSET}"));
    let start_time = component
        .field_f32("startTime")
        .unwrap_or_else(|| panic!("missing Fan.startTime in {FAN_PREFAB_ASSET}"));
    let on_time = component
        .field_f32("onTime")
        .unwrap_or_else(|| panic!("missing Fan.onTime in {FAN_PREFAB_ASSET}"));
    let off_time = component
        .field_f32("offTime")
        .unwrap_or_else(|| panic!("missing Fan.offTime in {FAN_PREFAB_ASSET}"));
    let delayed_start = component
        .field_f32("delayedStart")
        .unwrap_or_else(|| panic!("missing Fan.delayedStart in {FAN_PREFAB_ASSET}"));
    let always_on = component
        .field_bool("alwaysOn")
        .unwrap_or_else(|| panic!("missing Fan.alwaysOn in {FAN_PREFAB_ASSET}"));

    FanPrefabDefaults {
        target_force,
        start_time,
        on_time,
        off_time,
        delayed_start,
        always_on,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_bridge_overrides, parse_fan_overrides, project_bridge_runtime, project_fan_runtime,
    };

    const BRIDGE_OVERRIDE: &str = "GameObject Bridge\n\tComponent Bridge\n\t\tFloat stepLength = 1.25\n\t\tFloat stepGap = 0.3\n\tGameObject EndPoint\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = 4.5\n\t\t\t\tFloat y = -1.25\n";

    const BRIDGE_ENDPOINT_ONLY_OVERRIDE: &str = "GameObject Bridge\n\tGameObject EndPoint\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = 5.5\n\t\t\t\tFloat y = 1.25\n";

    const FAN_OVERRIDE: &str = "GameObject Fan\n\tComponent Fan\n\t\tFloat targetForce = 12.5\n\t\tFloat startTime = 1\n\t\tFloat onTime = 2\n\t\tFloat offTime = 3\n\t\tFloat delayedStart = 4\n\t\tBoolean alwaysOn = True\n";

    #[test]
    fn bridge_override_parser_reads_endpoint_and_stride_values() {
        let overrides = parse_bridge_overrides(Some(BRIDGE_OVERRIDE));
        assert_eq!(overrides.end_point_x, Some(4.5));
        assert_eq!(overrides.end_point_y, Some(-1.25));
        assert_eq!(overrides.step_length, Some(1.25));
        assert_eq!(overrides.step_gap, Some(0.3));
    }

    #[test]
    fn bridge_override_parser_still_runs_when_only_endpoint_is_overridden() {
        let overrides = parse_bridge_overrides(Some(BRIDGE_ENDPOINT_ONLY_OVERRIDE));

        assert_eq!(overrides.end_point_x, Some(5.5));
        assert_eq!(overrides.end_point_y, Some(1.25));
        assert_eq!(overrides.step_length, None);
        assert_eq!(overrides.step_gap, None);
    }

    #[test]
    fn bridge_runtime_projection_uses_embedded_prefab_defaults() {
        let runtime = project_bridge_runtime(None);

        assert_eq!(runtime.step_length, 1.0);
        assert_eq!(runtime.step_gap, 0.2);
        assert!((runtime.raw_end_point_x - 2.561546).abs() < 1e-6);
        assert!(runtime.raw_end_point_y.abs() < 1e-6);
        assert_eq!(runtime.step_count, 2);
        assert!((runtime.runtime_end_point_x - 2.5).abs() < 1e-6);
        assert!(runtime.runtime_end_point_y.abs() < 1e-6);
    }

    #[test]
    fn bridge_runtime_projection_repositions_endpoint_like_unity_on_data_loaded() {
        let runtime = project_bridge_runtime(Some(BRIDGE_OVERRIDE));
        let runtime_distance = (runtime.runtime_end_point_x * runtime.runtime_end_point_x
            + runtime.runtime_end_point_y * runtime.runtime_end_point_y)
            .sqrt();

        assert_eq!(runtime.step_count, 3);
        assert!((runtime.stride - 1.55).abs() < 1e-6);
        assert!((runtime_distance - 4.8).abs() < 1e-5);
        assert!((runtime.angle - (-1.25f32).atan2(4.5)).abs() < 1e-6);
    }

    #[test]
    fn fan_override_parser_reads_component_fields() {
        let overrides = parse_fan_overrides(Some(FAN_OVERRIDE));
        assert_eq!(overrides.target_force, Some(12.5));
        assert_eq!(overrides.start_time, Some(1.0));
        assert_eq!(overrides.on_time, Some(2.0));
        assert_eq!(overrides.off_time, Some(3.0));
        assert_eq!(overrides.delayed_start, Some(4.0));
        assert_eq!(overrides.always_on, Some(true));
    }

    #[test]
    fn fan_runtime_projection_uses_embedded_prefab_defaults() {
        let runtime = project_fan_runtime(None);

        assert_eq!(runtime.target_force, 115.0);
        assert_eq!(runtime.start_time, 2.0);
        assert_eq!(runtime.on_time, 4.0);
        assert_eq!(runtime.off_time, 2.0);
        assert_eq!(runtime.delayed_start, 5.0);
        assert!(runtime.always_on);
    }

    #[test]
    fn fan_runtime_projection_merges_overrides_and_unity_init_values() {
        let runtime = project_fan_runtime(Some(FAN_OVERRIDE));

        assert_eq!(runtime.target_force, 12.5);
        assert_eq!(runtime.start_time, 1.0);
        assert_eq!(runtime.on_time, 2.0);
        assert_eq!(runtime.off_time, 3.0);
        assert_eq!(runtime.delayed_start, 6.0);
        assert!(runtime.always_on);
    }
}
