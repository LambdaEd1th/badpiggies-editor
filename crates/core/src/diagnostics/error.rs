use std::fmt;
use std::io;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppErrorMessage {
    Raw(String),
    Key(&'static str),
    Key1 { key: &'static str, name: String },
}

impl AppErrorMessage {
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

    pub fn key_name(&self) -> Option<(&'static str, Option<&str>)> {
        match self {
            Self::Raw(_) => None,
            Self::Key(key) => Some((key, None)),
            Self::Key1 { key, name } => Some((key, Some(name))),
        }
    }
}

impl fmt::Display for AppErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(message) => f.write_str(message),
            Self::Key(key) => f.write_str(key),
            Self::Key1 { key, name } => write!(f, "{key}: {name}"),
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    InvalidData(AppErrorMessage),
    Crypto(AppErrorMessage),
    Browser(AppErrorMessage),
    State(AppErrorMessage),
}

impl AppError {
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

    pub fn browser_key(key: &'static str) -> Self {
        Self::Browser(AppErrorMessage::key(key))
    }

    pub fn browser_key1(key: &'static str, name: impl Into<String>) -> Self {
        Self::Browser(AppErrorMessage::key1(key, name))
    }

    pub fn state_key(key: &'static str) -> Self {
        Self::State(AppErrorMessage::key(key))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidData(message) => write!(f, "Invalid data: {message}"),
            Self::Crypto(message) => write!(f, "Crypto error: {message}"),
            Self::Browser(message) => write!(f, "Browser error: {message}"),
            Self::State(message) => write!(f, "State error: {message}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidData(_) | Self::Crypto(_) | Self::Browser(_) | Self::State(_) => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
