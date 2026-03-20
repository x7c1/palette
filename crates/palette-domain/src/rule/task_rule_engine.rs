use super::TaskEffect;
use crate::task::{TaskId, TaskStatus, TaskStore};

pub struct TaskRuleEngine<S> {
    store: S,
}

impl<S: TaskStore> TaskRuleEngine<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Determine which tasks in a workflow should transition to Ready.
    /// A task becomes Ready when all of its dependencies are Done.
    /// Tasks with no dependencies start as Ready immediately.
    pub fn resolve_ready_tasks(&self, task_ids: &[TaskId]) -> Result<Vec<TaskEffect>, S::Error> {
        let mut effects = Vec::new();

        for task_id in task_ids {
            let Some(task) = self.store.get_task(task_id)? else {
                continue;
            };

            if task.status != TaskStatus::Pending {
                continue;
            }

            // Parent must be active (Ready or InProgress) for child to become Ready
            if let Some(ref parent_id) = task.parent_id {
                let parent_active = self.store.get_task(parent_id)?.is_some_and(|p| {
                    p.status == TaskStatus::Ready || p.status == TaskStatus::InProgress
                });
                if !parent_active {
                    continue;
                }
            }

            let deps = self.store.get_task_dependencies(task_id)?;
            let all_deps_done = deps.iter().all(|dep_id| {
                self.store
                    .get_task(dep_id)
                    .ok()
                    .flatten()
                    .is_some_and(|dep| dep.status == TaskStatus::Done)
            });

            if all_deps_done {
                effects.push(TaskEffect::TaskStatusChanged {
                    task_id: task_id.clone(),
                    new_status: TaskStatus::Ready,
                });
            }
        }

        Ok(effects)
    }

    /// When a task completes, check if the parent task can also complete.
    /// A parent task is complete when all its children are Done.
    pub fn on_task_completed(&self, task_id: &TaskId) -> Result<Vec<TaskEffect>, S::Error> {
        let mut effects = Vec::new();

        let Some(task) = self.store.get_task(task_id)? else {
            return Ok(effects);
        };

        // Check if any sibling tasks can now become Ready
        if let Some(ref parent_id) = task.parent_id {
            let siblings = self.store.get_child_tasks(parent_id)?;

            for sibling in &siblings {
                if sibling.status != TaskStatus::Pending {
                    continue;
                }
                let deps = self.store.get_task_dependencies(&sibling.id)?;
                let all_deps_done = deps.iter().all(|dep_id| {
                    self.store
                        .get_task(dep_id)
                        .ok()
                        .flatten()
                        .is_some_and(|dep| dep.status == TaskStatus::Done)
                });
                if all_deps_done {
                    effects.push(TaskEffect::TaskStatusChanged {
                        task_id: sibling.id.clone(),
                        new_status: TaskStatus::Ready,
                    });
                }
            }

            // Check if parent can complete (all children Done)
            let all_children_done = siblings.iter().all(|s| {
                s.status == TaskStatus::Done || s.id == *task_id // the task that just completed
            });
            if all_children_done {
                let parent = self.store.get_task(parent_id)?;
                if let Some(p) = parent
                    && p.status != TaskStatus::Done
                {
                    effects.push(TaskEffect::TaskStatusChanged {
                        task_id: parent_id.clone(),
                        new_status: TaskStatus::Done,
                    });
                }
            }
        }

        Ok(effects)
    }
}
