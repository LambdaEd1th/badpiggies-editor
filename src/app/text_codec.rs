//! Text-format level (de)serialization helpers used by the UI layer.

use crate::diagnostics::error::{AppError, AppResult};
use crate::i18n::locale::I18n;
use crate::domain::types::LevelData;

pub(super) fn status_parse_error_message(i18n: &I18n, error: impl Into<AppError>) -> String {
    let error = error.into();
    i18n.fmt1("status_parse_error", &error.localized(i18n))
}

pub(super) fn parse_level_text(name: &str, text: &str) -> AppResult<LevelData> {
    if name.ends_with(".yaml") || name.ends_with(".yml") {
        serde_yaml::from_str(text).map_err(|error| {
            AppError::invalid_data_key1("error_parse_yaml_level", error.to_string())
        })
    } else if name.ends_with(".toml") {
        toml::from_str(text).map_err(|error| {
            AppError::invalid_data_key1("error_parse_toml_level", error.to_string())
        })
    } else {
        Err(AppError::invalid_data_key("error_unsupported_file_format"))
    }
}

pub(super) fn serialize_level_yaml(level: &LevelData) -> AppResult<String> {
    serde_yaml::to_string(level).map_err(|error| {
        AppError::invalid_data_key1("error_serialize_yaml_level", error.to_string())
    })
}

pub(super) fn serialize_level_toml(level: &LevelData) -> AppResult<String> {
    toml::to_string_pretty(level).map_err(|error| {
        AppError::invalid_data_key1("error_serialize_toml_level", error.to_string())
    })
}
