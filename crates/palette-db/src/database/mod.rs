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
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA locking_mode=EXCLUSIVE;")?;
        schema::initialize(&conn).map_err(|e| {
            if matches!(
                e,
                rusqlite::Error::SqliteFailure(ref f, _)
                    if f.code == rusqlite::ffi::ErrorCode::DatabaseBusy
            ) {
                Error::InstanceAlreadyRunning {
                    db_path: path.to_path_buf(),
                }
            } else {
                Error::Storage(e)
            }
        })?;
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

/// Convert a lookup error (unknown enum ID) into `DataCorruption`.
pub(crate) fn corrupt(reason: String) -> crate::Error {
    crate::Error::DataCorruption { reason }
}

/// Convert a parse error (`ReasonKey` impl) into `DataCorruption`.
pub(crate) fn corrupt_parse(e: impl palette_core::ReasonKey) -> crate::Error {
    crate::Error::DataCorruption {
        reason: e.reason_key(),
    }
}

/// Parse an RFC3339 datetime string from the database.
pub(crate) fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
pub(crate) mod test_helpers;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn open_sets_exclusive_locking_mode() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let _db = Database::open(&db_path).unwrap();

        // Verify EXCLUSIVE mode by checking that a second open fails
        let result = Database::open(&db_path);
        match result {
            Err(Error::InstanceAlreadyRunning { db_path: p }) => {
                assert_eq!(p, dir.path().join("test.db"));
            }
            Err(e) => panic!("expected InstanceAlreadyRunning, got: {e}"),
            Ok(_) => panic!("expected InstanceAlreadyRunning, but open succeeded"),
        }
    }

    #[test]
    fn separate_databases_can_coexist() {
        let dir1 = TempDir::new().unwrap();
        let dir2 = TempDir::new().unwrap();
        let _db1 = Database::open(&dir1.path().join("test.db")).unwrap();
        let _db2 = Database::open(&dir2.path().join("test.db")).unwrap();
    }
}
