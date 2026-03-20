use super::Database;
use palette_domain::task::{Task, TaskRow};

impl Database {
    /// Recursively build a Task tree from a TaskRow.
    pub(crate) fn build_task_tree(&self, row: TaskRow) -> crate::Result<Task> {
        let child_rows = self.get_child_task_rows(&row.id)?;
        let children: Vec<Task> = child_rows
            .into_iter()
            .map(|child_row| self.build_task_tree(child_row))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Task {
            id: row.id,
            workflow_id: row.workflow_id,
            parent_id: row.parent_id,
            title: row.title,
            plan_path: row.plan_path,
            status: row.status,
            children,
        })
    }
}
