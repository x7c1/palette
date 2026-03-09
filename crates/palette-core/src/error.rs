pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Toml(toml::de::Error),
    Db(palette_db::Error),
    Tmux(palette_tmux::Error),
    Docker(String),
    Internal(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Self::Toml(e)
    }
}

impl From<palette_db::Error> for Error {
    fn from(e: palette_db::Error) -> Self {
        Self::Db(e)
    }
}

impl From<palette_tmux::Error> for Error {
    fn from(e: palette_tmux::Error) -> Self {
        Self::Tmux(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Toml(e) => Some(e),
            Self::Db(e) => Some(e),
            Self::Tmux(e) => Some(e),
            Self::Docker(_) | Self::Internal(_) => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Toml(e) => write!(f, "TOML error: {e}"),
            Self::Db(e) => write!(f, "DB error: {e}"),
            Self::Tmux(e) => write!(f, "tmux error: {e}"),
            Self::Docker(msg) => write!(f, "Docker error: {msg}"),
            Self::Internal(msg) => f.write_str(msg),
        }
    }
}
