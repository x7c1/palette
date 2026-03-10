use crate::error::Error;
mod repository_row;
use crate::schema;
use chrono::{DateTime, Utc};
use palette_domain::agent::*;
use palette_domain::review::*;
use palette_domain::task::*;
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

mod assign_task;
mod count_active_members;
mod create_task;
mod dequeue_message;
mod enqueue_message;
mod find_assignable_tasks;
mod find_reviews_for_work;
mod find_works_for_review;
mod get_dependencies;
mod get_dependents;
mod get_review_comments;
mod get_review_submissions;
mod get_task;
mod has_pending_messages;
mod list_tasks;
mod submit_review;
mod update_task_status;

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

/// Query a single task by ID from a connection or transaction.
fn query_task(conn: &Connection, id: &TaskId) -> crate::Result<Option<Task>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, description, assignee, status, priority, repositories, pr_url, created_at, updated_at, notes, assigned_at
         FROM tasks WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], |row| Ok(row_to_task(row)))?;
    match rows.next() {
        Some(Ok(task)) => Ok(Some(task)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

fn row_to_task(row: &rusqlite::Row) -> Task {
    let repos_str: Option<String> = row.get(7).unwrap();
    let repositories: Option<Vec<Repository>> =
        repos_str.and_then(|s| repository_row::repositories_from_json(&s));

    Task {
        id: TaskId::new(row.get::<_, String>(0).unwrap()),
        task_type: row.get::<_, String>(1).unwrap().parse().unwrap(),
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

    pub fn tid(s: &str) -> TaskId {
        TaskId::new(s)
    }

    pub fn aid(s: &str) -> AgentId {
        AgentId::new(s)
    }

    pub fn create_work(db: &Database, id: &str, priority: Option<Priority>, deps: Vec<TaskId>) {
        db.create_task(&CreateTaskRequest {
            id: Some(tid(id)),
            task_type: TaskType::Work,
            title: format!("Task {id}"),
            description: None,
            assignee: None,
            priority,
            repositories: None,
            depends_on: deps,
        })
        .unwrap();
    }
}
