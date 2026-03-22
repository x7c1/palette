use super::TaskEffect;
use crate::task::{Task, TaskId, TaskStatus, TaskStore};

pub struct TaskRuleEngine<S> {
    store: S,
}

impl<S: TaskStore> TaskRuleEngine<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Determine which tasks in a workflow should transition to Ready.
    /// A task becomes Ready when all of its dependencies are Done
    /// and its parent is active (Ready or InProgress).
    pub fn resolve_ready_tasks(&self, task_ids: &[TaskId]) -> Result<Vec<TaskEffect>, S::Error> {
        task_ids
            .iter()
            .filter_map(|id| self.check_ready(id).transpose())
            .collect()
    }

    /// When a task completes, check if sibling tasks can now become Ready
    /// and whether the parent task can also complete.
    pub fn on_task_completed(&self, task_id: &TaskId) -> Result<Vec<TaskEffect>, S::Error> {
        let Some(task) = self.store.get_task(task_id)? else {
            return Ok(vec![]);
        };
        let Some(ref parent_id) = task.parent_id else {
            return Ok(vec![]);
        };

        let siblings = self.store.get_child_tasks(parent_id)?;

        let mut effects: Vec<TaskEffect> = siblings
            .iter()
            .filter(|s| s.status == TaskStatus::Pending)
            .filter_map(|s| self.check_ready(&s.id).transpose())
            .collect::<Result<Vec<_>, _>>()?;

        if self.all_children_done(&siblings, task_id)
            && let Some(parent) = self.store.get_task(parent_id)?
            && parent.status != TaskStatus::Completed
        {
            effects.push(TaskEffect::TaskStatusChanged {
                task_id: parent_id.clone(),
                new_status: TaskStatus::Completed,
            });
        }

        Ok(effects)
    }

    /// Check if a single task should transition to Ready.
    /// Returns Some(effect) if the task should become Ready, None otherwise.
    fn check_ready(&self, task_id: &TaskId) -> Result<Option<TaskEffect>, S::Error> {
        let Some(task) = self.store.get_task(task_id)? else {
            return Ok(None);
        };
        if task.status != TaskStatus::Pending {
            return Ok(None);
        }
        if !self.parent_is_active(&task)? {
            return Ok(None);
        }
        if !self.all_deps_done(&task)? {
            return Ok(None);
        }
        Ok(Some(TaskEffect::TaskStatusChanged {
            task_id: task_id.clone(),
            new_status: TaskStatus::Ready,
        }))
    }

    fn parent_is_active(&self, task: &Task) -> Result<bool, S::Error> {
        let Some(ref parent_id) = task.parent_id else {
            return Ok(true);
        };
        Ok(self
            .store
            .get_task(parent_id)?
            .is_some_and(|p| p.status == TaskStatus::Ready || p.status == TaskStatus::InProgress))
    }

    fn all_deps_done(&self, task: &Task) -> Result<bool, S::Error> {
        for dep_id in &task.depends_on {
            let done = self
                .store
                .get_task(dep_id)?
                .is_some_and(|d| d.status == TaskStatus::Completed);
            if !done {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn all_children_done(&self, siblings: &[Task], just_completed: &TaskId) -> bool {
        siblings
            .iter()
            .all(|s| s.status == TaskStatus::Completed || s.id == *just_completed)
    }
}
