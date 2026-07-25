use std::{error::Error, fmt, io};

pub type Result<T> = std::result::Result<T, StillrunError>;

#[derive(Debug)]
pub enum StillrunError {
    InvalidInput(String),
    NotFound(String),
    Unsupported(String),
    Io(io::Error),
    Db(rusqlite::Error),
    Json(serde_json::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl StillrunError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }
}

impl fmt::Display for StillrunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::NotFound(message) => write!(f, "not found: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported: {message}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Db(err) => write!(f, "database error: {err}"),
            Self::Json(err) => write!(f, "json error: {err}"),
            Self::TomlDe(err) => write!(f, "toml decode error: {err}"),
            Self::TomlSer(err) => write!(f, "toml encode error: {err}"),
        }
    }
}

impl Error for StillrunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Db(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::TomlDe(err) => Some(err),
            Self::TomlSer(err) => Some(err),
            Self::InvalidInput(_) | Self::NotFound(_) | Self::Unsupported(_) => None,
        }
    }
}

impl From<io::Error> for StillrunError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for StillrunError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Db(value)
    }
}

impl From<serde_json::Error> for StillrunError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<toml::de::Error> for StillrunError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for StillrunError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}
