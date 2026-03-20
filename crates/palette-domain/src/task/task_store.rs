use super::{Task, TaskId, TaskRow, TaskStatus};

/// Abstraction over task persistence, enabling domain logic
/// to remain independent of storage implementation.
pub trait TaskStore {
    type Error: std::fmt::Debug;

    /// Get a task row by ID (flat, without children).
    fn get_task_row(&self, id: &TaskId) -> Result<Option<TaskRow>, Self::Error>;

    /// Get all child task rows for a parent (flat, without grandchildren).
    fn get_child_task_rows(&self, parent_id: &TaskId) -> Result<Vec<TaskRow>, Self::Error>;

    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Self::Error>;
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error>;

    /// Get a task with its full subtree of children populated.
    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error> {
        let row = match self.get_task_row(id)? {
            Some(r) => r,
            None => return Ok(None),
        };
        let task = self.build_tree(row)?;
        Ok(Some(task))
    }

    /// Get child tasks with their subtrees populated.
    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        let rows = self.get_child_task_rows(parent_id)?;
        rows.into_iter().map(|row| self.build_tree(row)).collect()
    }

    /// Recursively build a Task tree from a TaskRow.
    fn build_tree(&self, row: TaskRow) -> Result<Task, Self::Error> {
        let child_rows = self.get_child_task_rows(&row.id)?;
        let children: Vec<Task> = child_rows
            .into_iter()
            .map(|child_row| self.build_tree(child_row))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Task {
            id: row.id,
            workflow_id: row.workflow_id,
            title: row.title,
            plan_path: row.plan_path,
            status: row.status,
            children,
        })
    }
}

impl<T: TaskStore> TaskStore for &T {
    type Error = T::Error;

    fn get_task_row(&self, id: &TaskId) -> Result<Option<TaskRow>, Self::Error> {
        (**self).get_task_row(id)
    }
    fn get_child_task_rows(&self, parent_id: &TaskId) -> Result<Vec<TaskRow>, Self::Error> {
        (**self).get_child_task_rows(parent_id)
    }
    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Self::Error> {
        (**self).get_task_dependencies(task_id)
    }
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error> {
        (**self).update_task_status(id, status)
    }
}

impl<T: TaskStore> TaskStore for std::sync::Arc<T> {
    type Error = T::Error;

    fn get_task_row(&self, id: &TaskId) -> Result<Option<TaskRow>, Self::Error> {
        (**self).get_task_row(id)
    }
    fn get_child_task_rows(&self, parent_id: &TaskId) -> Result<Vec<TaskRow>, Self::Error> {
        (**self).get_child_task_rows(parent_id)
    }
    fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, Self::Error> {
        (**self).get_task_dependencies(task_id)
    }
    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error> {
        (**self).update_task_status(id, status)
    }
}
