//! Asset loading — terrain name→texture maps, sprite data, texture cache, BG theme detection.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use eframe::egui;

/// Embedded game assets (compiled into the binary).
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
pub struct EmbeddedAssets;

/// Read asset bytes by relative path (e.g. "sprites/IngameAtlas.png").
pub fn read_asset(key: &str) -> Option<Cow<'static, [u8]>> {
    EmbeddedAssets::get(key).map(|f| f.data)
}

/// Build an `egui::ColorImage` with gamma-space premultiplied alpha.
///
/// egui 0.33+ uses `Rgba8Unorm` (not sRGB) textures — the shader receives raw
/// bytes and does `vertex_color * tex_gamma` in gamma space.  Therefore stored
/// premultiplied values must use gamma-space premultiply: `r' = r * a / 255`.
fn color_image_premultiplied(size: [usize; 2], rgba: &[u8]) -> egui::ColorImage {
    let pixels = rgba
        .chunks_exact(4)
        .map(|p| {
            let (r, g, b, a) = (p[0], p[1], p[2], p[3]);
            if a == 255 {
                egui::Color32::from_rgba_premultiplied(r, g, b, 255)
            } else if a == 0 {
                egui::Color32::TRANSPARENT
            } else {
                let af = a as f32 * (1.0 / 255.0);
                let rp = (r as f32 * af + 0.5) as u8;
                let gp = (g as f32 * af + 0.5) as u8;
                let bp = (b as f32 * af + 0.5) as u8;
                egui::Color32::from_rgba_premultiplied(rp, gp, bp, a)
            }
        })
        .collect();
    egui::ColorImage::new(size, pixels)
}

// ── Terrain name → texture filename maps ─────────────

/// Terrain prefab name → 512x512 rock fill texture.
fn terrain_fill_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Rocks_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Rocks_Texture_02.png"),
            ("e2dTerrainBase_04", "Ground_Rocks_Texture_04.png"),
            ("e2dTerrainBase_05_night", "Ground_Rocks_Texture.png"),
            ("e2dTerrainBase_Halloween", "Ground_Halloween_Texture.png"),
            ("e2dTerrainBase_MM_Ice", "Ground_Ice_Texture.png"),
            ("e2dTerrainBase_morning", "Ground_Rocks_Texture_06.png"),
            ("e2dTerrainBase_MM_rock", "Ground_Temple_Rock_Texture.png"),
            ("e2dTerrainBase_MM_sand", "Ground_Temple_Tile_Texture.png"),
            (
                "e2dTerrainBase_MM_TempleDarkRock",
                "Ground_Temple_Dark_Texture.png",
            ),
            ("e2dTerrainBase_MM_caveSand", "Ground_Maya_cave_texture.png"),
        ])
    })
}

/// Terrain prefab → Splat0 (surface/grass) 16x16 texture.
fn terrain_splat0_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Grass_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Grass_Texture.png"),
            ("e2dTerrainBase_04", "Ground_Cave_Texture.png"),
            ("e2dTerrainBase_05_night", "Ground_Grass_Texture_02.png"),
            (
                "e2dTerrainBase_Halloween",
                "Ground_Halloween_Cream_Texture.png",
            ),
            ("e2dTerrainBase_MM_Ice", "Ground_Snow_Texture.png"),
            ("e2dTerrainBase_morning", "Ground_Grass_Texture_3.png"),
            ("e2dTerrainBase_MM_rock", "Ground_Grass_Maya_Texture.png"),
            ("e2dTerrainBase_MM_sand", "Ground_Grass_Maya_Texture.png"),
            (
                "e2dTerrainBase_MM_TempleDarkRock",
                "Ground_Grass_Maya_Texture.png",
            ),
            (
                "e2dTerrainBase_MM_caveSand",
                "Ground_Grass_Maya_Texture.png",
            ),
            // Dark variants with different Splat0 than their base
            ("e2dTerrainDark_MM_CaveSand", "Ground_Grass_Texture.png"),
            (
                "e2dTerrainDark_MM_rock",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
        ])
    })
}

/// Terrain prefab → Splat1 (outline) 16x16 texture.
fn terrain_splat1_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainBase", "Ground_Rocks_Outline_Texture.png"),
            ("e2dTerrainBase_02", "Ground_Rocks_Outline_Texture_02.png"),
            ("e2dTerrainBase_04", "Ground_Rocks_Outline_Texture_04.png"),
            (
                "e2dTerrainBase_05_night",
                "Ground_Rocks_Outline_Texture_05.png",
            ),
            (
                "e2dTerrainBase_Halloween",
                "Ground_Halloween_Outline_Texture.png",
            ),
            ("e2dTerrainBase_MM_Ice", "Ground_Ice_Outline.png"),
            (
                "e2dTerrainBase_morning",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
            (
                "e2dTerrainBase_MM_rock",
                "Ground_Rocks_Outline_Texture_03.png",
            ),
            (
                "e2dTerrainBase_MM_sand",
                "Ground_Rocks_Outline_Texture_03.png",
            ),
            (
                "e2dTerrainBase_MM_TempleDarkRock",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
            (
                "e2dTerrainBase_MM_caveSand",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
            // Dark variants with different Splat1 than their base
            ("e2dTerrainDark_02", "Ground_Rocks_Outline_Texture.png"),
            ("e2dTerrainDark_03", "Ground_Rocks_Outline_Texture_05.png"),
            (
                "e2dTerrainDark_05_night(150)",
                "Ground_Rocks_Outline_Texture_04.png",
            ),
            ("e2dTerrainDark_MM", "Ground_Rocks_Outline_Texture_06.png"),
            (
                "e2dTerrainDark_MM_CaveSand",
                "Ground_Rocks_Outline_Texture_06.png",
            ),
            (
                "e2dTerrainDark_MM_rock",
                "Ground_Rocks_Outline_Texture_03.png",
            ),
        ])
    })
}

/// Dark terrain variants → base prefab name they share textures with.
fn dark_terrain_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ("e2dTerrainDark", "e2dTerrainBase"),
            ("e2dTerrainDark_02", "e2dTerrainBase_02"),
            ("e2dTerrainDark_03", "e2dTerrainBase_02"),
            ("e2dTerrainDark_05_night(150)", "e2dTerrainBase_05_night"),
            ("e2dTerrainDark_MM", "e2dTerrainBase"),
            ("e2dTerrainDark_MM_CaveSand", "e2dTerrainBase_MM_caveSand"),
            (
                "e2dTerrainDark_MM_TempleDarkRock",
                "e2dTerrainBase_MM_TempleDarkRock",
            ),
            ("e2dTerrainDark_MM_rock", "e2dTerrainBase_MM_rock"),
            ("e2dTerrainDark Halloween", "e2dTerrainBase_Halloween"),
            ("e2dTerrainDark morning", "e2dTerrainBase_morning"),
            ("e2dTerrainDark_morning", "e2dTerrainBase_morning"),
            ("e2dTerrainDark cave", "e2dTerrainBase"),
            ("e2dTerrainDark_cave", "e2dTerrainBase"),
        ])
    })
}

/// Normalize a binary terrain object name to a known prefab key.
fn normalize_terrain(raw: &str) -> String {
    let mut n = raw.to_string();
    // Strip transition suffixes: " _ to ..." or " - to ..."
    if let Some(pos) = n.find(" _ to ").or_else(|| n.find(" - to ")) {
        n.truncate(pos);
    }
    // Strip trailing annotations like " EP1"
    if let Some(pos) = n.rfind(" EP") {
        n.truncate(pos);
    }
    // Strip trailing " - ..."
    if let Some(pos) = n.rfind(" - ") {
        n.truncate(pos);
    }
    // Strip trailing digit suffixes like " 131x3"
    let trimmed = n.trim_end();
    if let Some(pos) = trimmed.rfind(' ') {
        let tail = &trimmed[pos + 1..];
        if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            n.truncate(pos);
        }
    }
    n = n.trim().to_string();
    // Strip _X/_x suffix
    if n.ends_with("_X") || n.ends_with("_x") {
        n.truncate(n.len() - 2);
    }
    // Strip trailing " -"
    if n.ends_with(" -") {
        n.truncate(n.len() - 2);
        n = n.trim().to_string();
    }
    n
}

/// Resolve terrain name (possibly dark variant) to its base prefab key.
fn resolve_terrain_base(name: &str) -> String {
    let key = normalize_terrain(name);
    dark_terrain_map()
        .get(key.as_str())
        .map(|s| s.to_string())
        .unwrap_or(key)
}

/// Get the fill texture filename for a terrain object name.
pub fn get_terrain_fill_texture(terrain_name: &str) -> Option<&'static str> {
    let base = resolve_terrain_base(terrain_name);
    terrain_fill_map().get(base.as_str()).copied()
}

/// Get Splat0 (surface/grass) texture filename.
pub fn get_terrain_splat0(terrain_name: &str) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    // Check direct entry first (dark variants may have their own texture)
    terrain_splat0_map().get(key.as_str()).copied().or_else(|| {
        let base = resolve_terrain_base(terrain_name);
        terrain_splat0_map().get(base.as_str()).copied()
    })
}

/// Get Splat1 (outline) texture filename.
pub fn get_terrain_splat1(terrain_name: &str) -> Option<&'static str> {
    let key = normalize_terrain(terrain_name);
    // Check direct entry first (dark variants may have their own texture)
    terrain_splat1_map().get(key.as_str()).copied().or_else(|| {
        let base = resolve_terrain_base(terrain_name);
        terrain_splat1_map().get(base.as_str()).copied()
    })
}

/// Whether this is a "dark" terrain (underground fill).
pub fn is_dark_terrain(terrain_name: &str) -> bool {
    let key = normalize_terrain(terrain_name);
    dark_terrain_map().contains_key(key.as_str())
}

// ── Background theme detection ───────────────────────

const BG_THEME_PATTERNS: &[(&str, &str)] = &[
    ("MayaCave2Dark", "MayaCave2Dark"),
    ("MayaCaveDark", "MayaCaveDark"),
    ("MayaCave", "MayaCave"),
    ("MayaTemple", "MayaTemple"),
    ("MayaHigh", "MayaHigh"),
    ("Maya", "Maya"),
    ("Jungle", "Jungle"),
    ("Plateau", "Plateau"),
    ("Morning", "Morning"),
    ("Night", "Night"),
    ("Halloween", "Halloween"),
    ("Cave", "Cave"),
];

/// Detect which background theme to use from level object names.
pub fn detect_bg_theme(object_names: &[String]) -> Option<&'static str> {
    for name in object_names {
        for &(pattern, theme) in BG_THEME_PATTERNS {
            if name.contains(pattern) {
                return Some(theme);
            }
        }
    }
    None
}

/// Sky top color per theme (sampled from first pixel row of each sky PNG).
pub fn sky_top_color(theme: &str) -> egui::Color32 {
    match theme {
        "Jungle" => egui::Color32::from_rgb(0x26, 0xaa, 0xc2),
        "Plateau" => egui::Color32::from_rgb(0x26, 0x78, 0xc2),
        "Night" => egui::Color32::from_rgb(0x43, 0x47, 0x54),
        "Morning" => egui::Color32::from_rgb(0xf7, 0xf8, 0xda),
        "Halloween" => egui::Color32::from_rgb(0x0a, 0x4b, 0x38),
        "Cave" => egui::Color32::from_rgb(0x58, 0xc0, 0x44),
        "Maya" => egui::Color32::from_rgb(0x26, 0xaa, 0xc2),
        "MayaCave" => egui::Color32::from_rgb(0x58, 0xc0, 0x44),
        "MayaCave2Dark" => egui::Color32::from_rgb(0x03, 0x12, 0x12),
        "MayaCaveDark" => egui::Color32::from_rgb(0x58, 0xc0, 0x44),
        "MayaHigh" => egui::Color32::from_rgb(0x96, 0xb6, 0xc7),
        "MayaTemple" => egui::Color32::from_rgb(0x26, 0x78, 0xc2),
        _ => egui::Color32::from_rgb(0x26, 0xaa, 0xc2),
    }
}

/// Ground fill color per theme (sampled from Background_*_Sheet fill sprite UV regions).
pub fn ground_color(theme: &str) -> egui::Color32 {
    match theme {
        "Jungle" => egui::Color32::from_rgb(0x33, 0x88, 0x44),
        "Plateau" => egui::Color32::from_rgb(0x33, 0x77, 0x66),
        "Night" => egui::Color32::from_rgb(0x20, 0x2d, 0x42),
        "Morning" => egui::Color32::from_rgb(0x33, 0x44, 0x55),
        "Halloween" => egui::Color32::from_rgb(0x3d, 0x2c, 0x4d),
        "Cave" => egui::Color32::from_rgb(0x11, 0x21, 0x11),
        "Maya" => egui::Color32::from_rgb(0x05, 0x18, 0x26),
        "MayaCave" | "MayaCave2Dark" | "MayaCaveDark" => egui::Color32::from_rgb(0x11, 0x21, 0x11),
        "MayaHigh" | "MayaTemple" => egui::Color32::from_rgb(0x05, 0x18, 0x26),
        _ => egui::Color32::from_rgb(0x33, 0x77, 0x66),
    }
}

// ── Props tint per theme (Unity GenericProps material `_Color`) ──

/// Returns the `_Color` tint that Unity applies to Props sprites via
/// `GenericPropsNight.mat` / `GenericPropsMorning2.mat`.
pub fn props_tint_color(theme: Option<&str>) -> [f32; 4] {
    match theme {
        // GenericPropsNight.mat  _Color = (0.745, 0.745, 1, 1)
        Some("Night" | "Halloween" | "MayaCaveDark" | "MayaCave2Dark") => {
            [0.7450981, 0.7450981, 1.0, 1.0]
        }
        // GenericPropsMorning2.mat  _Color = (0.443, 0.532, 0.582, 1)
        Some("Morning") => [0.443, 0.532, 0.582, 1.0],
        // GenericProps.mat  _Color = (1, 1, 1, 1)
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

/// Sprites that keep their original material and are NOT tinted by
/// GenericPropsNight / GenericPropsMorning2 at runtime in Unity.
pub fn skip_props_tint(name: &str) -> bool {
    name.starts_with("Crystal_")
        || name.starts_with("Glow_")
        || name.starts_with("Lit")
        || name.starts_with("Secret_")
        || name.starts_with("Star_")
}

// ── Color helpers ────────────────────────────────────

/// Named prefab colors for known types.
pub fn get_object_color(name: &str, prefab_index: i16) -> egui::Color32 {
    if name.contains("Background") {
        return egui::Color32::from_rgb(0x2a, 0x4a, 0x2e);
    }
    if name.contains("Goal") {
        return egui::Color32::from_rgb(0xff, 0xd7, 0x00);
    }
    if name.contains("StarBox") {
        return egui::Color32::from_rgb(0xff, 0xeb, 0x3b);
    }
    if name.contains("DessertPlace") {
        return egui::Color32::from_rgb(0xff, 0x98, 0x00);
    }
    if name.contains("TNT") {
        return egui::Color32::from_rgb(0xf4, 0x43, 0x36);
    }
    if name.contains("Pig") {
        return egui::Color32::from_rgb(0xff, 0x69, 0xb4);
    }

    // HSL-based color from prefab index
    let hue = ((prefab_index as i32 * 47) % 360 + 360) % 360;
    hsl_to_rgb(hue as f32, 0.6, 0.55)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> egui::Color32 {
    let a = s * l.min(1.0 - l);
    let f = |n: f32| -> f32 {
        let k = (n + h / 30.0) % 12.0;
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    egui::Color32::from_rgb(
        (f(0.0) * 255.0) as u8,
        (f(8.0) * 255.0) as u8,
        (f(4.0) * 255.0) as u8,
    )
}

/// Whether this object should be skipped during rendering.
pub fn should_skip_render(name: &str) -> bool {
    if name.starts_with("Cloud") && name.ends_with("Set") {
        return true;
    }
    const SKIP_EXACT: &[&str] = &[
        "Props",
        "Prop",
        "Challenges",
        "DessertPlaces",
        "LitArea",
        "reference",
    ];
    if SKIP_EXACT.contains(&name) {
        return true;
    }
    const SKIP_CONTAINS: &[&str] = &[
        "CameraSystem",
        "LevelManager",
        "LevelStart",
        "Background",
        "Decoration ",
        "DontUsePart",
        "Challenge",
        "Tutorial",
        "Achievement",
    ];
    SKIP_CONTAINS.iter().any(|s| name.contains(s))
}

// ── Texture cache ────────────────────────────────────

/// Texture cache for egui texture handles.
pub struct TextureCache {
    textures: HashMap<String, egui::TextureHandle>,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Load a PNG and register it as an egui texture.
    /// `path` is a relative asset key (e.g. "sprites/IngameAtlas.png").
    pub fn load_texture(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let handle = ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a texture from raw RGBA bytes (for control textures decoded from level data).
    #[allow(dead_code)]
    pub fn load_from_rgba(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        pixels: &[u8],
        width: usize,
        height: usize,
    ) -> egui::TextureId {
        if let Some(handle) = self.textures.get(key) {
            return handle.id();
        }
        let color_image = color_image_premultiplied([width, height], pixels);
        let handle = ctx.load_texture(key, color_image, egui::TextureOptions::LINEAR);
        let id = handle.id();
        self.textures.insert(key.to_string(), handle);
        id
    }

    /// Load a PNG texture with repeat (tiling) wrap mode.
    /// `path` is a relative asset key (e.g. "ground/Ground_Rocks_Texture.png").
    pub fn load_texture_repeat(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    /// Load a PNG texture with repeat wrap and vertical flip.
    /// Flipping matches Unity/Three.js convention where V=0 is image bottom.
    pub fn load_texture_repeat_flipv(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        name: &str,
    ) -> Option<egui::TextureId> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.id());
        }

        let data = read_asset(path)?;
        let img = image::load_from_memory(&data).ok()?.to_rgba8();
        let img = image::imageops::flip_vertical(&img);
        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.into_raw();
        let color_image = color_image_premultiplied(size, &pixels);
        let options = egui::TextureOptions {
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..egui::TextureOptions::LINEAR
        };
        let handle = ctx.load_texture(name, color_image, options);
        let id = handle.id();
        self.textures.insert(name.to_string(), handle);
        Some(id)
    }

    pub fn get(&self, path: &str) -> Option<egui::TextureId> {
        self.textures.get(path).map(|h| h.id())
    }

    /// Get the pixel dimensions of a loaded texture.
    pub fn texture_size(&self, name: &str) -> Option<[usize; 2]> {
        self.textures.get(name).map(|h| h.size())
    }
}
