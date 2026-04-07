use crate::models::JobRow;
use palette_domain::job::*;
use palette_domain::task::TaskId;
use palette_domain::worker::WorkerId;
use rusqlite::{Connection, params};

use super::super::{corrupt, corrupt_parse, parse_datetime};

pub(crate) const JOB_COLUMNS: &str = "id, task_id, type_id, title, plan_path, assignee_id, status_id, priority_id, repository, command, perspective, pull_request_owner, pull_request_repo, pull_request_number, created_at, updated_at, notes, assigned_at";

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
        command: row.get("command")?,
        perspective: row.get("perspective")?,
        pull_request_owner: row.get("pull_request_owner")?,
        pull_request_repo: row.get("pull_request_repo")?,
        pull_request_number: row.get("pull_request_number")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        notes: row.get("notes")?,
        assigned_at: row.get("assigned_at")?,
    })
}

/// Convert a JobRow to a domain Job.
pub(crate) fn into_job(row: JobRow) -> crate::Result<Job> {
    let job_type = crate::lookup::job_type_from_id(row.type_id).map_err(corrupt)?;

    let status = crate::lookup::job_status_from_id(row.status_id, job_type).map_err(corrupt)?;

    let priority = row
        .priority_id
        .map(|id| crate::lookup::priority_from_id(id).map_err(corrupt))
        .transpose()?;

    let repository = row
        .repository
        .map(|s| super::repository_row::repository_from_json(&s))
        .transpose()?;

    let task_id = TaskId::parse(row.task_id).map_err(corrupt_parse)?;
    let title = Title::parse(row.title).map_err(corrupt_parse)?;
    let plan_path = row
        .plan_path
        .map(PlanPath::parse)
        .transpose()
        .map_err(corrupt_parse)?;

    let review_target = match (
        row.pull_request_owner,
        row.pull_request_repo,
        row.pull_request_number,
    ) {
        (Some(owner), Some(repo), Some(number)) => {
            let pr = PullRequest::parse(owner, repo, number as u64).map_err(corrupt_parse)?;
            ReviewTarget::PullRequest(pr)
        }
        _ => ReviewTarget::CraftOutput,
    };

    let detail = match job_type {
        JobType::Craft => {
            let repository = repository
                .ok_or_else(|| corrupt(format!("craft job missing repository: {}", row.id)))?;
            JobDetail::Craft { repository }
        }
        JobType::Review => JobDetail::Review {
            perspective: row
                .perspective
                .map(PerspectiveName::parse)
                .transpose()
                .map_err(corrupt_parse)?,
            target: review_target,
        },
        JobType::ReviewIntegrate => JobDetail::ReviewIntegrate {
            target: review_target,
        },
        JobType::Orchestrator => JobDetail::Orchestrator {
            command: row.command,
        },
        JobType::Operator => JobDetail::Operator,
    };

    Ok(Job {
        id: JobId::parse(row.id).map_err(corrupt_parse)?,
        task_id,
        title,
        plan_path,
        assignee_id: row
            .assignee_id
            .map(WorkerId::parse)
            .transpose()
            .map_err(corrupt_parse)?,
        status,
        priority,
        detail,
        created_at: parse_datetime(&row.created_at),
        updated_at: parse_datetime(&row.updated_at),
        notes: row.notes,
        assigned_at: row.assigned_at.map(|s| parse_datetime(&s)),
    })
}

/// Query a single job by ID.
pub(crate) fn query_job(conn: &Connection, id: &JobId) -> crate::Result<Option<Job>> {
    let mut stmt = conn.prepare(&format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id = ?1"))?;
    stmt.query_map(params![id.as_ref()], read_job_row)?
        .next()
        .transpose()?
        .map(into_job)
        .transpose()
}
