//! Project asset loading from a Unity `.unitypackage` file.
//!
//! Unity package entries are expected in the standard layout:
//!     <guid>/asset
//!     <guid>/asset.meta
//!     <guid>/pathname
//!
//! Lookups are addressed by Unity pathname (`"Assets/..."`) at runtime.

use std::borrow::Cow;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::io::{Cursor, Read};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use tar::Archive;

#[cfg(not(target_arch = "wasm32"))]
const EXTERNAL_UNITYPACKAGE_PATH_ENV: &str = "BP_EDITOR_EXTERNAL_UNITYPACKAGE_PATH";
#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_UNITYPACKAGE_RELATIVE_PATH: &str =
    "assets/data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage";
#[cfg(target_arch = "wasm32")]
const DEFAULT_UNITYPACKAGE_RUNTIME_ASSET: &str =
    "data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage";

struct ExternalAssetIndex {
    asset_bytes_by_pathname: HashMap<String, Vec<u8>>,
    by_pathname: HashMap<String, String>,
    by_guid: HashMap<String, String>,
}

#[derive(Default)]
struct UnityPackageEntry {
    pathname: Option<String>,
    asset_bytes: Option<Vec<u8>>,
    meta_text: Option<String>,
}

fn external_index() -> Option<&'static ExternalAssetIndex> {
    static EXTERNAL_INDEX: OnceLock<Option<ExternalAssetIndex>> = OnceLock::new();
    EXTERNAL_INDEX
        .get_or_init(|| {
            #[cfg(target_arch = "wasm32")]
            {
                let bytes = crate::data::runtime_assets::read_runtime_asset_bytes(
                    DEFAULT_UNITYPACKAGE_RUNTIME_ASSET,
                );
                Some(build_external_index_from_unitypackage_bytes(
                    bytes,
                    DEFAULT_UNITYPACKAGE_RUNTIME_ASSET,
                ))
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let package_path = env::var_os(EXTERNAL_UNITYPACKAGE_PATH_ENV)
                    .map(PathBuf::from)
                    .unwrap_or_else(default_unitypackage_path);

                if !package_path.is_file() {
                    panic!(
                        "Unity package not found at {}. Set {} or place package at {}.",
                        package_path.display(),
                        EXTERNAL_UNITYPACKAGE_PATH_ENV,
                        DEFAULT_UNITYPACKAGE_RELATIVE_PATH
                    );
                }

                Some(build_external_index_from_unitypackage(package_path))
            }
        })
        .as_ref()
}

#[cfg(not(target_arch = "wasm32"))]
fn default_unitypackage_path() -> PathBuf {
    env::current_dir()
        .expect("failed to read current working directory")
        .join(DEFAULT_UNITYPACKAGE_RELATIVE_PATH)
}

#[cfg(not(target_arch = "wasm32"))]
fn build_external_index_from_unitypackage(package_path: PathBuf) -> ExternalAssetIndex {
    let mut bytes = Vec::new();
    let file = fs::File::open(&package_path).unwrap_or_else(|error| {
        panic!(
            "Failed to open unitypackage {}: {}",
            package_path.display(),
            error
        )
    });
    let mut reader = std::io::BufReader::new(file);
    reader.read_to_end(&mut bytes).unwrap_or_else(|error| {
        panic!(
            "Failed to read unitypackage bytes {}: {}",
            package_path.display(),
            error
        )
    });

    build_external_index_from_unitypackage_bytes(bytes, &package_path.display().to_string())
}

fn build_external_index_from_unitypackage_bytes(
    bytes: Vec<u8>,
    source_name: &str,
) -> ExternalAssetIndex {
    let mut per_guid: HashMap<String, UnityPackageEntry> = HashMap::new();
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(decoder);

    let entries = archive.entries().unwrap_or_else(|error| {
        panic!(
            "Failed to read unitypackage entries {}: {}",
            source_name, error
        )
    });

    for entry_result in entries {
        let mut entry = entry_result.unwrap_or_else(|error| {
            panic!(
                "Failed to read unitypackage entry {}: {}",
                source_name, error
            )
        });
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path().unwrap_or_else(|error| {
            panic!(
                "Failed to parse unitypackage entry path {}: {}",
                source_name, error
            )
        });
        let parts: Vec<_> = path.iter().collect();
        if parts.len() != 2 {
            continue;
        }

        let guid = parts[0].to_string_lossy().to_string();
        let leaf = parts[1].to_string_lossy().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap_or_else(|error| {
            panic!(
                "Failed to read unitypackage entry bytes {}: {}",
                source_name, error
            )
        });

        let slot = per_guid.entry(guid).or_default();
        match leaf.as_str() {
            "pathname" => {
                let text = String::from_utf8_lossy(&data).trim().to_string();
                if !text.is_empty() {
                    slot.pathname = Some(text);
                }
            }
            "asset" => {
                slot.asset_bytes = Some(data);
            }
            "asset.meta" => {
                slot.meta_text = Some(String::from_utf8_lossy(&data).into_owned());
            }
            _ => {}
        }
    }

    let mut by_pathname: HashMap<String, String> = HashMap::new();
    let mut by_guid: HashMap<String, String> = HashMap::new();
    let mut asset_bytes_by_pathname: HashMap<String, Vec<u8>> = HashMap::new();

    for (folder_guid, entry) in per_guid {
        let Some(pathname) = entry.pathname else {
            continue;
        };
        let Some(asset_bytes) = entry.asset_bytes else {
            continue;
        };

        let guid = entry
            .meta_text
            .as_deref()
            .and_then(read_guid_from_meta_text)
            .unwrap_or(folder_guid);

        by_guid.insert(guid.clone(), pathname.clone());
        by_pathname.insert(pathname.clone(), guid);
        asset_bytes_by_pathname.insert(pathname, asset_bytes);
    }

    ExternalAssetIndex {
        asset_bytes_by_pathname,
        by_pathname,
        by_guid,
    }
}

fn read_guid_from_meta_text(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("guid:")
            .map(|value| value.trim().to_string())
            .filter(|guid| !guid.is_empty())
    })
}

fn read_asset_by_guid(guid: &str) -> Option<Cow<'static, [u8]>> {
    let index = external_index()?;
    let pathname = index.by_guid.get(guid)?;
    let bytes = read_external_asset_by_pathname(index, pathname)?;
    Some(Cow::Owned(bytes))
}

fn read_external_asset_by_pathname(index: &ExternalAssetIndex, pathname: &str) -> Option<Vec<u8>> {
    index.asset_bytes_by_pathname.get(pathname).cloned()
}

fn pathname_by_guid(guid: &str) -> Option<&'static str> {
    let index = external_index()?;
    index.by_guid.get(guid).map(|pathname| pathname.as_str())
}

pub fn guid_for_pathname(pathname: &str) -> Option<&'static str> {
    let index = external_index()?;
    let guid = index.by_pathname.get(pathname)?;
    if guid.is_empty() {
        return None;
    }
    Some(guid.as_str())
}

/// Read by full Unity pathname, e.g. `"Assets/Texture2D/foo.png"`.
pub fn read_pathname(pathname: &str) -> Option<Cow<'static, [u8]>> {
    let index = external_index()?;
    let bytes = read_external_asset_by_pathname(index, pathname)?;
    Some(Cow::Owned(bytes))
}

pub fn read_pathname_text(pathname: &str) -> Option<String> {
    read_pathname(pathname).map(|bytes| String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

pub fn read_guid_text(guid: &str) -> Option<String> {
    read_asset_by_guid(guid).map(|bytes| String::from_utf8_lossy(bytes.as_ref()).into_owned())
}

pub fn pathname_for_guid(guid: &str) -> Option<&'static str> {
    pathname_by_guid(guid)
}

/// Iterate full Unity pathnames matching `prefix` and `suffix`.
/// Example: `list_pathnames("Assets/Prefab/", ".prefab")` returns
/// `["Assets/Prefab/Cake_01.prefab", ...]`.
pub fn list_pathnames(prefix: &str, suffix: &str) -> Vec<String> {
    let Some(index) = external_index() else {
        return Vec::new();
    };

    let mut out: Vec<String> = index
        .by_pathname
        .keys()
        .filter(|p| p.starts_with(prefix) && p.ends_with(suffix))
        .cloned()
        .collect();
    out.sort();
    out
}
