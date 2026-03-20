use crate::Error;
use crate::database::Database;
use palette_domain::task::*;

impl TaskStore for Database {
    type Error = Error;

    fn get_task_row(&self, id: &TaskId) -> Result<Option<TaskRow>, Error> {
        self.get_task_row(id)
    }

    fn get_child_task_rows(&self, parent_id: &TaskId) -> Result<Vec<TaskRow>, Error> {
        self.get_child_task_rows(parent_id)
    }

    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Error> {
        self.get_task_dependencies(task_id)
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Error> {
        self.update_task_status(id, status)
    }
}
