pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Db(palette_db::Error),
    Tmux(palette_tmux::Error),
    Docker(palette_docker::Error),
    Internal(String),
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

impl From<palette_docker::Error> for Error {
    fn from(e: palette_docker::Error) -> Self {
        Self::Docker(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Db(e) => Some(e),
            Self::Tmux(e) => Some(e),
            Self::Docker(e) => Some(e),
            Self::Internal(_) => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Db(e) => write!(f, "DB error: {e}"),
            Self::Tmux(e) => write!(f, "tmux error: {e}"),
            Self::Docker(e) => write!(f, "Docker error: {e}"),
            Self::Internal(msg) => f.write_str(msg),
        }
    }
}
