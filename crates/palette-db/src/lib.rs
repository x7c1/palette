mod error;
pub use error::{Error, Result};

/// Acquire the Mutex lock, converting a poisoned lock into Error.
macro_rules! lock {
    ($mutex:expr) => {
        $mutex.lock().map_err(|_| $crate::Error::LockPoisoned)?
    };
}

pub(crate) mod models;

mod database;
pub use database::CreateTaskRequest;
pub use database::Database;
pub use database::InsertAgentRequest;

mod job_store;
pub(crate) mod lookup;
mod schema;
