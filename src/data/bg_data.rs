//! Background theme data — loads bg sprite data from embedded TOML.
//!
//! Each theme has multiple sprite entries organized into parallax layers.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::diagnostics::error::{AppError, AppResult};

/// Parallax layer with a speed factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BgLayer {
    Sky,        // speed 0.8
    Camera,     // speed 1.0
    Further,    // speed 0.7
    Far,        // speed 0.6
    Near,       // speed 0.4
    Ground,     // speed 0.0
    Foreground, // speed -0.4
}

impl BgLayer {
    pub fn parallax_speed(&self) -> f32 {
        match self {
            BgLayer::Sky => 0.8,
            BgLayer::Camera => 1.0,
            BgLayer::Further => 0.7,
            BgLayer::Far => 0.6,
            BgLayer::Near => 0.4,
            BgLayer::Ground => 0.0,
            BgLayer::Foreground => -0.4,
        }
    }

    /// Render order (lower = drawn first = further back).
    pub fn order(&self) -> i32 {
        match self {
            BgLayer::Sky => 0,
            BgLayer::Camera => 1,
            BgLayer::Further => 2,
            BgLayer::Far => 3,
            BgLayer::Near => 4,
            BgLayer::Ground => 5,
            BgLayer::Foreground => 20,
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "sky" => Some(BgLayer::Sky),
            "camera" => Some(BgLayer::Camera),
            "further" => Some(BgLayer::Further),
            "far" => Some(BgLayer::Far),
            "near" => Some(BgLayer::Near),
            "ground" => Some(BgLayer::Ground),
            "foreground" => Some(BgLayer::Foreground),
            _ => None,
        }
    }
}

/// A background sprite ready for rendering.
#[derive(Debug, Clone)]
pub struct BgSprite {
    pub name: String,
    pub atlas: Option<String>,
    pub fill_color: Option<[u8; 3]>,
    pub sky_texture: Option<String>,
    pub uv_x: f32,
    pub uv_y: f32,
    pub grid_w: f32,
    pub grid_h: f32,
    pub sprite_w: f32,
    pub sprite_h: f32,
    pub subdiv: f32,
    pub border: f32,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub layer: BgLayer,
    pub local_x: f32,
    pub local_y: f32,
    pub parent_group: String,
    pub tint: [f32; 4],
    pub alpha_blend: bool,
}

/// Full theme data with sprites organized by layer.
#[derive(Debug, Clone)]
pub struct BgTheme {
    pub sprites: Vec<BgSprite>,
    pub group_defaults: HashMap<String, [f32; 3]>,
    /// PositionSerializer child root-order → group name mapping.
    /// When present, per-level `childLocalPositions` arrays are applied as
    /// group position overrides (EP6 background prefabs).
    pub child_order: Vec<String>,
}

// ── TOML deserialization ──

#[derive(Deserialize)]
struct ThemeToml {
    sprites: Vec<SpriteToml>,
    #[serde(rename = "groupDefaults")]
    group_defaults: HashMap<String, [f64; 3]>,
    #[serde(rename = "childOrder", default)]
    child_order: Vec<String>,
}

#[derive(Deserialize)]
struct SpriteToml {
    #[serde(rename = "name")]
    _name: String,
    atlas: Option<String>,
    #[serde(rename = "fillColor")]
    fill_color: Option<String>,
    #[serde(rename = "skyTexture")]
    sky_texture: Option<String>,
    #[serde(rename = "uvX")]
    uv_x: f64,
    #[serde(rename = "uvY")]
    uv_y: f64,
    #[serde(rename = "gridW")]
    grid_w: f64,
    #[serde(rename = "gridH")]
    grid_h: f64,
    #[serde(rename = "spriteW")]
    sprite_w: f64,
    #[serde(rename = "spriteH")]
    sprite_h: f64,
    subdiv: f64,
    border: f64,
    #[serde(rename = "worldX")]
    world_x: f64,
    #[serde(rename = "worldY")]
    world_y: f64,
    #[serde(rename = "worldZ")]
    world_z: f64,
    #[serde(rename = "worldScaleX")]
    world_scale_x: f64,
    #[serde(rename = "worldScaleY")]
    world_scale_y: f64,
    layer: String,
    #[serde(rename = "localX", default)]
    local_x: f64,
    #[serde(rename = "localY", default)]
    local_y: f64,
    #[serde(rename = "parentGroup", default)]
    parent_group: String,
    tint: Option<String>,
    #[serde(rename = "alphaBlend", default)]
    alpha_blend: bool,
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r, g, b])
}

static BG_THEMES: OnceLock<HashMap<String, BgTheme>> = OnceLock::new();

fn build_themes() -> HashMap<String, BgTheme> {
    let raw = match try_load_themes() {
        Ok(raw) => raw,
        Err(error) => {
            log::error!("Failed to load background themes: {error}");
            return HashMap::new();
        }
    };

    let mut themes = HashMap::with_capacity(raw.len());
    for (name, theme_toml) in raw {
        let mut sprites = Vec::with_capacity(theme_toml.sprites.len());
        for s in &theme_toml.sprites {
            let layer = match BgLayer::from_str(&s.layer) {
                Some(l) => l,
                None => continue,
            };
            sprites.push(BgSprite {
                name: s._name.clone(),
                atlas: s.atlas.clone(),
                fill_color: s.fill_color.as_deref().and_then(parse_hex_color),
                sky_texture: s.sky_texture.clone(),
                uv_x: s.uv_x as f32,
                uv_y: s.uv_y as f32,
                grid_w: s.grid_w as f32,
                grid_h: s.grid_h as f32,
                sprite_w: s.sprite_w as f32,
                sprite_h: s.sprite_h as f32,
                subdiv: s.subdiv as f32,
                border: s.border as f32,
                world_x: s.world_x as f32,
                world_y: s.world_y as f32,
                world_z: s.world_z as f32,
                scale_x: s.world_scale_x as f32,
                scale_y: s.world_scale_y as f32,
                layer,
                local_x: s.local_x as f32,
                local_y: s.local_y as f32,
                parent_group: s.parent_group.clone(),
                tint: s
                    .tint
                    .as_deref()
                    .and_then(parse_hex_color)
                    .map(|[r, g, b]| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
                    .unwrap_or([1.0, 1.0, 1.0, 1.0]),
                alpha_blend: s.alpha_blend,
            });
        }
        // Sort by layer order then by Z (farther back first)
        sprites.sort_by(|a, b| {
            a.layer.order().cmp(&b.layer.order()).then(
                b.world_z
                    .partial_cmp(&a.world_z)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let group_defaults: HashMap<String, [f32; 3]> = theme_toml
            .group_defaults
            .iter()
            .map(|(k, v)| (k.clone(), [v[0] as f32, v[1] as f32, v[2] as f32]))
            .collect();
        themes.insert(
            name,
            BgTheme {
                sprites,
                group_defaults,
                child_order: theme_toml.child_order,
            },
        );
    }
    themes
}

fn try_load_themes() -> AppResult<HashMap<String, ThemeToml>> {
    let toml_str = include_str!("../../assets/bg-data.toml");
    toml::from_str(toml_str)
        .map_err(|error| AppError::invalid_data_key1("error_bg_data_parse", error.to_string()))
}

/// Get background theme data by name.
pub fn get_theme(name: &str) -> Option<&'static BgTheme> {
    let themes = BG_THEMES.get_or_init(build_themes);
    themes.get(name)
}

/// Parsed BG overrides: group/sprite name → partial position override.
#[derive(Default)]
pub struct BgOverrides {
    pub groups: HashMap<String, [Option<f32>; 3]>,
    pub sprites: HashMap<String, [Option<f32>; 3]>,
}

/// Parse BG override text from a BackgroundObject's PrefabOverrideData.
pub fn parse_bg_overrides(raw: &str) -> BgOverrides {
    let mut result = BgOverrides::default();
    let mut current_group = String::new();
    let mut current_sprite = String::new();
    // "group" or "sprite" — what m_LocalPosition applies to
    let mut parsing_for = "";

    for line in raw.lines() {
        let stripped = line.trim_end_matches('\r');
        let depth = stripped.len() - stripped.trim_start_matches('\t').len();
        let content = stripped.trim();
        if content.is_empty() {
            continue;
        }

        if depth == 1 && content.starts_with("GameObject ") {
            current_group = content[11..].trim().to_string();
            current_sprite.clear();
            parsing_for = "";
        } else if depth == 2 && content.starts_with("GameObject ") {
            current_sprite = content[11..].trim().to_string();
            parsing_for = "";
        } else if depth == 2 && content == "Component UnityEngine.Transform" {
            parsing_for = "group";
        } else if depth == 3 && content == "Component UnityEngine.Transform" {
            parsing_for = "sprite";
        } else if content.starts_with("Float ")
            && let Some(rest) = content.strip_prefix("Float ")
        {
            // Parse "x = 1.23" or "y = -4.56"
            let parts: Vec<&str> = rest.splitn(2, '=').collect();
            if parts.len() == 2 {
                let axis = parts[0].trim();
                if let Ok(val) = parts[1].trim().parse::<f32>() {
                    let (target_name, target_map) = match parsing_for {
                        "group" => (&current_group, &mut result.groups),
                        "sprite" => (&current_sprite, &mut result.sprites),
                        _ => continue,
                    };
                    if !target_name.is_empty() {
                        let entry = target_map
                            .entry(target_name.clone())
                            .or_insert([None, None, None]);
                        match axis {
                            "x" => entry[0] = Some(val),
                            "y" => entry[1] = Some(val),
                            "z" => entry[2] = Some(val),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    result
}

/// Apply BG overrides to a theme's sprites, returning modified copies with updated positions.
pub fn apply_bg_overrides(theme: &BgTheme, overrides: &BgOverrides) -> Vec<BgSprite> {
    if overrides.groups.is_empty() && overrides.sprites.is_empty() {
        return theme.sprites.clone();
    }
    theme
        .sprites
        .iter()
        .map(|s| {
            let defaults = match theme.group_defaults.get(&s.parent_group) {
                Some(d) => d,
                None => return s.clone(),
            };
            let group_ovr = overrides.groups.get(&s.parent_group);
            let sprite_ovr = overrides.sprites.get(&s.name);
            if group_ovr.is_none() && sprite_ovr.is_none() {
                return s.clone();
            }
            let gx = group_ovr.and_then(|o| o[0]).unwrap_or(defaults[0]);
            let gy = group_ovr.and_then(|o| o[1]).unwrap_or(defaults[1]);
            let gz = group_ovr.and_then(|o| o[2]).unwrap_or(defaults[2]);
            // When a sprite IS its own group parent (name == parentGroup),
            // its default localY already equals the group default position
            // (both represent the same Transform.m_LocalPosition). Using
            // localY here would double-count the offset. Treat it as 0.
            let is_group_root = s.name == s.parent_group;
            let lx = sprite_ovr.and_then(|o| o[0]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_x
            });
            let ly = sprite_ovr.and_then(|o| o[1]).unwrap_or(if is_group_root {
                0.0
            } else {
                s.local_y
            });
            let new_x = gx + lx;
            let new_y = gy + ly;
            let sprite_local_z = s.world_z - defaults[2];
            let new_z = gz + sprite_local_z;
            let mut out = s.clone();
            out.world_x = new_x;
            out.world_y = new_y;
            out.world_z = new_z;
            out
        })
        .collect()
}

/// Parse PositionSerializer `childLocalPositions` into group overrides.
///
/// EP6 background prefabs use a `PositionSerializer` component with an array
/// of child positions (indexed by `m_RootOrder`).  The `child_order` slice
/// maps each array index to the corresponding background group name so we
/// can produce the same `BgOverrides` struct that `apply_bg_overrides` expects.
pub fn parse_position_serializer_overrides(raw: &str, child_order: &[String]) -> BgOverrides {
    let mut result = BgOverrides {
        groups: HashMap::new(),
        sprites: HashMap::new(),
    };

    if !raw.contains("PositionSerializer") || child_order.is_empty() {
        return result;
    }

    let mut current_element: Option<usize> = None;
    let mut current_pos: [Option<f32>; 3] = [None, None, None];

    for line in raw.lines() {
        let content = line.trim();
        if let Some(rest) = content.strip_prefix("Element ") {
            // Flush previous element
            if let Some(idx) = current_element
                && idx < child_order.len()
                && !child_order[idx].is_empty()
            {
                result.groups.insert(child_order[idx].clone(), current_pos);
            }
            current_element = rest.trim().parse::<usize>().ok();
            current_pos = [None, None, None];
        } else if let Some(rest) = content.strip_prefix("Float ")
            && let Some((axis, val_str)) = rest.split_once('=')
            && let Ok(val) = val_str.trim().parse::<f32>()
        {
            match axis.trim() {
                "x" => current_pos[0] = Some(val),
                "y" => current_pos[1] = Some(val),
                "z" => current_pos[2] = Some(val),
                _ => {}
            }
        }
    }
    // Flush last element
    if let Some(idx) = current_element
        && idx < child_order.len()
        && !child_order[idx].is_empty()
    {
        result.groups.insert(child_order[idx].clone(), current_pos);
    }

    result
}

/// All known background atlas filenames.
pub fn bg_atlas_files() -> &'static [&'static str] {
    &[
        "Background_Cave_Sheet_01.png",
        "Background_Halloween_Sheet_01.png",
        "Background_Jungle_Sheet_01.png",
        "Background_Jungle_Sheet_02.png",
        "Background_Maya_Sheet_01.png",
        "Background_Maya_Sheet_02.png",
        "Background_Maya_Sheet_03.png",
        "Background_Maya_Sheet_04.png",
        "Background_Maya_Sheet_05.png",
        "Background_Morning_Sheet_01.png",
        "Background_Morning_Sheet_02.png",
        "Background_Night_Sheet_01.png",
        "Background_Plateaus_Sheet_01.png",
    ]
}

/// All known sky texture filenames.
pub fn sky_texture_files() -> &'static [&'static str] {
    &[
        "Halloween_Sky_Texture.png",
        "Jungle_Sky_Texture.png",
        "Maya_Backgrounds_sky.png",
        "Morning_Sky_Texture.png",
        "Night_Sky_Texture.png",
        "Plateau_Sky_Texture.png",
    ]
}
