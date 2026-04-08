use palette_domain::job::JobError;
use palette_domain::review::ReviewError;
use std::fmt;
use std::path::PathBuf;

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
    /// Filesystem I/O error (e.g., creating database directory).
    Io(std::io::Error),
    /// Lock acquisition failed (Mutex poisoned).
    LockPoisoned,
    /// Another Palette instance is already running on the same data directory.
    InstanceAlreadyRunning { db_path: PathBuf },
    /// Stored data violates domain constraints (e.g., invalid ID format,
    /// unknown enum value). Indicates data corruption or schema mismatch.
    DataCorruption {
        /// Machine-readable reason key identifying the violation.
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Storage(e) => write!(f, "database error: {e}"),
            Error::Job(e) => write!(f, "{e}"),
            Error::Review(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::LockPoisoned => write!(f, "database lock poisoned"),
            Error::InstanceAlreadyRunning { db_path } => write!(
                f,
                "another Palette instance is already running on {}",
                db_path.display()
            ),
            Error::DataCorruption { reason } => write!(f, "data corruption: {reason}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Storage(e) => Some(e),
            Error::Job(e) => Some(e),
            Error::Review(e) => Some(e),
            Error::Io(e) => Some(e),
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
