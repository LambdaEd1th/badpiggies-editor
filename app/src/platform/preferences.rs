use crate::editor_state::ThemePreference;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_DEFAULT_WIDTH: u32 = 1440;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_DEFAULT_HEIGHT: u32 = 900;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_MIN_WIDTH: u32 = 1180;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const DESKTOP_WINDOW_MIN_HEIGHT: u32 = 720;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowSizePreference {
    pub width: u32,
    pub height: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for WindowSizePreference {
    fn default() -> Self {
        Self {
            width: DESKTOP_WINDOW_DEFAULT_WIDTH,
            height: DESKTOP_WINDOW_DEFAULT_HEIGHT,
        }
    }
}

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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_window_size_preference() -> WindowSizePreference {
    let Some(path) = window_size_preference_path() else {
        return WindowSizePreference::default();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| parse_window_size_preference(&value))
        .map(clamp_window_size_preference)
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_window_size_preference(width: u32, height: u32) -> Result<(), String> {
    let size = clamp_window_size_preference(WindowSizePreference { width, height });
    let path = window_size_preference_path()
        .ok_or_else(|| "could not resolve window size preference path".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, format!("{}x{}", size.width, size.height))
        .map_err(|error| error.to_string())
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

#[cfg(not(target_arch = "wasm32"))]
fn window_size_preference_path() -> Option<std::path::PathBuf> {
    Some(config_dir()?.join("badpiggies-editor").join("window-size"))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_window_size_preference(value: &str) -> Option<WindowSizePreference> {
    let (width, height) = value.trim().split_once(['x', 'X', ','])?;
    Some(WindowSizePreference {
        width: width.trim().parse().ok()?,
        height: height.trim().parse().ok()?,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn clamp_window_size_preference(size: WindowSizePreference) -> WindowSizePreference {
    WindowSizePreference {
        width: size.width.max(DESKTOP_WINDOW_MIN_WIDTH),
        height: size.height.max(DESKTOP_WINDOW_MIN_HEIGHT),
    }
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

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use super::{
        DESKTOP_WINDOW_MIN_HEIGHT, DESKTOP_WINDOW_MIN_WIDTH, WindowSizePreference,
        clamp_window_size_preference, parse_window_size_preference,
    };

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn parses_window_size_preferences() {
        assert_eq!(
            parse_window_size_preference("1440x900"),
            Some(WindowSizePreference {
                width: 1440,
                height: 900,
            })
        );
        assert_eq!(
            parse_window_size_preference("1280, 760"),
            Some(WindowSizePreference {
                width: 1280,
                height: 760,
            })
        );
        assert_eq!(parse_window_size_preference("bad"), None);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn clamps_window_size_preferences() {
        assert_eq!(
            clamp_window_size_preference(WindowSizePreference {
                width: 100,
                height: 100,
            }),
            WindowSizePreference {
                width: DESKTOP_WINDOW_MIN_WIDTH,
                height: DESKTOP_WINDOW_MIN_HEIGHT,
            }
        );
    }
}
