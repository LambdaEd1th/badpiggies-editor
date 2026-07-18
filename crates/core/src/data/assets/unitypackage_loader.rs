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
use std::io::{Cursor, Read};
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use tar::Archive;

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
            let bytes = crate::data::runtime_assets::take_runtime_asset_bytes(
                DEFAULT_UNITYPACKAGE_RUNTIME_ASSET,
            );
            Some(build_external_index_from_unitypackage_bytes(
                bytes,
                DEFAULT_UNITYPACKAGE_RUNTIME_ASSET,
            ))
        })
        .as_ref()
}

pub(super) fn prepare_index() {
    let _ = external_index();
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
    Some(Cow::Borrowed(bytes))
}

fn read_external_asset_by_pathname<'a>(
    index: &'a ExternalAssetIndex,
    pathname: &str,
) -> Option<&'a [u8]> {
    index
        .asset_bytes_by_pathname
        .get(pathname)
        .map(Vec::as_slice)
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
    Some(Cow::Borrowed(bytes))
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
