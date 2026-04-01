use palette_domain::worker::WorkerRole;

pub fn worker_role_id(role: WorkerRole) -> i64 {
    match role {
        WorkerRole::Approver => 1,
        WorkerRole::ReviewIntegrator => 2,
        WorkerRole::Member => 3,
    }
}

pub fn worker_role_from_id(id: i64) -> Result<WorkerRole, String> {
    match id {
        1 => Ok(WorkerRole::Approver),
        2 => Ok(WorkerRole::ReviewIntegrator),
        3 => Ok(WorkerRole::Member),
        _ => Err(format!("unknown worker_role id: {id}")),
    }
}
