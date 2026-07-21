use std::path::{Path, PathBuf};

pub(crate) fn external_test_levels_root() -> PathBuf {
    std::env::var_os("BP_EDITOR_TEST_LEVELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../test_levels"))
}

pub(crate) fn external_test_level(relative_path: &str) -> Option<PathBuf> {
    let path = external_test_levels_root().join(relative_path);
    path.is_file().then_some(path)
}
