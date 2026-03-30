use crate::database::{id_conversion_error, parse_datetime};
use crate::lookup;
use palette_domain::job::*;
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use rusqlite::{Connection, params};

/// Map a database row to a Job domain object.
pub(crate) fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    let repos_str: Option<String> = row.get("repository")?;
    let repository: Option<Repository> =
        repos_str.and_then(|s| super::repository_row::repository_from_json(&s));

    let type_id: i64 = row.get("type_id")?;
    let job_type = lookup::job_type_from_id(type_id).map_err(id_conversion_error)?;

    let status_id_val: i64 = row.get("status_id")?;
    let status =
        lookup::job_status_from_id(status_id_val, job_type).map_err(id_conversion_error)?;

    Ok(Job {
        id: JobId::new(row.get::<_, String>("id")?),
        task_id: TaskId::parse(row.get::<_, String>("task_id")?)
            .map_err(|e| id_conversion_error(e.reason_key()))?,
        job_type,
        title: row.get("title")?,
        plan_path: row.get("plan_path")?,
        assignee_id: row
            .get::<_, Option<String>>("assignee_id")?
            .map(WorkerId::new),
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

/// Query a single job by ID.
pub(crate) fn query_job(conn: &Connection, id: &JobId) -> crate::Result<Option<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, type_id, title, plan_path, assignee_id, status_id, priority_id, repository, pr_url, created_at, updated_at, notes, assigned_at
         FROM jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.as_ref()], row_to_job)?;
    rows.next().transpose().map_err(Into::into)
}
