use crate::editor_state::ThemePreference;

#[cfg(target_arch = "wasm32")]
const THEME_PREFERENCE_KEY: &str = "badpiggies-editor-theme";

#[cfg(not(target_arch = "wasm32"))]
pub fn read_theme_preference() -> ThemePreference {
    let Some(path) = theme_preference_path() else {
        return ThemePreference::System;
    };
    std::fs::read_to_string(path)
        .ok()
        .map(|value| ThemePreference::from_code(&value))
        .unwrap_or(ThemePreference::System)
}

#[cfg(target_arch = "wasm32")]
pub fn read_theme_preference() -> ThemePreference {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(THEME_PREFERENCE_KEY).ok().flatten())
        .map(|value| ThemePreference::from_code(&value))
        .unwrap_or(ThemePreference::System)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_theme_preference(theme: ThemePreference) -> Result<(), String> {
    let path = theme_preference_path()
        .ok_or_else(|| "could not resolve theme preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, theme.code()).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn save_theme_preference(theme: ThemePreference) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
    let storage = window
        .local_storage()
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| "localStorage is unavailable".to_string())?;
    storage
        .set_item(THEME_PREFERENCE_KEY, theme.code())
        .map_err(|error| format!("{error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn theme_preference_path() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("badpiggies-editor").join("theme"))
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|home| home.join("Library").join("Application Support"))
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(std::path::PathBuf::from)
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
fn config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".config"))
        })
}
