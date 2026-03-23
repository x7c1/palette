use palette_db::Database;
use palette_domain::task::{Task, TaskId, TaskStatus, TaskStore, TaskTree, TaskTreeNode};
use palette_domain::workflow::WorkflowId;
use std::cell::RefCell;
use std::collections::HashMap;

/// TaskStore implementation that combines a TaskTree (structure from Blueprint)
/// with task statuses (execution state from DB) to produce full Task objects.
///
/// Reads are served from in-memory data (TaskTree + cached statuses).
/// Writes go to both the in-memory cache and the database.
pub struct TaskStoreImpl<'a> {
    db: &'a Database,
    tree: TaskTree,
    workflow_id: WorkflowId,
    statuses: RefCell<HashMap<TaskId, TaskStatus>>,
}

impl<'a> TaskStoreImpl<'a> {
    /// Build a TaskStoreImpl from a TaskTree and a map of task statuses.
    pub fn new(
        db: &'a Database,
        tree: TaskTree,
        workflow_id: WorkflowId,
        statuses: HashMap<TaskId, TaskStatus>,
    ) -> Self {
        Self {
            db,
            tree,
            workflow_id,
            statuses: RefCell::new(statuses),
        }
    }

    /// Build a TaskStoreImpl by reading the Blueprint file and task statuses from DB.
    pub fn from_db(db: &'a Database, workflow_id: &WorkflowId) -> Result<Self, Error> {
        let workflow = db
            .get_workflow(workflow_id)
            .map_err(Error::Db)?
            .ok_or_else(|| Error::WorkflowNotFound(workflow_id.clone()))?;

        let blueprint = palette_fs::read_blueprint(std::path::Path::new(&workflow.blueprint_path))
            .map_err(Error::Blueprint)?;
        let tree = blueprint.to_task_tree(workflow_id);
        let statuses = db.get_task_statuses(workflow_id).map_err(Error::Db)?;

        Ok(Self::new(db, tree, workflow_id.clone(), statuses))
    }

    pub fn tree(&self) -> &TaskTree {
        &self.tree
    }

    pub fn root_id(&self) -> &TaskId {
        self.tree.root_id()
    }

    fn build_task(&self, node: &TaskTreeNode) -> Task {
        let statuses = self.statuses.borrow();
        let status = statuses
            .get(&node.id)
            .copied()
            .unwrap_or(TaskStatus::Pending);

        let children: Vec<Task> = node
            .children
            .iter()
            .filter_map(|child_id| self.tree.get(child_id))
            .map(|child_node| self.build_task(child_node))
            .collect();

        Task {
            id: node.id.clone(),
            workflow_id: self.workflow_id.clone(),
            parent_id: node.parent_id.clone(),
            key: node.key.clone(),
            plan_path: node.plan_path.clone(),
            job_type: node.job_type,
            description: node.description.clone(),
            priority: node.priority,
            repository: node.repository.clone(),
            status,
            children,
            depends_on: node.depends_on.clone(),
        }
    }
}

impl TaskStore for TaskStoreImpl<'_> {
    type Error = palette_db::Error;

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, Self::Error> {
        Ok(self.tree.get(id).map(|node| self.build_task(node)))
    }

    fn get_child_tasks(&self, parent_id: &TaskId) -> Result<Vec<Task>, Self::Error> {
        let Some(parent_node) = self.tree.get(parent_id) else {
            return Ok(vec![]);
        };
        Ok(parent_node
            .children
            .iter()
            .filter_map(|child_id| self.tree.get(child_id))
            .map(|child_node| self.build_task(child_node))
            .collect())
    }

    fn update_task_status(&self, id: &TaskId, status: TaskStatus) -> Result<(), Self::Error> {
        self.db.update_task_status(id, status)?;
        self.statuses.borrow_mut().insert(id.clone(), status);
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    Db(palette_db::Error),
    Blueprint(palette_fs::BlueprintReadError),
    WorkflowNotFound(WorkflowId),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Db(e) => write!(f, "database error: {e}"),
            Error::Blueprint(e) => write!(f, "blueprint error: {e}"),
            Error::WorkflowNotFound(id) => write!(f, "workflow not found: {id}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use palette_domain::job::JobType;
    use palette_domain::task::TaskTree;

    fn setup() -> (Database, TaskTree, WorkflowId) {
        let db = Database::open_in_memory().unwrap();
        let wf_id = WorkflowId::new("wf-test");
        db.create_workflow(&wf_id, "/dev/null").unwrap();

        let yaml = r#"
task:
  key: root
  children:
    - key: a
      type: craft
      plan_path: test/a
    - key: b
      type: craft
      plan_path: test/b
      depends_on: [a]
"#;
        let blueprint: palette_fs::TaskTreeBlueprint = serde_yaml::from_str(yaml).unwrap();
        let tree = blueprint.to_task_tree(&wf_id);

        // Register tasks in DB
        for task_id in tree.task_ids() {
            db.create_task(&palette_db::CreateTaskRequest {
                id: task_id.clone(),
                workflow_id: wf_id.clone(),
            })
            .unwrap();
        }

        (db, tree, wf_id)
    }

    /// Helper to get TaskId by key from the tree.
    fn id(tree: &TaskTree, key: &str) -> TaskId {
        tree.find_by_key(key).unwrap().id.clone()
    }

    #[test]
    fn get_task_returns_full_task() {
        let (db, tree, wf_id) = setup();
        let root_id = id(&tree, "root");
        let a_id = id(&tree, "a");

        db.update_task_status(&root_id, TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&a_id, TaskStatus::Ready).unwrap();

        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id.clone(), statuses);

        let root = store.get_task(&root_id).unwrap().unwrap();
        assert_eq!(root.key, "root");
        assert_eq!(root.status, TaskStatus::InProgress);
        assert_eq!(root.workflow_id, wf_id);
        assert_eq!(root.children.len(), 2);

        let a = store.get_task(&a_id).unwrap().unwrap();
        assert_eq!(a.job_type, Some(JobType::Craft));
        assert_eq!(a.status, TaskStatus::Ready);
        assert!(a.depends_on.is_empty());

        let b_id = id(&store.tree(), "b");
        let b = store.get_task(&b_id).unwrap().unwrap();
        assert_eq!(b.status, TaskStatus::Pending);
        assert_eq!(b.depends_on, vec![a_id]);
    }

    #[test]
    fn get_child_tasks_returns_children() {
        let (db, tree, wf_id) = setup();
        let root_id = id(&tree, "root");
        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id, statuses);

        let children = store.get_child_tasks(&root_id).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn update_task_status_writes_to_db() {
        let (db, tree, wf_id) = setup();
        let a_id = id(&tree, "a");
        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id.clone(), statuses);

        store
            .update_task_status(&a_id, TaskStatus::Completed)
            .unwrap();

        // Verify in-memory cache is updated
        let a = store.get_task(&a_id).unwrap().unwrap();
        assert_eq!(a.status, TaskStatus::Completed);

        // Verify DB is updated
        let state = db.get_task_state(&a_id).unwrap().unwrap();
        assert_eq!(state.status, TaskStatus::Completed);
    }

    #[test]
    fn task_rule_engine_resolves_ready_tasks() {
        use palette_domain::rule::{TaskEffect, TaskRuleEngine};

        let (db, tree, wf_id) = setup();
        let root_id = id(&tree, "root");
        let a_id = id(&tree, "a");
        let b_id = id(&tree, "b");

        db.update_task_status(&root_id, TaskStatus::InProgress)
            .unwrap();

        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id, statuses);
        let engine = TaskRuleEngine::new(&store);

        let task_ids = vec![a_id.clone(), b_id];
        let effects = engine.resolve_ready_tasks(&task_ids).unwrap();

        // Only 'a' should become Ready (b depends on a)
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: a_id,
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn task_rule_engine_cascades_completion() {
        use palette_domain::rule::{TaskEffect, TaskRuleEngine};

        let (db, tree, wf_id) = setup();
        let root_id = id(&tree, "root");
        let a_id = id(&tree, "a");
        let b_id = id(&tree, "b");

        db.update_task_status(&root_id, TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&a_id, TaskStatus::Completed).unwrap();

        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id, statuses);
        let engine = TaskRuleEngine::new(&store);

        let effects = engine.on_task_completed(&a_id).unwrap();

        // b should become Ready (dependency on a is satisfied)
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0],
            TaskEffect::TaskStatusChanged {
                task_id: b_id,
                new_status: TaskStatus::Ready,
            }
        );
    }

    #[test]
    fn all_children_done_completes_parent() {
        use palette_domain::rule::{TaskEffect, TaskRuleEngine};

        let (db, tree, wf_id) = setup();
        let root_id = id(&tree, "root");
        let a_id = id(&tree, "a");
        let b_id = id(&tree, "b");

        db.update_task_status(&root_id, TaskStatus::InProgress)
            .unwrap();
        db.update_task_status(&a_id, TaskStatus::Completed).unwrap();
        db.update_task_status(&b_id, TaskStatus::Completed).unwrap();

        let statuses = db.get_task_statuses(&wf_id).unwrap();
        let store = TaskStoreImpl::new(&db, tree, wf_id, statuses);
        let engine = TaskRuleEngine::new(&store);

        let effects = engine.on_task_completed(&b_id).unwrap();

        assert!(effects.iter().any(|e| *e
            == TaskEffect::TaskStatusChanged {
                task_id: root_id.clone(),
                new_status: TaskStatus::Completed,
            }));
    }
}
