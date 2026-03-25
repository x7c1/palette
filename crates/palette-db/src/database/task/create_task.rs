use super::super::{Database, lock};
use palette_domain::task::{TaskId, TaskStatus};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

pub struct CreateTaskRequest {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
}

impl Database {
    pub fn create_task(&self, req: &CreateTaskRequest) -> crate::Result<()> {
        let conn = lock(&self.conn)?;
        conn.execute(
            "INSERT INTO tasks (id, workflow_id, status_id) VALUES (?1, ?2, ?3)",
            params![
                req.id.as_ref(),
                req.workflow_id.as_ref(),
                crate::lookup::task_status_id(TaskStatus::Pending),
            ],
        )?;
        Ok(())
    }
}
