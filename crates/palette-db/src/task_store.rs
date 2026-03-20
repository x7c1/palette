use crate::Error;
use crate::database::Database;
use palette_domain::task::*;

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
