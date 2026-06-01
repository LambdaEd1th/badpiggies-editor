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
#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(not(target_arch = "wasm32"))]
use flate2::read::GzDecoder;
#[cfg(not(target_arch = "wasm32"))]
use tar::Archive;

#[path = "generated.rs"]
mod generated;

pub use generated::ProjectAssets;

#[cfg(not(target_arch = "wasm32"))]
const EXTERNAL_ASSETS_DIR_ENV: &str = "BP_EDITOR_EXTERNAL_ASSETS_DIR";
#[cfg(not(target_arch = "wasm32"))]
const EXTERNAL_UNITYPACKAGE_PATH_ENV: &str = "BP_EDITOR_EXTERNAL_UNITYPACKAGE_PATH";

struct AssetIndex {
    /// "Assets/<...>" pathname -> GUID
    by_pathname: HashMap<String, String>,
    /// GUID -> "Assets/<...>" pathname
    by_guid: HashMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct ExternalAssetIndex {
    source: ExternalAssetSource,
    by_pathname: HashMap<String, String>,
    by_guid: HashMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
enum ExternalAssetSource {
    Directory(PathBuf),
    UnityPackage {
        asset_bytes_by_pathname: HashMap<String, Vec<u8>>,
    },
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct UnityPackageEntry {
    pathname: Option<String>,
    asset_bytes: Option<Vec<u8>>,
    meta_text: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
fn external_index() -> Option<&'static ExternalAssetIndex> {
    static EXTERNAL_INDEX: OnceLock<Option<ExternalAssetIndex>> = OnceLock::new();
    EXTERNAL_INDEX
        .get_or_init(|| {
            let external_assets_dir = env::var_os(EXTERNAL_ASSETS_DIR_ENV).map(PathBuf::from);
            let external_unitypackage =
                env::var_os(EXTERNAL_UNITYPACKAGE_PATH_ENV).map(PathBuf::from);

            if let Some(root_path) = external_assets_dir {
                if !root_path.is_dir() {
                    panic!(
                        "{} points to a non-directory path: {}",
                        EXTERNAL_ASSETS_DIR_ENV,
                        root_path.display()
                    );
                }
                if external_unitypackage.is_some() {
                    log::warn!(
                        "Both {} and {} are set; {} takes precedence.",
                        EXTERNAL_ASSETS_DIR_ENV,
                        EXTERNAL_UNITYPACKAGE_PATH_ENV,
                        EXTERNAL_ASSETS_DIR_ENV
                    );
                }
                return Some(build_external_index_from_assets_dir(root_path));
            }

            if let Some(package_path) = external_unitypackage {
                if !package_path.is_file() {
                    panic!(
                        "{} points to a non-file path: {}",
                        EXTERNAL_UNITYPACKAGE_PATH_ENV,
                        package_path.display()
                    );
                }
                return Some(build_external_index_from_unitypackage(package_path));
            }

            None
        })
        .as_ref()
}

#[cfg(not(target_arch = "wasm32"))]
fn build_external_index_from_assets_dir(root_dir: PathBuf) -> ExternalAssetIndex {
    let mut by_pathname: HashMap<String, String> = HashMap::new();
    let mut by_guid: HashMap<String, String> = HashMap::new();
    collect_external_asset_entries(&root_dir, &root_dir, &mut by_pathname, &mut by_guid);
    ExternalAssetIndex {
        source: ExternalAssetSource::Directory(root_dir),
        by_pathname,
        by_guid,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_external_index_from_unitypackage(package_path: PathBuf) -> ExternalAssetIndex {
    let mut per_guid: HashMap<String, UnityPackageEntry> = HashMap::new();

    let file = fs::File::open(&package_path).unwrap_or_else(|error| {
        panic!(
            "Failed to open unitypackage {}: {}",
            package_path.display(),
            error
        )
    });
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let entries = archive.entries().unwrap_or_else(|error| {
        panic!(
            "Failed to read unitypackage entries {}: {}",
            package_path.display(),
            error
        )
    });

    for entry_result in entries {
        let mut entry = entry_result.unwrap_or_else(|error| {
            panic!(
                "Failed to read unitypackage entry {}: {}",
                package_path.display(),
                error
            )
        });
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry.path().unwrap_or_else(|error| {
            panic!(
                "Failed to parse unitypackage entry path {}: {}",
                package_path.display(),
                error
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
                package_path.display(),
                error
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
    let mut missing_entries = Vec::new();

    for (folder_guid, entry) in per_guid {
        let Some(pathname) = entry.pathname else {
            missing_entries.push(format!("{folder_guid}: missing pathname"));
            continue;
        };
        let Some(asset_bytes) = entry.asset_bytes else {
            missing_entries.push(format!("{folder_guid}: missing asset for {pathname}"));
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

    if !missing_entries.is_empty() {
        missing_entries.sort();
        panic!(
            "Unitypackage {} has incomplete entries:\n{}",
            package_path.display(),
            missing_entries
                .into_iter()
                .map(|item| format!("  - {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    ExternalAssetIndex {
        source: ExternalAssetSource::UnityPackage {
            asset_bytes_by_pathname,
        },
        by_pathname,
        by_guid,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_external_asset_entries(
    root_dir: &Path,
    current_dir: &Path,
    by_pathname: &mut HashMap<String, String>,
    by_guid: &mut HashMap<String, String>,
) {
    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_external_asset_entries(root_dir, &path, by_pathname, by_guid);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("meta") {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root_dir) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let pathname = format!("Assets/{relative}");
        let guid = read_guid_from_meta_file(&path).unwrap_or_default();

        if !guid.is_empty() {
            by_guid.insert(guid.clone(), pathname.clone());
        }
        by_pathname.insert(pathname, guid);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_guid_from_meta_file(asset_path: &Path) -> Option<String> {
    let mut meta = asset_path.as_os_str().to_owned();
    meta.push(".meta");
    let meta_path = PathBuf::from(meta);
    let text = fs::read_to_string(meta_path).ok()?;
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("guid:")
            .map(|value| value.trim().to_string())
            .filter(|guid| !guid.is_empty())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_guid_from_meta_text(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("guid:")
            .map(|value| value.trim().to_string())
            .filter(|guid| !guid.is_empty())
    })
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
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(index) = external_index() {
            let pathname = index.by_guid.get(guid)?;
            let bytes = read_external_asset_by_pathname(index, pathname)?;
            return Some(Cow::Owned(bytes));
        }
    }

    ProjectAssets::get(&format!("{guid}/asset")).map(|f| f.data)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_external_asset_by_pathname(index: &ExternalAssetIndex, pathname: &str) -> Option<Vec<u8>> {
    match &index.source {
        ExternalAssetSource::Directory(root_dir) => {
            let relative = pathname.strip_prefix("Assets/")?;
            let path = root_dir.join(relative);
            fs::read(path).ok()
        }
        ExternalAssetSource::UnityPackage {
            asset_bytes_by_pathname,
        } => asset_bytes_by_pathname.get(pathname).cloned(),
    }
}

fn pathname_by_guid(guid: &str) -> Option<&'static str> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(index) = external_index() {
            return index.by_guid.get(guid).map(|pathname| pathname.as_str());
        }
    }

    index().by_guid.get(guid).map(|s| s.as_str())
}

pub fn guid_for_pathname(pathname: &str) -> Option<&'static str> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(index) = external_index() {
            let guid = index.by_pathname.get(pathname)?;
            if guid.is_empty() {
                return None;
            }
            return Some(guid.as_str());
        }
    }

    index().by_pathname.get(pathname).map(|s| s.as_str())
}

/// Read by full Unity pathname, e.g. `"Assets/Texture2D/foo.png"`.
pub fn read_pathname(pathname: &str) -> Option<Cow<'static, [u8]>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(index) = external_index() {
            let bytes = read_external_asset_by_pathname(index, pathname)?;
            return Some(Cow::Owned(bytes));
        }
    }

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
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(index) = external_index() {
            let mut out: Vec<String> = index
                .by_pathname
                .keys()
                .filter(|p| p.starts_with(prefix) && p.ends_with(suffix))
                .cloned()
                .collect();
            out.sort();
            return out;
        }
    }

    let mut out: Vec<String> = index()
        .by_pathname
        .keys()
        .filter(|p| p.starts_with(prefix) && p.ends_with(suffix))
        .cloned()
        .collect();
    out.sort();
    out
}
