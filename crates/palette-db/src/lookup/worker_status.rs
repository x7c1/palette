use palette_domain::worker::WorkerStatus;

pub fn worker_status_id(status: WorkerStatus) -> i64 {
    match status {
        WorkerStatus::Booting => 1,
        WorkerStatus::Working => 2,
        WorkerStatus::Idle => 3,
        WorkerStatus::WaitingPermission => 4,
        WorkerStatus::Crashed => 5,
        WorkerStatus::Suspended => 6,
    }
}

pub fn worker_status_from_id(id: i64) -> Result<WorkerStatus, String> {
    match id {
        1 => Ok(WorkerStatus::Booting),
        2 => Ok(WorkerStatus::Working),
        3 => Ok(WorkerStatus::Idle),
        4 => Ok(WorkerStatus::WaitingPermission),
        5 => Ok(WorkerStatus::Crashed),
        6 => Ok(WorkerStatus::Suspended),
        _ => Err(format!("unknown worker_status id: {id}")),
    }
}
