use palette_domain::job::JobError;
use palette_domain::review::ReviewError;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

/// Database-layer error combining storage and domain errors.
#[derive(Debug)]
pub enum Error {
    /// SQLite or other storage error.
    Storage(rusqlite::Error),
    /// Domain job error.
    Job(JobError),
    /// Domain review error.
    Review(ReviewError),
    /// Lock acquisition failed (Mutex poisoned).
    LockPoisoned,
    /// Generic internal error.
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Storage(e) => write!(f, "database error: {e}"),
            Error::Job(e) => write!(f, "{e}"),
            Error::Review(e) => write!(f, "{e}"),
            Error::LockPoisoned => write!(f, "database lock poisoned"),
            Error::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Storage(e) => Some(e),
            Error::Job(e) => Some(e),
            Error::Review(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Storage(e)
    }
}

impl From<JobError> for Error {
    fn from(e: JobError) -> Self {
        Error::Job(e)
    }
}

impl From<ReviewError> for Error {
    fn from(e: ReviewError) -> Self {
        Error::Review(e)
    }
}
