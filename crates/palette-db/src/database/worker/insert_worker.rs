use super::super::{Database, lock};
use crate::lookup;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::*;
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

/// Request to insert a new worker.
pub struct InsertWorkerRequest {
    pub id: WorkerId,
    pub workflow_id: WorkflowId,
    pub role: WorkerRole,
    pub status: WorkerStatus,
    pub supervisor_id: Option<WorkerId>,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub session_id: Option<WorkerSessionId>,
    pub task_id: TaskId,
}

impl Database {
    /// Insert a new worker record.
    pub fn insert_worker(&self, req: &InsertWorkerRequest) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO workers (id, workflow_id, role_id, status_id, supervisor_id, container_id, terminal_target, session_id, task_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                req.id.as_ref(),
                req.workflow_id.as_ref(),
                lookup::worker_role_id(req.role),
                lookup::worker_status_id(req.status),
                req.supervisor_id.as_ref().map(|s| s.as_ref()),
                req.container_id.as_ref(),
                req.terminal_target.as_ref(),
                req.session_id.as_ref().map(|s| s.as_ref()),
                req.task_id.as_ref(),
                now,
                now,
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::database::test_helpers::test_db;

    pub fn insert_test_worker(db: &Database, id: &str, role: WorkerRole, workflow_id: &str) {
        let wf_id = WorkflowId::new(workflow_id);
        let _ = db.create_workflow(&wf_id, "test/blueprint.yaml");
        db.insert_worker(&InsertWorkerRequest {
            id: WorkerId::new(id),
            workflow_id: wf_id,
            role,
            status: WorkerStatus::Booting,
            supervisor_id: None,
            container_id: ContainerId::new(format!("container-{id}")),
            terminal_target: TerminalTarget::new(format!("pane-{id}")),
            session_id: None,
            task_id: TaskId::new(format!("task-{id}")),
        })
        .unwrap();
    }

    #[test]
    fn insert_and_find_worker() {
        let db = test_db();
        insert_test_worker(&db, "leader-1", WorkerRole::Leader, "wf-1");

        let worker = db.find_worker(&WorkerId::new("leader-1")).unwrap().unwrap();
        assert_eq!(worker.id, WorkerId::new("leader-1"));
        assert_eq!(worker.role, WorkerRole::Leader);
        assert_eq!(worker.status, WorkerStatus::Booting);
    }
}
