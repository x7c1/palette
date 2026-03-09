use palette_domain::{ReviewError, TaskError};
use std::fmt;

/// Database-layer error combining storage and domain errors.
#[derive(Debug)]
pub enum DbError {
    /// SQLite or other storage error.
    Storage(rusqlite::Error),
    /// Domain task error.
    Task(TaskError),
    /// Domain review error.
    Review(ReviewError),
    /// Lock acquisition failed (Mutex poisoned).
    LockPoisoned,
    /// Generic internal error.
    Internal(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Storage(e) => write!(f, "database error: {e}"),
            DbError::Task(e) => write!(f, "{e}"),
            DbError::Review(e) => write!(f, "{e}"),
            DbError::LockPoisoned => write!(f, "database lock poisoned"),
            DbError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DbError::Storage(e) => Some(e),
            DbError::Task(e) => Some(e),
            DbError::Review(e) => Some(e),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Storage(e)
    }
}

impl From<TaskError> for DbError {
    fn from(e: TaskError) -> Self {
        DbError::Task(e)
    }
}

impl From<ReviewError> for DbError {
    fn from(e: ReviewError) -> Self {
        DbError::Review(e)
    }
}
