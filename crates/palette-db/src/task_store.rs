use crate::Error;
use crate::database::Database;
use palette_domain::task::*;

impl TaskStore for Database {
    type Error = Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Error> {
        self.get_task(id)
    }

    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Error> {
        self.get_child_tasks(parent_id)
    }

    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Error> {
        self.get_task_dependencies(task_id)
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Error> {
        self.update_task_status(id, status)
    }
}
