use std::sync::OnceLock;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use serde::Deserialize;

use super::assets;

pub type HermiteKey = (f32, f32, f32, f32);

const BIRD_SLEEP2_ASSET: &str = "unity/animation/BirdSleep2.anim";
const ACHIEVEMENT_POPUP_ENTER_ASSET: &str = "unity/animation/AchievementPopupEnter.anim";
const GOAL_VANISHING_ASSET: &str = "unity/animation/GoalVanishing.anim";
const OCEAN_ANIMATION_ASSET: &str = "unity/animation/OceanAnimation.anim";
const OCEAN_FOAM_ANIMATION_ASSET: &str = "unity/animation/OceanFoamAnimation.anim";
const ROTATING_GLOW_ASSET: &str = "unity/animation/RotatingGlow.anim";

static BIRD_SLEEP2_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();
static ACHIEVEMENT_POPUP_ENTER_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();
static GOAL_VANISHING_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();
static OCEAN_ANIMATION_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();
static OCEAN_FOAM_ANIMATION_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();
static ROTATING_GLOW_CLIP: OnceLock<Option<UnityAnimationClip>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct UnityVec3Curves {
    pub x: Vec<HermiteKey>,
    pub y: Vec<HermiteKey>,
    pub z: Vec<HermiteKey>,
}

#[derive(Debug, Clone)]
pub struct UnityVec4Curves {
    pub x: Vec<HermiteKey>,
    pub y: Vec<HermiteKey>,
    pub z: Vec<HermiteKey>,
    pub w: Vec<HermiteKey>,
}

#[derive(Debug, Clone)]
pub struct UnityTransformCurves {
    pub path: String,
    pub keys: UnityVec3Curves,
}

#[derive(Debug, Clone)]
pub struct UnityRotationCurves {
    pub path: String,
    pub keys: UnityVec4Curves,
}

#[derive(Debug, Clone)]
pub struct UnityFloatCurve {
    pub path: String,
    pub attribute: String,
    pub keys: Vec<HermiteKey>,
}

#[derive(Debug, Clone)]
pub struct UnityAnimationClip {
    pub duration: f32,
    pub loops: bool,
    pub rotation_curves: Vec<UnityRotationCurves>,
    pub position_curves: Vec<UnityTransformCurves>,
    pub scale_curves: Vec<UnityTransformCurves>,
    pub float_curves: Vec<UnityFloatCurve>,
}

impl UnityAnimationClip {
    pub fn sample_time(&self, time: f64, phase: f32) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }

        let raw_time = time as f32 + phase;
        if self.loops {
            raw_time.rem_euclid(self.duration)
        } else {
            raw_time.clamp(0.0, self.duration)
        }
    }

    pub fn root_position(&self) -> Option<&UnityVec3Curves> {
        self.position_curves
            .iter()
            .find(|curve| curve.path.is_empty())
            .map(|curve| &curve.keys)
    }

    pub fn root_scale(&self) -> Option<&UnityVec3Curves> {
        self.scale_curves
            .iter()
            .find(|curve| curve.path.is_empty())
            .map(|curve| &curve.keys)
    }

    pub fn root_rotation(&self) -> Option<&UnityVec4Curves> {
        self.rotation_curves
            .iter()
            .find(|curve| curve.path.is_empty())
            .map(|curve| &curve.keys)
    }

    pub fn root_float_curve(&self, attribute: &str) -> Option<&[HermiteKey]> {
        self.float_curves
            .iter()
            .find(|curve| curve.path.is_empty() && curve.attribute == attribute)
            .map(|curve| curve.keys.as_slice())
    }
}

pub fn bird_sleep_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&BIRD_SLEEP2_CLIP, BIRD_SLEEP2_ASSET)
}

pub fn achievement_popup_enter_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&ACHIEVEMENT_POPUP_ENTER_CLIP, ACHIEVEMENT_POPUP_ENTER_ASSET)
}

pub fn goal_vanishing_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&GOAL_VANISHING_CLIP, GOAL_VANISHING_ASSET)
}

pub fn ocean_animation_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&OCEAN_ANIMATION_CLIP, OCEAN_ANIMATION_ASSET)
}

pub fn ocean_foam_animation_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&OCEAN_FOAM_ANIMATION_CLIP, OCEAN_FOAM_ANIMATION_ASSET)
}

pub fn rotating_glow_clip() -> Option<&'static UnityAnimationClip> {
    cached_clip(&ROTATING_GLOW_CLIP, ROTATING_GLOW_ASSET)
}

pub fn load_clip(asset_key: &str) -> Option<UnityAnimationClip> {
    let text = read_animation_text(asset_key)?;
    match parse_clip(&text) {
        Ok(clip) => Some(clip),
        Err(error) => {
            log::error!("Failed to parse Unity animation {}: {}", asset_key, error);
            None
        }
    }
}

pub fn parse_clip(text: &str) -> Result<UnityAnimationClip, serde_yaml::Error> {
    let sanitized = sanitize_unity_yaml(text);
    let document: UnityAnimationDocument = serde_yaml::from_str(&sanitized)?;
    Ok(document.clip.into_runtime())
}

fn cached_clip(
    cache: &'static OnceLock<Option<UnityAnimationClip>>,
    asset_key: &'static str,
) -> Option<&'static UnityAnimationClip> {
    cache.get_or_init(|| load_clip(asset_key)).as_ref()
}

fn sanitize_unity_yaml(text: &str) -> String {
    let mut sanitized = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with("%YAML") || line.starts_with("%TAG") {
            continue;
        }
        if line.starts_with("--- !u!") {
            sanitized.push_str("---\n");
            continue;
        }
        sanitized.push_str(line);
        sanitized.push('\n');
    }
    sanitized
}

fn read_animation_text(asset_key: &str) -> Option<String> {
    if let Some(bytes) = assets::read_asset(asset_key) {
        return Some(String::from_utf8_lossy(bytes.as_ref()).into_owned());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(text) = read_workspace_animation_text(asset_key) {
            return Some(text);
        }
    }

    None
}

#[cfg(not(target_arch = "wasm32"))]
fn read_workspace_animation_text(asset_key: &str) -> Option<String> {
    let filename = asset_key.strip_prefix("unity/animation/")?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("unity_assets/AnimationClip")
        .join(filename);
    std::fs::read_to_string(path).ok()
}

#[derive(Debug, Deserialize)]
struct UnityAnimationDocument {
    #[serde(rename = "AnimationClip")]
    clip: UnityAnimationClipYaml,
}

#[derive(Debug, Deserialize)]
struct UnityAnimationClipYaml {
    #[serde(rename = "m_RotationCurves", default)]
    rotation_curves: Vec<UnityRotationCurveYaml>,
    #[serde(rename = "m_PositionCurves", default)]
    position_curves: Vec<UnityTransformCurveYaml>,
    #[serde(rename = "m_ScaleCurves", default)]
    scale_curves: Vec<UnityTransformCurveYaml>,
    #[serde(rename = "m_FloatCurves", default)]
    float_curves: Vec<UnityFloatCurveYaml>,
    #[serde(rename = "m_WrapMode", default)]
    wrap_mode: i32,
    #[serde(rename = "m_AnimationClipSettings", default)]
    settings: UnityAnimationClipSettingsYaml,
}

impl UnityAnimationClipYaml {
    fn into_runtime(self) -> UnityAnimationClip {
        let rotation_curves: Vec<UnityRotationCurves> = self
            .rotation_curves
            .into_iter()
            .map(UnityRotationCurveYaml::into_runtime)
            .collect();
        let position_curves: Vec<UnityTransformCurves> = self
            .position_curves
            .into_iter()
            .map(UnityTransformCurveYaml::into_runtime)
            .collect();
        let scale_curves: Vec<UnityTransformCurves> = self
            .scale_curves
            .into_iter()
            .map(UnityTransformCurveYaml::into_runtime)
            .collect();
        let float_curves: Vec<UnityFloatCurve> = self
            .float_curves
            .into_iter()
            .map(UnityFloatCurveYaml::into_runtime)
            .collect();
        let duration = rotation_curves
            .iter()
            .flat_map(|curve| {
                curve
                    .keys
                    .x
                    .iter()
                    .chain(curve.keys.y.iter())
                    .chain(curve.keys.z.iter())
                    .chain(curve.keys.w.iter())
            })
            .chain(position_curves
                .iter()
            .chain(scale_curves.iter())
            .flat_map(|curve| {
                curve
                    .keys
                    .x
                    .iter()
                    .chain(curve.keys.y.iter())
                    .chain(curve.keys.z.iter())
            }))
            .chain(float_curves.iter().flat_map(|curve| curve.keys.iter()))
            .fold(0.0_f32, |max_time, key| max_time.max(key.0));

        UnityAnimationClip {
            duration,
            loops: self.wrap_mode == 2 || self.settings.loop_time != 0,
            rotation_curves,
            position_curves,
            scale_curves,
            float_curves,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct UnityAnimationClipSettingsYaml {
    #[serde(rename = "m_LoopTime", default)]
    loop_time: i32,
}

#[derive(Debug, Deserialize)]
struct UnityTransformCurveYaml {
    curve: UnityCurveYaml,
    #[serde(default)]
    path: Option<String>,
}

impl UnityTransformCurveYaml {
    fn into_runtime(self) -> UnityTransformCurves {
        let keys = self.curve.keys;
        UnityTransformCurves {
            path: self.path.unwrap_or_default(),
            keys: UnityVec3Curves {
                x: keys
                    .iter()
                    .map(|key| (key.time, key.value.x, key.in_slope.x, key.out_slope.x))
                    .collect(),
                y: keys
                    .iter()
                    .map(|key| (key.time, key.value.y, key.in_slope.y, key.out_slope.y))
                    .collect(),
                z: keys
                    .iter()
                    .map(|key| (key.time, key.value.z, key.in_slope.z, key.out_slope.z))
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct UnityRotationCurveYaml {
    curve: UnityQuatCurveYaml,
    #[serde(default)]
    path: Option<String>,
}

impl UnityRotationCurveYaml {
    fn into_runtime(self) -> UnityRotationCurves {
        let keys = self.curve.keys;
        UnityRotationCurves {
            path: self.path.unwrap_or_default(),
            keys: UnityVec4Curves {
                x: keys
                    .iter()
                    .map(|key| (key.time, key.value.x, key.in_slope.x, key.out_slope.x))
                    .collect(),
                y: keys
                    .iter()
                    .map(|key| (key.time, key.value.y, key.in_slope.y, key.out_slope.y))
                    .collect(),
                z: keys
                    .iter()
                    .map(|key| (key.time, key.value.z, key.in_slope.z, key.out_slope.z))
                    .collect(),
                w: keys
                    .iter()
                    .map(|key| (key.time, key.value.w, key.in_slope.w, key.out_slope.w))
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct UnityFloatCurveYaml {
    curve: UnityScalarCurveYaml,
    #[serde(default)]
    attribute: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

impl UnityFloatCurveYaml {
    fn into_runtime(self) -> UnityFloatCurve {
        UnityFloatCurve {
            path: self.path.unwrap_or_default(),
            attribute: self.attribute.unwrap_or_default(),
            keys: self
                .curve
                .keys
                .into_iter()
                .map(|key| (key.time, key.value, key.in_slope, key.out_slope))
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UnityCurveYaml {
    #[serde(rename = "m_Curve", default)]
    keys: Vec<UnityCurveKeyYaml>,
}

#[derive(Debug, Deserialize)]
struct UnityQuatCurveYaml {
    #[serde(rename = "m_Curve", default)]
    keys: Vec<UnityQuatCurveKeyYaml>,
}

#[derive(Debug, Deserialize)]
struct UnityScalarCurveYaml {
    #[serde(rename = "m_Curve", default)]
    keys: Vec<UnityScalarCurveKeyYaml>,
}

#[derive(Debug, Deserialize)]
struct UnityCurveKeyYaml {
    time: f32,
    value: UnityVec3Yaml,
    #[serde(rename = "inSlope")]
    in_slope: UnityVec3Yaml,
    #[serde(rename = "outSlope")]
    out_slope: UnityVec3Yaml,
}

#[derive(Debug, Deserialize)]
struct UnityQuatCurveKeyYaml {
    time: f32,
    value: UnityVec4Yaml,
    #[serde(rename = "inSlope")]
    in_slope: UnityVec4Yaml,
    #[serde(rename = "outSlope")]
    out_slope: UnityVec4Yaml,
}

#[derive(Debug, Deserialize)]
struct UnityScalarCurveKeyYaml {
    time: f32,
    value: f32,
    #[serde(rename = "inSlope")]
    in_slope: f32,
    #[serde(rename = "outSlope")]
    out_slope: f32,
}

#[derive(Debug, Deserialize)]
struct UnityVec3Yaml {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Deserialize)]
struct UnityVec4Yaml {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn parses_bird_sleep2_root_curves() {
        let clip = parse_clip(include_str!("../../unity_assets/AnimationClip/BirdSleep2.anim"))
            .expect("BirdSleep2.anim should parse");
        let position = clip.root_position().expect("root position curve should exist");
        let scale = clip.root_scale().expect("root scale curve should exist");

        assert!(clip.loops);
        assert_close(clip.duration, 4.0);
        assert_eq!(position.y.len(), 3);
        assert_eq!(scale.x.len(), 3);
        assert_close(position.y[1].0, 1.833333);
        assert_close(position.y[1].1, -0.061);
        assert_close(position.y[1].2, -0.00255944);
        assert_close(position.y[1].3, -0.00255944);
        assert_close(scale.x[1].1, 1.1);
        assert_close(scale.y[1].1, 0.9);
    }

    #[test]
    fn bird_sleep_clip_loads_from_asset_pipeline() {
        assert!(
            assets::read_asset("unity/animation/BirdSleep2.anim").is_some(),
            "embedded BirdSleep2.anim should exist"
        );

        let clip = bird_sleep_clip().expect("bird sleep clip should load from assets");
        assert_close(clip.duration, 4.0);
        assert!(clip.loops);
    }

    #[test]
    fn parses_achievement_popup_enter_position_curve() {
        let clip =
            parse_clip(include_str!("../../unity_assets/AnimationClip/AchievementPopupEnter.anim"))
            .expect("AchievementPopupEnter.anim should parse");
        let position = clip.root_position().expect("root position curve should exist");

        assert!(!clip.loops);
        assert_eq!(position.y.len(), 4);
        assert_close(position.y[0].1, 13.0);
        assert_close(position.y[1].1, 8.5);
        assert_close(position.y[3].0, 2.666667);
    }

    #[test]
    fn parses_goal_vanishing_position_scale_and_alpha_curves() {
        let clip = parse_clip(include_str!("../../unity_assets/AnimationClip/GoalVanishing.anim"))
            .expect("GoalVanishing.anim should parse");
        let position = clip.root_position().expect("root position curve should exist");
        let scale = clip.root_scale().expect("root scale curve should exist");
        let alpha = clip
            .root_float_curve("_Color.a")
            .expect("goal alpha curve should exist");

        assert!(!clip.loops);
        assert_close(clip.duration, 1.0);
        assert_eq!(position.y.len(), 3);
        assert_eq!(scale.x.len(), 3);
        assert_eq!(alpha.len(), 3);
        assert_close(position.y[2].1, 13.51);
        assert_close(scale.x[1].1, 0.25);
        assert_close(scale.y[1].1, 1.5);
        assert_close(alpha[2].1, 0.0);
    }

    #[test]
    fn parses_rotating_glow_rotation_and_float_curves() {
        let clip = parse_clip(include_str!("../../unity_assets/AnimationClip/RotatingGlow.anim"))
            .expect("RotatingGlow.anim should parse");
        let rotation = clip.root_rotation().expect("root rotation curve should exist");
        let euler_z = clip
            .root_float_curve("m_LocalEulerAnglesHint.z")
            .expect("root float z curve should exist");

        assert!(clip.loops);
        assert_close(clip.duration, 5.0);
        assert_eq!(rotation.z.len(), 8);
        assert_eq!(rotation.w.len(), 8);
        assert_eq!(euler_z.len(), 28);
        assert_close(rotation.z[0].1, -3.258414e-07);
        assert_close(rotation.w[7].1, -0.7071069);
        assert_close(euler_z[0].1, 360.0);
        assert_close(euler_z[27].1, 270.0);
    }
}
