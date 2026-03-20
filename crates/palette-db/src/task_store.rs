use crate::Error;
use crate::database::Database;
use palette_domain::task::*;

impl Database {
    /// Recursively build a Task tree from a TaskRow.
    fn build_task_tree(&self, row: TaskRow) -> Result<Task, Error> {
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

impl TaskStore for Database {
    type Error = Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Error> {
        let row = match self.get_task_row(id)? {
            Some(r) => r,
            None => return Ok(None),
        };
        Ok(Some(self.build_task_tree(row)?))
    }

    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Error> {
        let rows = self.get_child_task_rows(parent_id)?;
        rows.into_iter()
            .map(|row| self.build_task_tree(row))
            .collect()
    }

    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Error> {
        self.get_task_dependencies(task_id)
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Error> {
        self.update_task_status(id, status)
    }
}
