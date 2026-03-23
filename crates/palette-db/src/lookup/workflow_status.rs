use palette_domain::workflow::WorkflowStatus;

pub fn workflow_status_id(status: WorkflowStatus) -> i64 {
    match status {
        WorkflowStatus::Active => 1,
        WorkflowStatus::Suspended => 2,
        WorkflowStatus::Completed => 3,
    }
}

pub fn workflow_status_from_id(id: i64) -> Result<WorkflowStatus, String> {
    match id {
        1 => Ok(WorkflowStatus::Active),
        2 => Ok(WorkflowStatus::Suspended),
        3 => Ok(WorkflowStatus::Completed),
        _ => Err(format!("invalid workflow_status id: {id}")),
    }
}
