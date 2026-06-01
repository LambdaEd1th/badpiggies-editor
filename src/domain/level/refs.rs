//! Level refs database.
//!
//! Terrain texture refs are rebuilt at runtime from embedded Unity loader prefabs.
//! Prefab names are rebuilt at runtime from embedded loader prefabs and prefab
//! YAMLs by resolving each loader `m_prefabs` entry's GameObject fileID.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use crate::data::assets;
use crate::diagnostics::error::{AppError, AppResult};

const LOADER_ASSET_PREFIX: &str = "Assets/Resources/levels/";
const PREFAB_ASSET_PREFIX: &str = "Assets/Prefab/";

#[derive(Debug, Clone)]
struct LoaderReference {
    guid: Option<String>,
    file_id: Option<String>,
    ref_type: i32,
}

#[derive(Debug, Clone)]
struct ParsedLoaderPrefab {
    level_key: String,
    prefab_file_ids: Vec<String>,
    references: Vec<LoaderReference>,
}

/// Parsed lookup: level_key → (ref_index → texture_filename)
type RefsMap = HashMap<String, HashMap<i32, String>>;
/// Parsed lookup: level_key → set(ref_index) where loader m_references entry is explicit null (fileID 0).
type NullRefsMap = HashMap<String, HashSet<i32>>;
/// Parsed lookup: level_key → (prefab_index → corrected_name)
type PrefabsMap = HashMap<String, HashMap<i16, String>>;
/// Parsed lookup: level_key → (ref_index → background prefab asset stem)
type BackgroundPrefabsMap = HashMap<String, HashMap<i32, String>>;

struct LevelRefsData {
    refs: RefsMap,
    null_refs: NullRefsMap,
    prefabs: PrefabsMap,
    background_prefabs: BackgroundPrefabsMap,
}

fn data() -> &'static LevelRefsData {
    static INSTANCE: OnceLock<LevelRefsData> = OnceLock::new();
    INSTANCE.get_or_init(|| match try_load_level_refs() {
        Ok(data) => data,
        Err(error) => {
            log::error!("Failed to load level refs: {error}");
            LevelRefsData {
                refs: HashMap::new(),
                null_refs: HashMap::new(),
                prefabs: HashMap::new(),
                background_prefabs: HashMap::new(),
            }
        }
    })
}

fn try_load_level_refs() -> AppResult<LevelRefsData> {
    let loaders = load_embedded_loaders()?;
    let prefab_names = load_prefab_names_by_file_id()?;
    let background_prefab_names = load_background_prefab_names_by_root_file_id();
    let refs = build_runtime_level_refs(&loaders);
    let null_refs = build_runtime_null_level_refs(&loaders);
    let prefabs = build_runtime_prefab_names(&loaders, &prefab_names);
    let background_prefabs =
        build_runtime_background_prefab_refs(&loaders, &background_prefab_names);

    Ok(LevelRefsData {
        refs,
        null_refs,
        prefabs,
        background_prefabs,
    })
}

fn build_runtime_null_level_refs(loaders: &[ParsedLoaderPrefab]) -> NullRefsMap {
    let mut null_refs = HashMap::new();

    for loader in loaders {
        let mut level_null_refs = HashSet::new();
        for (index, reference) in loader.references.iter().enumerate() {
            if reference.guid.is_none() && reference.file_id.as_deref() == Some("0") {
                level_null_refs.insert(index as i32);
            }
        }
        null_refs.insert(loader.level_key.clone(), level_null_refs);
    }

    null_refs
}

fn load_embedded_loaders() -> AppResult<Vec<ParsedLoaderPrefab>> {
    let mut loaders = Vec::new();

    for asset_path in assets::list_pathnames(LOADER_ASSET_PREFIX, "_loader.prefab") {
        let text = read_embedded_text(&asset_path)?;
        let Some(loader) = parse_loader_prefab(&text) else {
            continue;
        };
        loaders.push(loader);
    }

    Ok(loaders)
}

fn build_runtime_level_refs(loaders: &[ParsedLoaderPrefab]) -> RefsMap {
    let mut refs = HashMap::new();

    for loader in loaders {
        let mut level_refs = HashMap::new();
        for (index, reference) in loader.references.iter().enumerate() {
            if reference.ref_type != 3 {
                continue;
            }

            let Some(guid) = reference.guid.as_deref() else {
                continue;
            };
            let Some(texture_name) = texture_name_for_guid(guid) else {
                continue;
            };

            level_refs.insert(index as i32, texture_name.to_string());
        }

        refs.insert(loader.level_key.clone(), level_refs);
    }

    refs
}

fn load_prefab_names_by_file_id() -> AppResult<HashMap<String, String>> {
    let mut names_by_file_id = HashMap::new();

    for asset_path in assets::list_pathnames(PREFAB_ASSET_PREFIX, ".prefab") {
        let text = read_embedded_text(&asset_path)?;
        let prefab_name = Path::new(&asset_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(asset_path.as_str())
            .to_string();

        for file_id in parse_prefab_game_object_ids(&text) {
            names_by_file_id
                .entry(file_id)
                .or_insert_with(|| prefab_name.clone());
        }
    }

    Ok(names_by_file_id)
}

fn load_background_prefab_names_by_root_file_id() -> HashMap<String, String> {
    let mut names_by_file_id = HashMap::new();

    for asset_path in assets::list_pathnames("Assets/Resources/environment/background/", ".prefab")
    {
        let Some(text) = assets::read_pathname_text(&asset_path) else {
            continue;
        };
        let Some(file_id) = parse_prefab_root_game_object_id(&text) else {
            continue;
        };
        let prefab_name = Path::new(&asset_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(asset_path.as_str())
            .to_string();
        names_by_file_id.insert(file_id, prefab_name);
    }

    names_by_file_id
}

fn parse_prefab_root_game_object_id(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("m_RootGameObject: {fileID: ")
            .map(|value| value.trim_end_matches('}').trim().to_string())
    })
}

fn parse_prefab_game_object_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("--- !u!1 &"))
        .map(|file_id| file_id.trim().to_string())
        .collect()
}

fn build_runtime_prefab_names(
    loaders: &[ParsedLoaderPrefab],
    prefab_names: &HashMap<String, String>,
) -> PrefabsMap {
    let mut prefabs = HashMap::new();

    for loader in loaders {
        let mut level_prefabs = HashMap::new();
        for (index, file_id) in loader.prefab_file_ids.iter().enumerate() {
            let Ok(index) = i16::try_from(index) else {
                continue;
            };
            let Some(name) = prefab_names.get(file_id) else {
                continue;
            };
            level_prefabs.insert(index, name.clone());
        }
        prefabs.insert(loader.level_key.clone(), level_prefabs);
    }

    prefabs
}

fn build_runtime_background_prefab_refs(
    loaders: &[ParsedLoaderPrefab],
    background_prefab_names: &HashMap<String, String>,
) -> BackgroundPrefabsMap {
    let mut prefabs = HashMap::new();

    for loader in loaders {
        let mut level_prefabs = HashMap::new();
        for (index, reference) in loader.references.iter().enumerate() {
            if reference.ref_type != 2 {
                continue;
            }

            let Some(file_id) = reference.file_id.as_deref() else {
                continue;
            };
            let Some(name) = background_prefab_names.get(file_id) else {
                continue;
            };

            level_prefabs.insert(index as i32, name.clone());
        }
        prefabs.insert(loader.level_key.clone(), level_prefabs);
    }

    prefabs
}

fn read_embedded_text(path: &str) -> AppResult<String> {
    let bytes = assets::read_pathname(path).ok_or_else(|| {
        AppError::invalid_data_key1("error_level_refs_missing_asset", path.to_string())
    })?;
    String::from_utf8(bytes.into_owned()).map_err(|error| {
        AppError::invalid_data_key1("error_level_refs_invalid_utf8", format!("{path}: {error}"))
    })
}

fn parse_loader_prefab(text: &str) -> Option<ParsedLoaderPrefab> {
    let mut asset_name = None;
    let mut prefab_file_ids = Vec::new();
    let mut references = Vec::new();

    #[derive(Clone, Copy)]
    enum LoaderSection {
        None,
        Prefabs,
        References,
    }

    let mut section = LoaderSection::None;

    for line in text.lines() {
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_prefix("assetName:") {
            asset_name = Some(name.trim().to_string());
        }

        if trimmed == "m_prefabs:" {
            section = LoaderSection::Prefabs;
            continue;
        }

        if trimmed == "m_references:" {
            section = LoaderSection::References;
            continue;
        }

        match section {
            LoaderSection::None => {}
            LoaderSection::Prefabs => {
                if trimmed.starts_with("- ") {
                    if let Some(file_id) = extract_loader_field(trimmed, "fileID: ") {
                        prefab_file_ids.push(file_id.to_string());
                    }
                    continue;
                }

                if trimmed.starts_with("m_") || trimmed.starts_with("assetBundle:") {
                    section = LoaderSection::None;
                }
            }
            LoaderSection::References => {
                if trimmed.starts_with("- ") {
                    references.push(parse_loader_reference(trimmed));
                    continue;
                }

                if trimmed.starts_with("m_") || trimmed.starts_with("assetBundle:") {
                    section = LoaderSection::None;
                }
            }
        }
    }

    asset_name.map(|level_key| ParsedLoaderPrefab {
        level_key,
        prefab_file_ids,
        references,
    })
}

fn parse_loader_reference(line: &str) -> LoaderReference {
    LoaderReference {
        guid: extract_loader_field(line, "guid: ").map(ToOwned::to_owned),
        file_id: extract_loader_field(line, "fileID: ").map(ToOwned::to_owned),
        ref_type: extract_loader_field(line, "type: ")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or_default(),
    }
}

fn extract_loader_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let start = line.find(field)? + field.len();
    let rest = &line[start..];
    let end = rest
        .find(',')
        .or_else(|| rest.find('}'))
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialShaderKind {
    CustomUnlitMonochrome,
    CustomUnlitColorGeometry,
    CustomUnlitColorTransparentGeometry,
    CustomUnlitAlpha8BitColor,
    BuiltinUnlitTransparent,
    BuiltinUnlitTransparentCutout,
}

fn runtime_texture_name_for_guid(guid: &str) -> Option<&'static str> {
    let pathname = assets::pathname_for_guid(guid)?;
    texture_name_from_asset_pathname(pathname)
}

fn runtime_material_texture_name_for_guid(guid: &str) -> Option<&'static str> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    let texture_guid = main_texture_guid_from_material(&text)?;
    texture_name_for_guid(texture_guid)
}

#[cfg_attr(not(test), allow(dead_code))]
fn runtime_material_color_rgba_for_guid(guid: &str) -> Option<[u8; 4]> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    main_color_from_material(&text)
}

fn runtime_material_texture_name_for_guid_prefix(guid: &str) -> Option<&'static str> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_texture_prefixes)
        .get(guid_prefix(guid))
        .map(String::as_str)
}

fn runtime_material_color_rgba_for_guid_prefix(guid: &str) -> Option<[u8; 4]> {
    static MAP: OnceLock<HashMap<String, [u8; 4]>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_color_prefixes)
        .get(guid_prefix(guid))
        .copied()
}

fn runtime_material_alpha_blend_for_guid_prefix(guid: &str) -> bool {
    static MAP: OnceLock<HashMap<String, bool>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_alpha_blend_prefixes)
        .get(guid_prefix(guid))
        .copied()
        .unwrap_or(false)
}

fn runtime_material_alpha_blend_for_guid(guid: &str) -> bool {
    let Some(pathname) = assets::pathname_for_guid(guid) else {
        return false;
    };
    if !is_material_pathname(pathname) {
        return false;
    }

    assets::read_guid_text(guid)
        .is_some_and(|text| material_uses_alpha_blend_shader(&text, runtime_shader_names_by_guid()))
}

fn runtime_material_alpha8bit_for_guid_prefix(guid: &str) -> bool {
    static MAP: OnceLock<HashMap<String, bool>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_alpha8bit_prefixes)
        .get(guid_prefix(guid))
        .copied()
        .unwrap_or(false)
}

fn runtime_material_alpha8bit_for_guid(guid: &str) -> bool {
    let Some(pathname) = assets::pathname_for_guid(guid) else {
        return false;
    };
    if !is_material_pathname(pathname) {
        return false;
    }

    assets::read_guid_text(guid)
        .is_some_and(|text| material_uses_alpha8bit_shader(&text, runtime_shader_names_by_guid()))
}

fn runtime_material_cutoff_for_guid_prefix(guid: &str) -> Option<f32> {
    static MAP: OnceLock<HashMap<String, f32>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_cutoff_prefixes)
        .get(guid_prefix(guid))
        .copied()
}

fn runtime_material_cutoff_for_guid(guid: &str) -> Option<f32> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    cutoff_from_material(&text)
}

fn runtime_material_custom_render_queue_for_guid_prefix(guid: &str) -> Option<i32> {
    static MAP: OnceLock<HashMap<String, i32>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_custom_render_queue_prefixes)
        .get(guid_prefix(guid))
        .copied()
}

fn runtime_material_custom_render_queue_for_guid(guid: &str) -> Option<i32> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    custom_render_queue_from_material(&text)
}

fn runtime_material_shader_kind_for_guid(guid: &str) -> Option<MaterialShaderKind> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    material_shader_kind(&text, runtime_shader_names_by_guid())
}

fn runtime_material_shader_kind_for_guid_prefix(guid: &str) -> Option<MaterialShaderKind> {
    static MAP: OnceLock<HashMap<String, MaterialShaderKind>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_shader_kind_prefixes)
        .get(guid_prefix(guid))
        .copied()
}

fn runtime_material_main_tex_st_for_guid(guid: &str) -> Option<[f32; 4]> {
    let pathname = assets::pathname_for_guid(guid)?;
    if !is_material_pathname(pathname) {
        return None;
    }

    let text = assets::read_guid_text(guid)?;
    main_texture_st_from_material(&text)
}

fn runtime_material_main_tex_st_for_guid_prefix(guid: &str) -> Option<[f32; 4]> {
    static MAP: OnceLock<HashMap<String, [f32; 4]>> = OnceLock::new();

    MAP.get_or_init(build_runtime_material_main_tex_st_prefixes)
        .get(guid_prefix(guid))
        .copied()
}

fn build_runtime_material_texture_prefixes() -> HashMap<String, String> {
    let mut textures_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(texture_guid) = main_texture_guid_from_material(&text) else {
            continue;
        };
        let Some(texture_name) = texture_name_for_guid(texture_guid) else {
            continue;
        };
        textures_by_guid.insert(guid.to_string(), texture_name.to_string());
    }

    build_unique_prefix_map(&textures_by_guid)
}

fn build_runtime_material_color_prefixes() -> HashMap<String, [u8; 4]> {
    let mut colors_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(color) = main_color_from_material(&text) else {
            continue;
        };
        colors_by_guid.insert(guid.to_string(), color);
    }

    build_unique_prefix_map(&colors_by_guid)
}

fn build_runtime_material_alpha_blend_prefixes() -> HashMap<String, bool> {
    let shader_names = runtime_shader_names_by_guid();
    let mut alpha_blends_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        if !material_uses_alpha_blend_shader(&text, shader_names) {
            continue;
        }
        alpha_blends_by_guid.insert(guid.to_string(), true);
    }

    build_unique_prefix_map(&alpha_blends_by_guid)
}

fn build_runtime_material_alpha8bit_prefixes() -> HashMap<String, bool> {
    let shader_names = runtime_shader_names_by_guid();
    let mut alpha8bits_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        if !material_uses_alpha8bit_shader(&text, shader_names) {
            continue;
        }
        alpha8bits_by_guid.insert(guid.to_string(), true);
    }

    build_unique_prefix_map(&alpha8bits_by_guid)
}

fn build_runtime_material_cutoff_prefixes() -> HashMap<String, f32> {
    let mut cutoffs_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(cutoff) = cutoff_from_material(&text) else {
            continue;
        };
        cutoffs_by_guid.insert(guid.to_string(), cutoff);
    }

    build_unique_float_prefix_map(&cutoffs_by_guid)
}

fn build_runtime_material_custom_render_queue_prefixes() -> HashMap<String, i32> {
    let mut queues_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(render_queue) = custom_render_queue_from_material(&text) else {
            continue;
        };
        queues_by_guid.insert(guid.to_string(), render_queue);
    }

    build_unique_i32_prefix_map(&queues_by_guid)
}

fn build_runtime_material_shader_kind_prefixes() -> HashMap<String, MaterialShaderKind> {
    let shader_names = runtime_shader_names_by_guid();
    let mut shader_kinds_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(shader_kind) = material_shader_kind(&text, shader_names) else {
            continue;
        };
        shader_kinds_by_guid.insert(guid.to_string(), shader_kind);
    }

    build_unique_prefix_map(&shader_kinds_by_guid)
}

fn build_runtime_material_main_tex_st_prefixes() -> HashMap<String, [f32; 4]> {
    let mut main_tex_st_by_guid = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".mat") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(main_tex_st) = main_texture_st_from_material(&text) else {
            continue;
        };
        main_tex_st_by_guid.insert(guid.to_string(), main_tex_st);
    }

    build_unique_vec4_prefix_map(&main_tex_st_by_guid)
}

fn runtime_shader_names_by_guid() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

    MAP.get_or_init(build_runtime_shader_names_by_guid)
}

fn build_runtime_shader_names_by_guid() -> HashMap<String, String> {
    let mut shader_names = HashMap::new();

    for pathname in assets::list_pathnames("Assets/", ".shader") {
        let Some(guid) = assets::guid_for_pathname(&pathname) else {
            continue;
        };
        let Some(text) = assets::read_pathname_text(&pathname) else {
            continue;
        };
        let Some(shader_name) = shader_name_from_shader_asset(&text) else {
            continue;
        };
        shader_names.insert(guid.to_string(), shader_name.to_string());
    }

    shader_names
}

fn build_unique_prefix_map<T>(entries: &HashMap<String, T>) -> HashMap<String, T>
where
    T: Clone + Eq,
{
    let mut by_prefix: HashMap<String, Option<T>> = HashMap::new();

    for (guid, value) in entries {
        match by_prefix.get(guid_prefix(guid)) {
            None => {
                by_prefix.insert(guid_prefix(guid).to_string(), Some(value.clone()));
            }
            Some(Some(existing)) if existing == value => {}
            Some(_) => {
                by_prefix.insert(guid_prefix(guid).to_string(), None);
            }
        }
    }

    by_prefix
        .into_iter()
        .filter_map(|(prefix, value)| value.map(|value| (prefix, value)))
        .collect()
}

fn build_unique_float_prefix_map(entries: &HashMap<String, f32>) -> HashMap<String, f32> {
    let mut by_prefix: HashMap<String, Option<f32>> = HashMap::new();

    for (guid, value) in entries {
        match by_prefix.get(guid_prefix(guid)) {
            None => {
                by_prefix.insert(guid_prefix(guid).to_string(), Some(*value));
            }
            Some(Some(existing)) if existing.to_bits() == value.to_bits() => {}
            Some(_) => {
                by_prefix.insert(guid_prefix(guid).to_string(), None);
            }
        }
    }

    by_prefix
        .into_iter()
        .filter_map(|(prefix, value)| value.map(|value| (prefix, value)))
        .collect()
}

fn build_unique_i32_prefix_map(entries: &HashMap<String, i32>) -> HashMap<String, i32> {
    let mut by_prefix: HashMap<String, Option<i32>> = HashMap::new();

    for (guid, value) in entries {
        match by_prefix.get(guid_prefix(guid)) {
            None => {
                by_prefix.insert(guid_prefix(guid).to_string(), Some(*value));
            }
            Some(Some(existing)) if existing == value => {}
            Some(_) => {
                by_prefix.insert(guid_prefix(guid).to_string(), None);
            }
        }
    }

    by_prefix
        .into_iter()
        .filter_map(|(prefix, value)| value.map(|value| (prefix, value)))
        .collect()
}

fn build_unique_vec4_prefix_map(entries: &HashMap<String, [f32; 4]>) -> HashMap<String, [f32; 4]> {
    let mut by_prefix: HashMap<String, Option<[f32; 4]>> = HashMap::new();

    for (guid, value) in entries {
        match by_prefix.get(guid_prefix(guid)) {
            None => {
                by_prefix.insert(guid_prefix(guid).to_string(), Some(*value));
            }
            Some(Some(existing))
                if existing
                    .iter()
                    .zip(value.iter())
                    .all(|(lhs, rhs)| lhs.to_bits() == rhs.to_bits()) => {}
            Some(_) => {
                by_prefix.insert(guid_prefix(guid).to_string(), None);
            }
        }
    }

    by_prefix
        .into_iter()
        .filter_map(|(prefix, value)| value.map(|value| (prefix, value)))
        .collect()
}

fn texture_name_from_asset_pathname(pathname: &str) -> Option<&str> {
    if !is_texture_pathname(pathname) {
        return None;
    }

    Path::new(pathname)
        .file_name()
        .and_then(|name| name.to_str())
}

fn is_texture_pathname(pathname: &str) -> bool {
    let is_texture_dir =
        pathname.starts_with("Assets/Texture2D/") || pathname.starts_with("Assets/Resources/");
    if !is_texture_dir {
        return false;
    }

    matches!(
        Path::new(pathname)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("png") | Some("jpg") | Some("jpeg")
    )
}

fn is_material_pathname(pathname: &str) -> bool {
    pathname.starts_with("Assets/")
        && Path::new(pathname)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mat"))
}

fn main_texture_guid_from_material(text: &str) -> Option<&str> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .starts_with("m_Texture:")
            .then(|| extract_guid(trimmed))
            .flatten()
    })
}

fn main_color_from_material(text: &str) -> Option<[u8; 4]> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .starts_with("- _Color:")
            .then(|| parse_rgba_color(trimmed))
            .flatten()
    })
}

fn cutoff_from_material(text: &str) -> Option<f32> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .starts_with("- _Cutoff:")
            .then(|| parse_scalar_value(trimmed))
            .flatten()
    })
}

fn main_texture_st_from_material(text: &str) -> Option<[f32; 4]> {
    let mut in_main_tex = false;
    let mut scale = None;
    let mut offset = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "- _MainTex:" {
            in_main_tex = true;
            continue;
        }
        if !in_main_tex {
            continue;
        }
        if trimmed.starts_with("- ") {
            break;
        }
        if trimmed.starts_with("m_Scale:") {
            scale = parse_vec2_like(trimmed);
            continue;
        }
        if trimmed.starts_with("m_Offset:") {
            offset = parse_vec2_like(trimmed);
            continue;
        }
    }

    match (scale, offset) {
        (Some(scale), Some(offset)) => Some([scale[0], scale[1], offset[0], offset[1]]),
        _ => None,
    }
}

fn custom_render_queue_from_material(text: &str) -> Option<i32> {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("m_CustomRenderQueue:")
                .and_then(|value| value.trim().parse::<i32>().ok())
        })
        .filter(|queue| *queue >= 0)
}

fn material_uses_alpha_blend_shader(text: &str, shader_names: &HashMap<String, String>) -> bool {
    material_shader_name(text, shader_names).is_some_and(|name| shader_name_uses_alpha_blend(&name))
}

fn material_uses_alpha8bit_shader(text: &str, shader_names: &HashMap<String, String>) -> bool {
    material_shader_name(text, shader_names).is_some_and(|name| shader_name_uses_alpha8bit(&name))
}

fn material_shader_kind(
    text: &str,
    shader_names: &HashMap<String, String>,
) -> Option<MaterialShaderKind> {
    let shader_name = material_shader_name(text, shader_names)?;
    match shader_name.as_str() {
        "_Custom/Unlit_Monochrome" => Some(MaterialShaderKind::CustomUnlitMonochrome),
        "_Custom/Unlit_Color_Geometry" => Some(MaterialShaderKind::CustomUnlitColorGeometry),
        "_Custom/Unlit_ColorTransparent_Geometry" => {
            Some(MaterialShaderKind::CustomUnlitColorTransparentGeometry)
        }
        "_Custom/Unlit_Alpha8Bit_Color" => Some(MaterialShaderKind::CustomUnlitAlpha8BitColor),
        "Unlit/Transparent" => Some(MaterialShaderKind::BuiltinUnlitTransparent),
        "Unlit/Transparent Cutout" => Some(MaterialShaderKind::BuiltinUnlitTransparentCutout),
        _ => None,
    }
}

fn material_shader_name(text: &str, shader_names: &HashMap<String, String>) -> Option<String> {
    let line = text
        .lines()
        .find(|line| line.trim().starts_with("m_Shader:"))?;
    let trimmed = line.trim();

    if let Some(guid) = extract_guid(trimmed) {
        if let Some(shader_name) = shader_names.get(guid) {
            return Some(shader_name.clone());
        }
        if let Some(shader_name) = fallback_shader_name_for_guid(guid) {
            return Some(shader_name.to_string());
        }
    }

    let file_id = extract_file_id(trimmed)?;
    builtin_shader_name(file_id).map(str::to_string)
}

fn fallback_shader_name_for_guid(guid: &str) -> Option<&'static str> {
    match guid {
        // Some embedded project snapshots keep material shader GUIDs but omit
        // the matching shader assets from the indexed asset set. Background
        // fill / far-bg materials still need these names to recover exact
        // shader modes from material YAML.
        "3ee1baa860fa20a74fa3b12a3f0bc258" => Some("_Custom/Unlit_Monochrome"),
        "f2bf0be753c6f2a3466cb9069962a880" => Some("_Custom/Unlit_ColorTransparent_Geometry"),
        _ => None,
    }
}

fn extract_guid(line: &str) -> Option<&str> {
    let start = line.find("guid: ")? + "guid: ".len();
    let rest = &line[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_hexdigit())
        .unwrap_or(rest.len());
    let guid = &rest[..end];
    (!guid.is_empty()).then_some(guid)
}

fn extract_file_id(line: &str) -> Option<&str> {
    let start = line.find("fileID: ")? + "fileID: ".len();
    let rest = &line[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(rest.len());
    let file_id = &rest[..end];
    (!file_id.is_empty()).then_some(file_id)
}

fn parse_rgba_color(line: &str) -> Option<[u8; 4]> {
    let start = line.find('{')? + 1;
    let end = line.rfind('}')?;
    let mut rgba = [0u8; 4];
    let mut seen = [false; 4];

    for part in line[start..end].split(',') {
        let (channel, value) = part.trim().split_once(':')?;
        let index = match channel.trim() {
            "r" => 0,
            "g" => 1,
            "b" => 2,
            "a" => 3,
            _ => continue,
        };
        let value = value.trim().parse::<f32>().ok()?;
        rgba[index] = (value.clamp(0.0, 1.0) * 255.0) as u8;
        seen[index] = true;
    }

    seen.iter().all(|seen| *seen).then_some(rgba)
}

fn parse_scalar_value(line: &str) -> Option<f32> {
    let (_, value) = line.split_once(':')?;
    value.trim().parse::<f32>().ok()
}

fn parse_vec2_like(line: &str) -> Option<[f32; 2]> {
    let start = line.find('{')? + 1;
    let end = line.rfind('}')?;
    let mut values = [0.0f32; 2];
    let mut seen = [false; 2];

    for part in line[start..end].split(',') {
        let (axis, value) = part.trim().split_once(':')?;
        let index = match axis.trim() {
            "x" => 0,
            "y" => 1,
            _ => continue,
        };
        values[index] = value.trim().parse::<f32>().ok()?;
        seen[index] = true;
    }

    seen.iter().all(|seen| *seen).then_some(values)
}

fn shader_name_from_shader_asset(text: &str) -> Option<&str> {
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("Shader \""))?;
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

fn builtin_shader_name(file_id: &str) -> Option<&'static str> {
    match file_id {
        "10750" => Some("Unlit/Transparent Cutout"),
        "10752" => Some("Unlit/Transparent"),
        _ => None,
    }
}

fn shader_name_uses_alpha_blend(shader_name: &str) -> bool {
    shader_name == "Unlit/Transparent" || shader_name.contains("ColorTransparent")
}

fn shader_name_uses_alpha8bit(shader_name: &str) -> bool {
    shader_name.contains("Alpha8Bit")
}

fn guid_prefix(guid: &str) -> &str {
    guid.get(..8).unwrap_or(guid)
}

pub(crate) fn texture_name_for_guid(guid: &str) -> Option<&'static str> {
    runtime_texture_name_for_guid(guid)
}

pub(crate) fn material_texture_name_for_guid(guid: &str) -> Option<&'static str> {
    runtime_material_texture_name_for_guid(guid)
}

pub(crate) fn material_texture_name_for_guid_prefix(guid: &str) -> Option<&'static str> {
    runtime_material_texture_name_for_guid_prefix(guid)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_color_for_guid(guid: &str) -> Option<[u8; 3]> {
    material_color_rgba_for_guid(guid).map(|[red, green, blue, _]| [red, green, blue])
}

pub(crate) fn material_color_for_guid_prefix(guid: &str) -> Option<[u8; 3]> {
    material_color_rgba_for_guid_prefix(guid).map(|[red, green, blue, _]| [red, green, blue])
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_color_rgba_for_guid(guid: &str) -> Option<[u8; 4]> {
    runtime_material_color_rgba_for_guid(guid)
}

pub(crate) fn material_color_rgba_for_guid_prefix(guid: &str) -> Option<[u8; 4]> {
    runtime_material_color_rgba_for_guid_prefix(guid)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_alpha_blend_for_guid(guid: &str) -> bool {
    runtime_material_alpha_blend_for_guid(guid)
}

pub(crate) fn material_alpha_blend_for_guid_prefix(guid: &str) -> bool {
    runtime_material_alpha_blend_for_guid_prefix(guid)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_alpha8bit_for_guid(guid: &str) -> bool {
    runtime_material_alpha8bit_for_guid(guid)
}

pub(crate) fn material_alpha8bit_for_guid_prefix(guid: &str) -> bool {
    runtime_material_alpha8bit_for_guid_prefix(guid)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_cutoff_for_guid(guid: &str) -> Option<f32> {
    runtime_material_cutoff_for_guid(guid)
}

pub(crate) fn material_cutoff_for_guid_prefix(guid: &str) -> Option<f32> {
    runtime_material_cutoff_for_guid_prefix(guid)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn material_custom_render_queue_for_guid(guid: &str) -> Option<i32> {
    runtime_material_custom_render_queue_for_guid(guid)
}

pub(crate) fn material_custom_render_queue_for_guid_prefix(guid: &str) -> Option<i32> {
    runtime_material_custom_render_queue_for_guid_prefix(guid)
}

pub(crate) fn material_shader_kind_for_guid_prefix(guid: &str) -> Option<MaterialShaderKind> {
    runtime_material_shader_kind_for_guid_prefix(guid)
}

pub(crate) fn material_shader_kind_for_guid(guid: &str) -> Option<MaterialShaderKind> {
    runtime_material_shader_kind_for_guid(guid)
}

pub(crate) fn material_main_tex_st_for_guid_prefix(guid: &str) -> Option<[f32; 4]> {
    runtime_material_main_tex_st_for_guid_prefix(guid)
}

pub(crate) fn material_main_tex_st_for_guid(guid: &str) -> Option<[f32; 4]> {
    runtime_material_main_tex_st_for_guid(guid)
}

/// Derive the level-refs key from a filename (strip `.bytes` extension).
pub fn level_key_from_filename(filename: &str) -> String {
    filename
        .strip_suffix(".bytes")
        .unwrap_or(filename)
        .to_string()
}

/// Look up a terrain texture filename by level key and reference index.
pub fn get_level_ref(level_key: &str, ref_index: i32) -> Option<&'static str> {
    data()
        .refs
        .get(level_key)
        .and_then(|m| m.get(&ref_index))
        .map(|s| s.as_str())
}

/// Returns true when loader m_references contains an explicit null entry ({fileID: 0})
/// at this index.
pub fn is_level_ref_null(level_key: &str, ref_index: i32) -> bool {
    data()
        .null_refs
        .get(level_key)
        .is_some_and(|set| set.contains(&ref_index))
}

/// Get the resolved prefab name for a loader prefab index.
pub fn get_prefab_override(level_key: &str, prefab_index: i16) -> Option<&'static str> {
    data()
        .prefabs
        .get(level_key)
        .and_then(|m| m.get(&prefab_index))
        .map(|s| s.as_str())
}

/// Get the resolved background prefab asset stem for a loader reference index.
pub fn get_background_prefab_ref(level_key: &str, ref_index: i32) -> Option<&'static str> {
    data()
        .background_prefabs
        .get(level_key)
        .and_then(|m| m.get(&ref_index))
        .map(|s| s.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        MaterialShaderKind, get_background_prefab_ref, get_level_ref, get_prefab_override,
        is_level_ref_null, level_key_from_filename, load_embedded_loaders,
        load_prefab_names_by_file_id, material_alpha_blend_for_guid_prefix,
        material_alpha8bit_for_guid_prefix, material_color_for_guid,
        material_color_for_guid_prefix, material_custom_render_queue_for_guid_prefix,
        material_cutoff_for_guid_prefix, material_main_tex_st_for_guid,
        material_main_tex_st_for_guid_prefix, material_shader_kind_for_guid,
        material_shader_kind_for_guid_prefix, material_texture_name_for_guid,
        material_texture_name_for_guid_prefix, texture_name_for_guid,
    };
    use crate::data::assets;
    use crate::domain::parser::parse_level;
    use crate::domain::types::LevelObject;
    use std::path::Path;

    fn collect_test_level_paths(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_test_level_paths(&path, out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("bytes") {
                continue;
            }
            let Ok(relative) =
                path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("../test_levels"))
            else {
                continue;
            };
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    fn all_test_level_paths() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../test_levels");
        let mut paths = Vec::new();
        collect_test_level_paths(&root, &mut paths);
        paths.sort();
        paths
    }

    fn parse_test_level(relative_path: &str) -> crate::domain::types::LevelData {
        let level_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels")
            .join(relative_path);
        let bytes = std::fs::read(&level_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", level_path.display()));
        parse_level(bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", level_path.display()))
    }

    #[test]
    fn level_key_from_filename_strips_bytes_suffix() {
        assert_eq!(
            level_key_from_filename("Level_14_data.bytes"),
            "Level_14_data"
        );
    }

    #[test]
    fn runtime_loader_refs_match_basic_level() {
        assert_eq!(
            get_level_ref("Level_14_data", 9),
            Some("Ground_Rocks_Texture.png")
        );
        assert_eq!(
            get_level_ref("Level_14_data", 10),
            Some("Ground_Grass_Texture.png")
        );
        assert_eq!(
            get_level_ref("Level_14_data", 11),
            Some("Ground_Rocks_Outline_Texture.png")
        );
    }

    #[test]
    fn runtime_loader_refs_use_unitypackage_truth_for_episode_6_sandboxes() {
        assert_eq!(
            get_level_ref("Episode_6_Ice Sandbox_data", 7),
            Some("Ground_Temple_Dark_Texture_02.png")
        );
        assert_eq!(
            get_level_ref("Episode_6_Ice Sandbox_data", 161),
            Some("Ground_Temple_Tile_Texture.png")
        );
    }

    #[test]
    fn runtime_loader_refs_use_correct_outline_pathnames() {
        assert_eq!(
            texture_name_for_guid("6ddb5c22ed9534b248104e4ab9bfaa4e"),
            Some("Border.png")
        );
        assert_eq!(
            texture_name_for_guid("977533b9ad59c2ae434820adb5b74076"),
            Some("Border_Maya_Cave.png")
        );
        assert_eq!(
            texture_name_for_guid("f2ac7f939745a094490b25021eef34be"),
            Some("Ground_Rocks_Outline_Texture_03.png")
        );
        assert_eq!(
            texture_name_for_guid("45852d194a2aa49342a35ed790392320"),
            Some("Ground_Temple_Dark_Texture.png")
        );
        assert_eq!(
            texture_name_for_guid("3ed677047877dc834d24835296ed719b"),
            Some("defaultcurvetexture.png")
        );
        assert_eq!(
            texture_name_for_guid("413dd7c251004ea54c009643f3907841"),
            Some("Ground_Rocks_Texture_05.png")
        );
        assert_eq!(
            texture_name_for_guid("7913446d9ac5efad454f59abc28c84ac"),
            Some("Ground_Temple_Tile_Texture.png")
        );
        assert_eq!(
            texture_name_for_guid("88e44c05da33a0b449016bd36686ac9c"),
            Some("Ground_Temple_Dark_Texture_02.png")
        );
        assert_eq!(
            texture_name_for_guid("f02b58597a8fa8b549c6d1e3fec2651a"),
            Some("Ground_Temple_Rock_Texture.png")
        );
        assert_eq!(
            get_level_ref("scenario_12_data", 17),
            Some("Ground_Rocks_Outline_Texture_03.png")
        );
    }

    #[test]
    fn exact_texture_and_material_guids_exist_in_embedded_assets() {
        assert_eq!(
            assets::pathname_for_guid("6ddb5c22ed9534b248104e4ab9bfaa4e"),
            Some("Assets/Texture2D/Border.png")
        );
        assert_eq!(
            assets::pathname_for_guid("884b9b90b5f2e49343f6ec0608bc01c9"),
            Some("Assets/Material/Particles_Sheet_01.mat")
        );
    }

    #[test]
    fn material_shader_flags_and_cutoff_can_come_from_embedded_assets() {
        assert!(material_alpha_blend_for_guid_prefix("c2d365ae"));
        assert_eq!(material_cutoff_for_guid_prefix("c2d365ae"), Some(0.5));

        assert!(material_alpha_blend_for_guid_prefix("b9dc7627"));
        assert!(!material_alpha8bit_for_guid_prefix("b9dc7627"));
        assert_eq!(material_cutoff_for_guid_prefix("b9dc7627"), Some(0.5));

        assert!(!material_alpha_blend_for_guid_prefix("ddb33f64"));
        assert!(material_alpha8bit_for_guid_prefix("ddb33f64"));
    }

    #[test]
    fn background_material_shader_kinds_and_main_tex_st_can_come_from_embedded_assets() {
        let jungle_fill_guid = assets::guid_for_pathname("Assets/Material/Jungle_Near_Fill.mat")
            .expect("expected Jungle_Near_Fill material guid");
        let jungle_far_guid =
            assets::guid_for_pathname("Assets/Material/Jungle_Environment_Far_BG.mat")
                .expect("expected Jungle_Environment_Far_BG material guid");
        let jungle_sky_guid =
            assets::guid_for_pathname("Assets/Material/Jungle_Environment_Sky.mat")
                .expect("expected Jungle_Environment_Sky material guid");
        let jungle_cutout_guid =
            assets::guid_for_pathname("Assets/Material/Jungle_Environment.mat")
                .expect("expected Jungle_Environment material guid");

        assert_eq!(
            material_shader_kind_for_guid(jungle_fill_guid),
            Some(MaterialShaderKind::CustomUnlitMonochrome)
        );
        assert_eq!(
            material_shader_kind_for_guid(jungle_far_guid),
            Some(MaterialShaderKind::CustomUnlitAlpha8BitColor)
        );
        assert_eq!(
            material_shader_kind_for_guid(jungle_sky_guid),
            Some(MaterialShaderKind::BuiltinUnlitTransparent)
        );
        assert_eq!(
            material_shader_kind_for_guid(jungle_cutout_guid),
            Some(MaterialShaderKind::BuiltinUnlitTransparentCutout)
        );

        assert_eq!(
            material_main_tex_st_for_guid(jungle_far_guid),
            Some([1.0, 1.0, 0.0, 0.0])
        );
        assert_eq!(
            material_main_tex_st_for_guid(jungle_sky_guid),
            Some([1.0, 1.0, 0.0, 0.0])
        );
        assert_eq!(material_main_tex_st_for_guid(jungle_fill_guid), None);

        assert!(material_shader_kind_for_guid_prefix(jungle_far_guid).is_some());
        assert!(material_main_tex_st_for_guid_prefix(jungle_far_guid).is_some());
    }

    #[test]
    fn exact_guid_resolution_matches_known_texture_and_material_assets() {
        assert_eq!(
            texture_name_for_guid("6ddb5c22ed9534b248104e4ab9bfaa4e"),
            Some("Border.png")
        );
        assert_eq!(
            texture_name_for_guid("3ed677047877dc834d24835296ed719b"),
            Some("defaultcurvetexture.png")
        );
        assert_eq!(
            texture_name_for_guid("413dd7c251004ea54c009643f3907841"),
            Some("Ground_Rocks_Texture_05.png")
        );
        assert_eq!(
            material_texture_name_for_guid("884b9b90b5f2e49343f6ec0608bc01c9"),
            Some("Particles_Sheet_01.png")
        );
        assert_eq!(
            material_texture_name_for_guid_prefix("0de59521"),
            Some("Background_Maya_Sheet_02.png")
        );
        assert_eq!(
            material_color_for_guid_prefix("e1c5ada5"),
            Some([0x21, 0x44, 0x21])
        );
        assert_eq!(
            material_color_for_guid("e1c5ada5ac0d118e4b797c296c32cf32"),
            Some([0x21, 0x44, 0x21])
        );
    }

    #[test]
    fn material_custom_render_queue_can_come_from_runtime_material_assets() {
        assert_eq!(
            material_custom_render_queue_for_guid_prefix("0f31fa21"),
            Some(3006)
        );
        assert_eq!(
            material_custom_render_queue_for_guid_prefix("38ea809d"),
            Some(3006)
        );
        assert_eq!(
            material_custom_render_queue_for_guid_prefix("ddb33f64"),
            None
        );
    }

    #[test]
    fn runtime_loader_prefabs_resolve_names() {
        assert_eq!(get_prefab_override("Level_51_data", 17), Some("Grass_09_0"));
        assert_eq!(get_prefab_override("Level_51_data", 19), Some("Grass_11_0"));
        assert_eq!(
            get_prefab_override("Level_Sandbox_06_data", 15),
            Some("Bush_03_0")
        );
    }

    #[test]
    fn runtime_loader_background_refs_resolve_episode6_background_prefabs() {
        assert_eq!(
            get_background_prefab_ref("episode_6_level_5_data", 4),
            Some("background_mm_cave_02_set_dark")
        );
        assert_eq!(
            get_background_prefab_ref("Episode_6_Ice Sandbox_data", 4),
            Some("background_mm_temple_01_set_01")
        );
        assert_eq!(
            get_background_prefab_ref("Episode_6_Tower Sandbox_data", 4),
            Some("background_mm_01_set")
        );
    }

    fn terrain_splat1_names_for_level(
        relative_path: &str,
        resolved_prefab_name: &str,
    ) -> std::collections::BTreeSet<String> {
        let level = parse_test_level(relative_path);
        let filename = Path::new(relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("test level file name");
        let level_key = level_key_from_filename(filename);

        level
            .objects
            .iter()
            .filter_map(|object| {
                let LevelObject::Prefab(prefab) = object else {
                    return None;
                };
                let terrain = prefab.terrain_data.as_deref()?;
                let resolved_name = get_prefab_override(&level_key, prefab.prefab_index)
                    .unwrap_or(prefab.name.as_str());
                if resolved_name != resolved_prefab_name {
                    return None;
                }
                let splat1_index = terrain.curve_textures.get(1)?.texture_index;
                get_level_ref(&level_key, splat1_index).map(str::to_string)
            })
            .collect()
    }

    #[test]
    fn runtime_loader_refs_keep_mm_maya_splat1_prefab_groups_from_unity_assets() {
        let border = texture_name_for_guid("6ddb5c22ed9534b248104e4ab9bfaa4e")
            .expect("Border guid")
            .to_string();
        let border_maya_cave = texture_name_for_guid("977533b9ad59c2ae434820adb5b74076")
            .expect("Border_Maya_Cave guid")
            .to_string();

        assert_eq!(
            terrain_splat1_names_for_level(
                "assetbundles/episode_6_levels.unity3d/episode_6_level_10_data.bytes",
                "e2dTerrainBase_MM_rock",
            ),
            std::collections::BTreeSet::from([border.clone()])
        );
        assert_eq!(
            terrain_splat1_names_for_level(
                "assetbundles/episode_6_levels.unity3d/episode_6_level_17_data.bytes",
                "e2dTerrainBase_MM_rock",
            ),
            std::collections::BTreeSet::from([border_maya_cave.clone()])
        );
        assert_eq!(
            terrain_splat1_names_for_level(
                "assetbundles/episode_6_levels.unity3d/episode_6_level_18_data.bytes",
                "e2dTerrainDark_MM_TempleDarkRock",
            ),
            std::collections::BTreeSet::from([border_maya_cave])
        );
    }

    #[test]
    fn course_02_loader_keeps_explicit_null_ref_zero() {
        assert!(is_level_ref_null("course_02_data", 0));
        assert!(!is_level_ref_null("course_02_data", 8));
        assert!(get_level_ref("course_02_data", 8).is_some());
    }

    #[test]
    #[ignore]
    fn debug_loader_prefab_mapping_for_star_crystal_and_rock() {
        let loaders = load_embedded_loaders().expect("embedded loaders");
        let loader_by_level = loaders
            .iter()
            .map(|loader| (loader.level_key.as_str(), loader))
            .collect::<std::collections::HashMap<_, _>>();
        let prefab_names = load_prefab_names_by_file_id().expect("prefab names by file id");

        let mut star_printed = 0usize;
        let mut crystal_printed = 0usize;
        let mut rock_printed = 0usize;

        for relative_path in all_test_level_paths() {
            let level = parse_test_level(&relative_path);
            let filename = Path::new(&relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test level file name");
            let level_key = level_key_from_filename(filename);
            let Some(loader) = loader_by_level.get(level_key.as_str()) else {
                continue;
            };

            for object in &level.objects {
                let LevelObject::Prefab(prefab) = object else {
                    continue;
                };
                let resolved = get_prefab_override(&level_key, prefab.prefab_index)
                    .unwrap_or(prefab.name.as_str());
                let bucket = if resolved.starts_with("Star_") {
                    &mut star_printed
                } else if resolved.starts_with("Crystal_") {
                    &mut crystal_printed
                } else if resolved == "Rock_01" {
                    &mut rock_printed
                } else {
                    continue;
                };
                if *bucket >= 6 {
                    continue;
                }

                let loader_file_id = usize::try_from(prefab.prefab_index)
                    .ok()
                    .and_then(|index| loader.prefab_file_ids.get(index))
                    .cloned();
                let loader_name = loader_file_id
                    .as_deref()
                    .and_then(|file_id| prefab_names.get(file_id))
                    .cloned();

                println!(
                    "{}: raw={} prefab_index={} resolved={} loader_file_id={:?} loader_name={:?}",
                    relative_path,
                    prefab.name,
                    prefab.prefab_index,
                    resolved,
                    loader_file_id,
                    loader_name,
                );

                *bucket += 1;
                if star_printed >= 6 && crystal_printed >= 6 && rock_printed >= 6 {
                    return;
                }
            }
        }

        println!(
            "printed_counts star={} crystal={} rock={}",
            star_printed, crystal_printed, rock_printed
        );
    }
}
