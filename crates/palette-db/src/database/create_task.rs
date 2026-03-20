use super::Database;
use crate::error::Error;
use palette_domain::task::{Task, TaskId, TaskStatus};
use palette_domain::workflow::WorkflowId;
use rusqlite::params;

pub struct CreateTaskRequest {
    pub id: TaskId,
    pub workflow_id: WorkflowId,
    pub parent_id: Option<TaskId>,
    pub title: String,
    pub plan_path: Option<String>,
    pub depends_on: Vec<TaskId>,
}

impl Database {
    pub fn create_task(&self, req: &CreateTaskRequest) -> crate::Result<Task> {
        let conn = lock!(self.conn);
        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO tasks (id, workflow_id, parent_id, title, plan_path, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                req.id.as_ref(),
                req.workflow_id.as_ref(),
                req.parent_id.as_ref().map(|id| id.as_ref()),
                req.title,
                req.plan_path,
                TaskStatus::Pending.as_str(),
            ],
        )?;

        for dep in &req.depends_on {
            tx.execute(
                "INSERT INTO task_dependencies (task_id, depends_on) VALUES (?1, ?2)",
                params![req.id.as_ref(), dep.as_ref()],
            )?;
        }

        tx.commit()?;

        Ok(Task {
            id: req.id.clone(),
            workflow_id: req.workflow_id.clone(),
            parent_id: req.parent_id.clone(),
            title: req.title.clone(),
            plan_path: req.plan_path.clone(),
            status: TaskStatus::Pending,
        })
    }
}
