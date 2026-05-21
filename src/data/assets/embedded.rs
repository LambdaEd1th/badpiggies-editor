//! Project asset loading via rust-embed.
//!
//! On-disk layout is the native Unity package layout:
//!     unity_assets/<guid>/asset
//!     unity_assets/<guid>/asset.meta
//!     unity_assets/<guid>/pathname
//!
//! Lookups go through a pathname index built once at startup. The GUID is
//! used internally as the rust-embed key; consumers address assets by Unity
//! pathname (`"Assets/Texture2D/foo.png"`).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

#[path = "generated.rs"]
mod generated;

pub use generated::ProjectAssets;

struct AssetIndex {
    /// "Assets/<...>" pathname -> GUID
    by_pathname: HashMap<String, String>,
    /// GUID -> "Assets/<...>" pathname
    by_guid: HashMap<String, String>,
}

fn index() -> &'static AssetIndex {
    static INDEX: OnceLock<AssetIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut by_pathname: HashMap<String, String> = HashMap::new();
        let mut by_guid: HashMap<String, String> = HashMap::new();
        for path in ProjectAssets::iter() {
            let path_ref: &str = path.as_ref();
            let Some(guid) = path_ref.strip_suffix("/pathname") else {
                continue;
            };
            let Some(file) = ProjectAssets::get(path_ref) else {
                continue;
            };
            let pathname = String::from_utf8_lossy(file.data.as_ref())
                .trim()
                .to_string();
            if pathname.is_empty() {
                continue;
            }
            if ProjectAssets::get(&format!("{guid}/asset")).is_none() {
                continue;
            }
            by_guid.insert(guid.to_string(), pathname.clone());
            by_pathname.insert(pathname, guid.to_string());
        }
        AssetIndex {
            by_pathname,
            by_guid,
        }
    })
}

fn read_asset_by_guid(guid: &str) -> Option<Cow<'static, [u8]>> {
    ProjectAssets::get(&format!("{guid}/asset")).map(|f| f.data)
}

fn pathname_by_guid(guid: &str) -> Option<&'static str> {
    index().by_guid.get(guid).map(|s| s.as_str())
}

pub fn guid_for_pathname(pathname: &str) -> Option<&'static str> {
    index().by_pathname.get(pathname).map(|s| s.as_str())
}

/// Read by full Unity pathname, e.g. `"Assets/Texture2D/foo.png"`.
pub fn read_pathname(pathname: &str) -> Option<Cow<'static, [u8]>> {
    read_asset_by_guid(guid_for_pathname(pathname)?)
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
    let mut out: Vec<String> = index()
        .by_pathname
        .keys()
        .filter(|p| p.starts_with(prefix) && p.ends_with(suffix))
        .cloned()
        .collect();
    out.sort();
    out
}
