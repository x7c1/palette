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

mod create_task;
pub use create_task::CreateTaskRequest;

mod create_workflow;
mod get_task;
mod update_task_status;

mod assign_job;
mod count_active_members;
mod create_job;
mod dequeue_message;
mod enqueue_message;
mod find_assignable_jobs;
mod get_job;
mod get_job_by_task;
mod get_review_comments;
mod get_review_submissions;
mod get_workflow;
mod has_pending_messages;
mod increment_member_counter;
mod list_jobs;
mod submit_review;
mod update_job_status;
mod update_workflow_status;

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
        "SELECT id, task_id, type_id, title, plan_path, assignee, status_id, priority_id, repository, pr_url, created_at, updated_at, notes, assigned_at
         FROM jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], row_to_job)?;
    rows.next().transpose().map_err(Into::into)
}

pub(super) fn id_conversion_error(e: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Integer,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
    )
}

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    use crate::lookup;
    use palette_domain::task::TaskId;

    let repos_str: Option<String> = row.get("repository")?;
    let repository: Option<Repository> =
        repos_str.and_then(|s| repository_row::repository_from_json(&s));

    let type_id: i64 = row.get("type_id")?;
    let job_type = lookup::job_type_from_id(type_id).map_err(id_conversion_error)?;

    let status_id_val: i64 = row.get("status_id")?;
    let status =
        lookup::job_status_from_id(status_id_val, job_type).map_err(id_conversion_error)?;

    Ok(Job {
        id: JobId::new(row.get::<_, String>("id")?),
        task_id: TaskId::new(row.get::<_, String>("task_id")?),
        job_type,
        title: row.get("title")?,
        plan_path: row.get("plan_path")?,
        assignee: row.get::<_, Option<String>>("assignee")?.map(AgentId::new),
        status,
        priority: row
            .get::<_, Option<i64>>("priority_id")?
            .map(|id| lookup::priority_from_id(id).map_err(id_conversion_error))
            .transpose()?,
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
    use palette_domain::task::TaskId;
    use palette_domain::workflow::WorkflowId;

    pub fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    pub fn jid(s: &str) -> JobId {
        JobId::new(s)
    }

    pub fn tid(s: &str) -> TaskId {
        TaskId::new(s)
    }

    pub fn aid(s: &str) -> AgentId {
        AgentId::new(s)
    }

    /// Create a workflow and a task for testing. Returns the TaskId.
    pub fn setup_task(db: &Database, task_id: &str) -> TaskId {
        let wf_id = WorkflowId::new(format!("wf-{task_id}"));
        let t_id = tid(task_id);
        // Ignore errors if workflow already exists
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        let _ = db.create_task(&CreateTaskRequest {
            id: t_id.clone(),
            workflow_id: wf_id,
        });
        t_id
    }

    pub fn create_craft(db: &Database, id: &str, priority: Option<Priority>) {
        let task_id = setup_task(db, &format!("task-{id}"));
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid(id)),
            job_type: JobType::Craft,
            title: format!("Job {id}"),
            plan_path: format!("test/{id}"),
            assignee: None,
            priority,
            repository: None,
        })
        .unwrap();
    }

    pub fn create_review(db: &Database, id: &str) {
        let task_id = setup_task(db, &format!("task-{id}"));
        db.create_job(&CreateJobRequest {
            task_id,
            id: Some(jid(id)),
            job_type: JobType::Review,
            title: format!("Review {id}"),
            plan_path: format!("test/{id}"),
            assignee: None,
            priority: None,
            repository: None,
        })
        .unwrap();
    }
}
