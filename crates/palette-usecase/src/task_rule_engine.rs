use crate::task_store::TaskStore;
use palette_domain::task::{Task, TaskId, TaskStatus};

pub struct TaskRuleEngine<'a> {
    store: &'a TaskStore<'a>,
}

impl<'a> TaskRuleEngine<'a> {
    pub fn new(store: &'a TaskStore<'a>) -> Self {
        Self { store }
    }

    /// Determine which tasks should transition to Ready, and update them directly.
    /// Returns the IDs of tasks that became Ready.
    pub fn resolve_ready_tasks(&self, task_ids: &[TaskId]) -> Vec<TaskId> {
        task_ids
            .iter()
            .filter(|id| self.check_and_activate(id))
            .cloned()
            .collect()
    }

    /// When a task completes, check if sibling tasks can now become Ready
    /// and whether the parent task can also complete.
    /// Returns info about what changed.
    pub fn on_task_completed(&self, task_id: &TaskId) -> TaskCompletionResult {
        let Some(task) = self.store.get_task(task_id) else {
            return TaskCompletionResult::default();
        };
        let Some(ref parent_id) = task.parent_id else {
            return TaskCompletionResult::default();
        };

        let siblings = self.store.get_child_tasks(parent_id);

        let newly_ready: Vec<TaskId> = siblings
            .iter()
            .filter(|s| s.status == TaskStatus::Pending)
            .filter(|s| self.check_and_activate(&s.id))
            .map(|s| s.id.clone())
            .collect();

        let parent_completed = if self.all_children_done(&siblings, task_id) {
            if let Some(parent) = self.store.get_task(parent_id) {
                if parent.status != TaskStatus::Completed {
                    Some(parent_id.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        TaskCompletionResult {
            newly_ready,
            parent_completed,
        }
    }

    /// Check if a single task should transition to Ready and update it if so.
    /// Returns true if the task was activated.
    fn check_and_activate(&self, task_id: &TaskId) -> bool {
        let Some(task) = self.store.get_task(task_id) else {
            return false;
        };
        if task.status != TaskStatus::Pending {
            return false;
        }
        if !self.parent_is_active(&task) {
            return false;
        }
        if !self.all_deps_done(&task) {
            return false;
        }
        // Caller is responsible for logging errors from this update.
        self.store
            .update_task_status(task_id, TaskStatus::Ready)
            .is_ok()
    }

    fn parent_is_active(&self, task: &Task) -> bool {
        let Some(ref parent_id) = task.parent_id else {
            return true;
        };
        self.store
            .get_task(parent_id)
            .is_some_and(|p| p.status == TaskStatus::Ready || p.status == TaskStatus::InProgress)
    }

    fn all_deps_done(&self, task: &Task) -> bool {
        for dep_id in &task.depends_on {
            let done = self
                .store
                .get_task(dep_id)
                .is_some_and(|d| d.status == TaskStatus::Completed);
            if !done {
                return false;
            }
        }
        true
    }

    fn all_children_done(&self, siblings: &[Task], just_completed: &TaskId) -> bool {
        siblings
            .iter()
            .all(|s| s.status == TaskStatus::Completed || s.id == *just_completed)
    }
}

/// Result of processing a task completion.
#[derive(Debug, Default)]
pub struct TaskCompletionResult {
    /// Sibling tasks that just became Ready.
    pub newly_ready: Vec<TaskId>,
    /// Parent task that can now be completed (if all children are done).
    pub parent_completed: Option<TaskId>,
}
