use crate::models::WorkflowRow;
use palette_domain::workflow::{Workflow, WorkflowId};

use super::super::{corrupt, corrupt_parse, parse_datetime};

/// Extract a raw DB row into a WorkflowRow (DB-native types only).
pub(super) fn read_workflow_row(row: &rusqlite::Row) -> rusqlite::Result<WorkflowRow> {
    Ok(WorkflowRow {
        id: row.get("id")?,
        blueprint_path: row.get("blueprint_path")?,
        status_id: row.get("status_id")?,
        started_at: row.get("started_at")?,
        blueprint_hash: row.get("blueprint_hash")?,
    })
}

/// Convert a WorkflowRow to a domain Workflow.
pub(super) fn into_workflow(row: WorkflowRow) -> crate::Result<Workflow> {
    let status = crate::lookup::workflow_status_from_id(row.status_id).map_err(corrupt)?;
    let id = WorkflowId::parse(row.id).map_err(corrupt_parse)?;

    Ok(Workflow {
        id,
        blueprint_path: row.blueprint_path,
        status,
        started_at: parse_datetime(&row.started_at),
        blueprint_hash: row.blueprint_hash,
    })
}
