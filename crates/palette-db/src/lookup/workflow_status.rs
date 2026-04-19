use palette_domain::workflow::WorkflowStatus;

pub fn workflow_status_id(status: WorkflowStatus) -> i64 {
    match status {
        WorkflowStatus::Active => 1,
        WorkflowStatus::Suspended => 2,
        WorkflowStatus::Completed => 3,
        WorkflowStatus::Suspending => 4,
        WorkflowStatus::Terminated => 5,
        WorkflowStatus::Failed => 6,
    }
}

pub fn workflow_status_from_id(id: i64) -> Result<WorkflowStatus, String> {
    match id {
        1 => Ok(WorkflowStatus::Active),
        2 => Ok(WorkflowStatus::Suspended),
        3 => Ok(WorkflowStatus::Completed),
        4 => Ok(WorkflowStatus::Suspending),
        5 => Ok(WorkflowStatus::Terminated),
        6 => Ok(WorkflowStatus::Failed),
        _ => Err(format!("invalid workflow_status id: {id}")),
    }
}
