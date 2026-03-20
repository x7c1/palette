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

mod build_task_tree;
mod create_task;
#[cfg(test)]
mod task_rule_engine_test;
pub use create_task::CreateTaskRequest;

mod create_workflow;
mod get_task;
mod get_task_dependencies;
mod update_task_status;

mod assign_job;
mod count_active_members;
mod create_job;
mod dequeue_message;
mod enqueue_message;
mod find_assignable_jobs;
mod find_crafts_for_review;
mod find_reviews_for_craft;
mod get_blueprint;
mod get_dependencies;
mod get_dependents;
mod get_job;
mod get_review_comments;
mod get_review_submissions;
mod has_pending_messages;
mod list_blueprints;
mod list_jobs;
mod save_blueprint;
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
        "SELECT id, task_id, type, title, plan_path, description, assignee, status, priority, repository, pr_url, created_at, updated_at, notes, assigned_at
         FROM jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], row_to_job)?;
    rows.next().transpose().map_err(Into::into)
}

pub(super) fn parse_column<T: std::str::FromStr<Err = String>>(
    row: &rusqlite::Row,
    column: &str,
) -> rusqlite::Result<T> {
    let s: String = row.get(column)?;
    s.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })
}

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    use palette_domain::task::TaskId;

    let repos_str: Option<String> = row.get("repository")?;
    let repository: Option<Repository> =
        repos_str.and_then(|s| repository_row::repository_from_json(&s));

    Ok(Job {
        id: JobId::new(row.get::<_, String>("id")?),
        task_id: row.get::<_, Option<String>>("task_id")?.map(TaskId::new),
        job_type: parse_column(row, "type")?,
        title: row.get("title")?,
        plan_path: row.get("plan_path")?,
        description: row.get("description")?,
        assignee: row.get::<_, Option<String>>("assignee")?.map(AgentId::new),
        status: parse_column(row, "status")?,
        priority: row
            .get::<_, Option<String>>("priority")?
            .and_then(|s| s.parse().ok()),
        repository,
        pr_url: row.get("pr_url")?,
        created_at: parse_datetime(&row.get::<_, String>("created_at")?),
        updated_at: parse_datetime(&row.get::<_, String>("updated_at")?),
        notes: row.get("notes")?,
        assigned_at: row
            .get::<_, Option<String>>("assigned_at")?
            .map(|s| parse_datetime(&s)),
    })
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
            task_id: None,
            id: Some(jid(id)),
            job_type: JobType::Craft,
            title: format!("Job {id}"),
            plan_path: format!("test/{id}"),
            description: None,
            assignee: None,
            priority,
            repository: None,
            depends_on: deps,
        })
        .unwrap();
    }

    pub fn create_review(db: &Database, id: &str, deps: Vec<JobId>) {
        db.create_job(&CreateJobRequest {
            task_id: None,
            id: Some(jid(id)),
            job_type: JobType::Review,
            title: format!("Review {id}"),
            plan_path: format!("test/{id}"),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: deps,
        })
        .unwrap();
    }
}
