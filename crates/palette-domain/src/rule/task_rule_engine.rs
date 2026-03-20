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
            let task = match self.store.get_task(task_id)? {
                Some(t) => t,
                None => continue,
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

        let task = match self.store.get_task(task_id)? {
            Some(t) => t,
            None => return Ok(effects),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;
    use crate::workflow::WorkflowId;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct MockTaskStore {
        tasks: RefCell<HashMap<String, Task>>,
        children: HashMap<String, Vec<String>>,
        deps: HashMap<String, Vec<String>>,
    }

    impl MockTaskStore {
        fn new() -> Self {
            Self {
                tasks: RefCell::new(HashMap::new()),
                children: HashMap::new(),
                deps: HashMap::new(),
            }
        }

        fn add_task(&mut self, id: &str, parent_id: Option<&str>, status: TaskStatus) {
            let task = Task {
                id: TaskId::new(id),
                workflow_id: WorkflowId::new("wf-test"),
                parent_id: parent_id.map(|p| TaskId::new(p)),
                title: id.to_string(),
                plan_path: None,
                status,
            };
            self.tasks.borrow_mut().insert(id.to_string(), task);
            if let Some(p) = parent_id {
                self.children
                    .entry(p.to_string())
                    .or_default()
                    .push(id.to_string());
            }
        }

        fn add_dep(&mut self, task_id: &str, depends_on: &str) {
            self.deps
                .entry(task_id.to_string())
                .or_default()
                .push(depends_on.to_string());
        }

        fn set_status(&self, id: &str, status: TaskStatus) {
            if let Some(task) = self.tasks.borrow_mut().get_mut(id) {
                task.status = status;
            }
        }
    }

    impl TaskStore for &MockTaskStore {
        type Error = String;

        fn get_task(&self, id: &TaskId) -> Result<Option<Task>, String> {
            Ok(self.tasks.borrow().get(id.as_ref()).cloned())
        }

        fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, String> {
            let tasks = self.tasks.borrow();
            let ids = self.children.get(parent_id.as_ref());
            Ok(ids
                .map(|ids| ids.iter().filter_map(|id| tasks.get(id).cloned()).collect())
                .unwrap_or_default())
        }

        fn get_task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>, String> {
            Ok(self
                .deps
                .get(task_id.as_ref())
                .map(|deps| deps.iter().map(|d| TaskId::new(d)).collect())
                .unwrap_or_default())
        }

        fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), String> {
            self.set_status(id.as_ref(), status);
            Ok(())
        }
    }

    #[test]
    fn tasks_without_deps_become_ready() {
        let mut store = MockTaskStore::new();
        store.add_task("root", None, TaskStatus::InProgress);
        store.add_task("a", Some("root"), TaskStatus::Pending);
        store.add_task("b", Some("root"), TaskStatus::Pending);
        store.add_dep("b", "a");

        let engine = TaskRuleEngine::new(&store);
        let task_ids = vec![TaskId::new("a"), TaskId::new("b")];
        let effects = engine.resolve_ready_tasks(&task_ids).unwrap();

        // Only "a" should be ready (no deps), "b" depends on "a"
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("a"),
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn completing_task_unblocks_dependents_and_propagates_to_parent() {
        let mut store = MockTaskStore::new();
        store.add_task("root", None, TaskStatus::InProgress);
        store.add_task("a", Some("root"), TaskStatus::Done);
        store.add_task("b", Some("root"), TaskStatus::Pending);
        store.add_dep("b", "a");

        let engine = TaskRuleEngine::new(&store);
        let effects = engine.on_task_completed(&TaskId::new("a")).unwrap();

        // "b" should become ready, parent should NOT complete (b is still pending)
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("b"),
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn all_children_done_completes_parent() {
        let mut store = MockTaskStore::new();
        store.add_task("root", None, TaskStatus::InProgress);
        store.add_task("a", Some("root"), TaskStatus::Done);
        store.add_task("b", Some("root"), TaskStatus::Done);

        let engine = TaskRuleEngine::new(&store);
        // "b" just completed
        let effects = engine.on_task_completed(&TaskId::new("b")).unwrap();

        assert!(effects.iter().any(|e| *e
            == TaskEffect::TaskStatusChanged {
                task_id: TaskId::new("root"),
                new_status: TaskStatus::Done,
            }));
    }
}
