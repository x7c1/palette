use crate::models::JobRow;
use palette_domain::job::*;
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use rusqlite::{Connection, params};

use super::super::parse_datetime;

/// Extract a raw DB row into a JobRow (DB-native types only).
pub(crate) fn read_job_row(row: &rusqlite::Row) -> rusqlite::Result<JobRow> {
    Ok(JobRow {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        type_id: row.get("type_id")?,
        title: row.get("title")?,
        plan_path: row.get("plan_path")?,
        assignee_id: row.get("assignee_id")?,
        status_id: row.get("status_id")?,
        priority_id: row.get("priority_id")?,
        repository: row.get("repository")?,
        pr_url: row.get("pr_url")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        notes: row.get("notes")?,
        assigned_at: row.get("assigned_at")?,
    })
}

/// Convert a JobRow to a domain Job.
pub(crate) fn into_job(row: JobRow) -> crate::Result<Job> {
    let job_type = crate::lookup::job_type_from_id(row.type_id)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;

    let status = crate::lookup::job_status_from_id(row.status_id, job_type)
        .map_err(|e| crate::Error::Internal(Box::new(e)))?;

    let priority = row
        .priority_id
        .map(|id| {
            crate::lookup::priority_from_id(id).map_err(|e| crate::Error::Internal(Box::new(e)))
        })
        .transpose()?;

    let repository = row
        .repository
        .and_then(|s| super::repository_row::repository_from_json(&s));

    let task_id =
        TaskId::parse(row.task_id).map_err(|e| crate::Error::Internal(Box::new(e.reason_key())))?;

    Ok(Job {
        id: JobId::new(row.id),
        task_id,
        job_type,
        title: row.title,
        plan_path: row.plan_path,
        assignee_id: row.assignee_id.map(WorkerId::new),
        status,
        priority,
        repository,
        pr_url: row.pr_url,
        created_at: parse_datetime(&row.created_at),
        updated_at: parse_datetime(&row.updated_at),
        notes: row.notes,
        assigned_at: row.assigned_at.map(|s| parse_datetime(&s)),
    })
}

/// Query a single job by ID.
pub(crate) fn query_job(conn: &Connection, id: &JobId) -> crate::Result<Option<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, type_id, title, plan_path, assignee_id, status_id, priority_id, repository, pr_url, created_at, updated_at, notes, assigned_at
         FROM jobs WHERE id = ?1",
    )?;
    stmt.query_map(params![id.as_ref()], read_job_row)?
        .next()
        .transpose()?
        .map(into_job)
        .transpose()
}
