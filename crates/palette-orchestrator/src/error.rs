pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    External(Box<dyn std::error::Error + Send + Sync>),
    Internal(String),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::External(e)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::External(e) => Some(e.as_ref()),
            Self::Internal(_) => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::External(e) => write!(f, "{e}"),
            Self::Internal(msg) => f.write_str(msg),
        }
    }
}
