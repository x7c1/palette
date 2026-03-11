use crate::error::Error;
mod repository_row;
use crate::schema;
use chrono::{DateTime, Utc};
use palette_domain::agent::*;
use palette_domain::job::*;
use palette_domain::review::*;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

/// Acquire the Mutex lock, converting a poisoned lock into Error.
macro_rules! lock {
    ($mutex:expr) => {
        $mutex.lock().map_err(|_| Error::LockPoisoned)?
    };
}

mod assign_job;
mod count_active_members;
mod create_job;
mod dequeue_message;
mod enqueue_message;
mod find_assignable_jobs;
mod find_crafts_for_review;
mod find_reviews_for_craft;
mod get_dependencies;
mod get_dependents;
mod get_job;
mod get_review_comments;
mod get_review_submissions;
mod has_pending_messages;
mod list_jobs;
mod submit_review;
mod update_job_status;

impl Database {
    pub fn open(path: &Path) -> crate::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Internal(format!(
                    "failed to create db directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let conn = Connection::open(path).map_err(|e| {
            Error::Internal(format!("failed to open database {}: {e}", path.display()))
        })?;
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
fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

/// Query a single job by ID from a connection or transaction.
fn query_job(conn: &Connection, id: &JobId) -> crate::Result<Option<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at
         FROM jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_job(row)))?;
    match rows.next() {
        Some(Ok(job)) => Ok(Some(job)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

fn row_to_job(row: &rusqlite::Row) -> Job {
    let repos_str: Option<String> = row.get(7).unwrap();
    let repositories: Option<Vec<Repository>> =
        repos_str.and_then(|s| repository_row::repositories_from_json(&s));

    Job {
        id: JobId::new(row.get::<_, String>(0).unwrap()),
        job_type: row.get::<_, String>(1).unwrap().parse().unwrap(),
        title: row.get(2).unwrap(),
        description: row.get(3).unwrap(),
        assignee: row.get::<_, Option<String>>(4).unwrap().map(AgentId::new),
        status: row.get::<_, String>(5).unwrap().parse().unwrap(),
        priority: row
            .get::<_, Option<String>>(6)
            .unwrap()
            .and_then(|s| s.parse().ok()),
        repositories,
        pr_url: row.get(8).unwrap(),
        created_at: parse_datetime(&row.get::<_, String>(9).unwrap()),
        updated_at: parse_datetime(&row.get::<_, String>(10).unwrap()),
        notes: row.get(11).unwrap(),
        assigned_at: row
            .get::<_, Option<String>>(12)
            .unwrap()
            .map(|s| parse_datetime(&s)),
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    pub fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    pub fn jid(s: &str) -> JobId {
        JobId::new(s)
    }

    pub fn aid(s: &str) -> AgentId {
        AgentId::new(s)
    }

    pub fn create_craft(db: &Database, id: &str, priority: Option<Priority>, deps: Vec<JobId>) {
        db.create_job(&CreateJobRequest {
            id: Some(jid(id)),
            job_type: JobType::Craft,
            title: format!("Job {id}"),
            description: None,
            assignee: None,
            priority,
            repositories: None,
            depends_on: deps,
        })
        .unwrap();
    }

    pub fn create_review(db: &Database, id: &str, deps: Vec<JobId>) {
        db.create_job(&CreateJobRequest {
            id: Some(jid(id)),
            job_type: JobType::Review,
            title: format!("Review {id}"),
            description: None,
            assignee: None,
            priority: None,
            repositories: None,
            depends_on: deps,
        })
        .unwrap();
    }
}
