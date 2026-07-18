//! Platform-neutral runtime asset registry.
//!
//! The app owns filesystem and browser I/O, then installs bytes here before any
//! parser or localization service accesses them.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_cache() -> MutexGuard<'static, HashMap<String, Vec<u8>>> {
    cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn install_runtime_asset(relative_path: impl Into<String>, bytes: Vec<u8>) {
    lock_cache().insert(relative_path.into(), bytes);
}

pub fn install_runtime_assets(assets: impl IntoIterator<Item = (String, Vec<u8>)>) {
    let mut cache = lock_cache();
    cache.extend(assets);
}

pub fn contains_runtime_asset(relative_path: &str) -> bool {
    lock_cache().contains_key(relative_path)
}

pub fn read_runtime_asset_bytes(relative_path: &str) -> Vec<u8> {
    if let Some(bytes) = lock_cache().get(relative_path).cloned() {
        return bytes;
    }

    #[cfg(test)]
    {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/assets")
            .join(relative_path);
        if let Ok(bytes) = std::fs::read(path) {
            install_runtime_asset(relative_path, bytes.clone());
            return bytes;
        }
    }

    panic!("runtime asset was not installed: {relative_path}")
}

/// Move a large one-shot runtime asset out of the registry without cloning it.
///
/// Archive-backed loaders use this once when constructing their permanent index;
/// retaining the compressed source bytes after that only duplicates memory.
pub fn take_runtime_asset_bytes(relative_path: &str) -> Vec<u8> {
    if let Some(bytes) = lock_cache().remove(relative_path) {
        return bytes;
    }

    #[cfg(test)]
    {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/assets")
            .join(relative_path);
        if let Ok(bytes) = std::fs::read(path) {
            return bytes;
        }
    }

    panic!("runtime asset was not installed: {relative_path}")
}

pub fn read_runtime_asset_text(relative_path: &str) -> String {
    let bytes = read_runtime_asset_bytes(relative_path);
    String::from_utf8(bytes).unwrap_or_else(|error| {
        panic!("runtime asset is not valid UTF-8 ({relative_path}): {error}")
    })
}

pub fn list_runtime_assets(prefix: &str, suffix: &str) -> Vec<String> {
    let cache = lock_cache();
    let mut paths = cache
        .keys()
        .filter(|path| path.starts_with(prefix) && path.ends_with(suffix))
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn missing_runtime_assets(required: &[&str]) -> Vec<String> {
    let cache = lock_cache();
    required
        .iter()
        .filter(|path| !cache.contains_key(**path))
        .map(|path| (*path).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{install_runtime_asset, list_runtime_assets, read_runtime_asset_text};

    #[test]
    fn installed_assets_can_be_read_and_listed() {
        install_runtime_asset("test-registry/example.ftl", b"hello".to_vec());
        assert_eq!(
            read_runtime_asset_text("test-registry/example.ftl"),
            "hello"
        );
        assert_eq!(
            list_runtime_assets("test-registry/", ".ftl"),
            vec!["test-registry/example.ftl"]
        );
    }
}
