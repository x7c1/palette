use crate::error::Error;
use crate::schema;
use chrono::{DateTime, Utc};
use palette_domain::job::*;
use palette_domain::review::*;
use palette_domain::worker::*;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

/// Acquire the Mutex lock, converting a poisoned lock into Error.
pub(crate) fn lock(
    mutex: &Mutex<Connection>,
) -> crate::Result<std::sync::MutexGuard<'_, Connection>> {
    mutex.lock().map_err(|_| crate::Error::LockPoisoned)
}

mod worker;
pub use worker::InsertWorkerRequest;

mod job;

mod message_queue;

mod task;
pub use task::CreateTaskRequest;

mod workflow;

impl Database {
    pub fn open(path: &Path) -> crate::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Internal(Box::new(e)))?;
        }
        let conn = Connection::open(path).map_err(|e| Error::Internal(Box::new(e)))?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> crate::Result<Self> {
        let conn = Connection::open_in_memory()?;
        schema::initialize(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

/// Parse an RFC3339 datetime string from the database.
pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

pub(crate) fn id_conversion_error(e: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Integer,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
    )
}

#[cfg(test)]
pub(crate) mod test_helpers;
