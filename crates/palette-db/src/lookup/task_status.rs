use palette_domain::task::TaskStatus;

pub fn task_status_id(status: TaskStatus) -> i64 {
    match status {
        TaskStatus::Pending => 1,
        TaskStatus::Ready => 2,
        TaskStatus::InProgress => 3,
        TaskStatus::Suspended => 4,
        TaskStatus::Completed => 5,
        TaskStatus::Terminated => 6,
    }
}

pub fn task_status_from_id(id: i64) -> Result<TaskStatus, String> {
    match id {
        1 => Ok(TaskStatus::Pending),
        2 => Ok(TaskStatus::Ready),
        3 => Ok(TaskStatus::InProgress),
        4 => Ok(TaskStatus::Suspended),
        5 => Ok(TaskStatus::Completed),
        6 => Ok(TaskStatus::Terminated),
        _ => Err(format!("invalid task_status id: {id}")),
    }
}
