use std::fmt;
use std::io;

use crate::i18n::locale::{I18n, Language};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppErrorMessage {
    #[cfg(not(target_arch = "wasm32"))]
    Raw(String),
    Key(&'static str),
    Key1 {
        key: &'static str,
        name: String,
    },
}

impl AppErrorMessage {
    #[cfg(not(target_arch = "wasm32"))]
    fn raw(message: impl Into<String>) -> Self {
        Self::Raw(message.into())
    }

    fn key(key: &'static str) -> Self {
        Self::Key(key)
    }

    fn key1(key: &'static str, name: impl Into<String>) -> Self {
        Self::Key1 {
            key,
            name: name.into(),
        }
    }

    fn localized(&self, i18n: &I18n) -> String {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Raw(message) => message.clone(),
            Self::Key(key) => i18n.get(key),
            Self::Key1 { key, name } => i18n.fmt1(key, name),
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    InvalidData(AppErrorMessage),
    Crypto(AppErrorMessage),
    #[cfg(target_arch = "wasm32")]
    Browser(AppErrorMessage),
    #[cfg(target_arch = "wasm32")]
    State(AppErrorMessage),
}

impl AppError {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::InvalidData(AppErrorMessage::raw(message))
    }

    pub fn invalid_data_key(key: &'static str) -> Self {
        Self::InvalidData(AppErrorMessage::key(key))
    }

    pub fn invalid_data_key1(key: &'static str, name: impl Into<String>) -> Self {
        Self::InvalidData(AppErrorMessage::key1(key, name))
    }

    pub fn crypto_key1(key: &'static str, name: impl Into<String>) -> Self {
        Self::Crypto(AppErrorMessage::key1(key, name))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn browser_key(key: &'static str) -> Self {
        Self::Browser(AppErrorMessage::key(key))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn browser_key1(key: &'static str, name: impl Into<String>) -> Self {
        Self::Browser(AppErrorMessage::key1(key, name))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn state_key(key: &'static str) -> Self {
        Self::State(AppErrorMessage::key(key))
    }

    pub fn localized(&self, i18n: &I18n) -> String {
        match self {
            Self::Io(error) => i18n.fmt1("app_error_io", &error.to_string()),
            Self::InvalidData(message) => localize_variant(i18n, "app_error_invalid_data", message),
            Self::Crypto(message) => localize_variant(i18n, "app_error_crypto", message),
            #[cfg(target_arch = "wasm32")]
            Self::Browser(message) => localize_variant(i18n, "app_error_browser", message),
            #[cfg(target_arch = "wasm32")]
            Self::State(message) => localize_variant(i18n, "app_error_state", message),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn localize_variant(i18n: &I18n, prefix_key: &str, message: &AppErrorMessage) -> String {
    match message {
        AppErrorMessage::Raw(detail) => i18n.fmt1(prefix_key, detail),
        _ => message.localized(i18n),
    }
}

#[cfg(target_arch = "wasm32")]
fn localize_variant(i18n: &I18n, _prefix_key: &str, message: &AppErrorMessage) -> String {
    message.localized(i18n)
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.localized(Language::from_system().i18n()))
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidData(_) | Self::Crypto(_) => None,
            #[cfg(target_arch = "wasm32")]
            Self::Browser(_) | Self::State(_) => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
