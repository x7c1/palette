use palette_domain::job::{CreateJobRequest, Job, JobFilter, JobId, JobStatus, JobType};
use palette_domain::review::{ReviewComment, ReviewSubmission, SubmitReviewRequest};
use palette_domain::task::{TaskId, TaskState, TaskStatus};
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_domain::worker::{
    ContainerId, WorkerId, WorkerRole, WorkerSessionId, WorkerState, WorkerStatus,
};
use palette_domain::workflow::{Workflow, WorkflowId, WorkflowStatus};
use palette_usecase::container_runtime::{PlanDirMount, WorkspaceVolume};
use palette_usecase::{BlueprintReader, ContainerRuntime, DataStore, TerminalSession};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use palette_usecase::data_store::{CreateTaskRequest, InsertWorkerRequest};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

// ---------------------------------------------------------------------------
// MockDataStore
// ---------------------------------------------------------------------------

pub struct MockDataStore {
    pub workers: Mutex<Vec<WorkerState>>,
    pub messages: Mutex<HashMap<WorkerId, Vec<String>>>,
    pub status_updates: Mutex<Vec<(WorkerId, WorkerStatus)>>,
}

impl MockDataStore {
    pub fn new() -> Self {
        Self {
            workers: Mutex::new(Vec::new()),
            messages: Mutex::new(HashMap::new()),
            status_updates: Mutex::new(Vec::new()),
        }
    }

    pub fn with_workers(workers: Vec<WorkerState>) -> Self {
        let store = Self::new();
        *store.workers.lock().unwrap() = workers;
        store
    }
}

impl DataStore for MockDataStore {
    fn list_all_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        Ok(self.workers.lock().unwrap().clone())
    }

    fn update_worker_status(&self, id: &WorkerId, status: WorkerStatus) -> Result<(), BoxErr> {
        self.status_updates
            .lock()
            .unwrap()
            .push((id.clone(), status));
        // Also update the in-memory worker
        let mut workers = self.workers.lock().unwrap();
        if let Some(w) = workers.iter_mut().find(|w| w.id == *id) {
            w.status = status;
        }
        Ok(())
    }

    fn enqueue_message(&self, target_id: &WorkerId, message: &str) -> Result<(), BoxErr> {
        self.messages
            .lock()
            .unwrap()
            .entry(target_id.clone())
            .or_default()
            .push(message.to_string());
        Ok(())
    }

    fn has_pending_messages(&self, target_id: &WorkerId) -> Result<bool, BoxErr> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .get(target_id)
            .is_some_and(|m| !m.is_empty()))
    }

    fn find_worker(&self, id: &WorkerId) -> Result<Option<WorkerState>, BoxErr> {
        Ok(self
            .workers
            .lock()
            .unwrap()
            .iter()
            .find(|w| w.id == *id)
            .cloned())
    }

    // -- Methods not used in worker_monitor tests (stub implementations) --

    fn insert_worker(&self, _req: &InsertWorkerRequest) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn find_worker_by_container(&self, _: &ContainerId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_supervisors(&self, _: &WorkflowId) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_members(&self, _: &WorkflowId) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_booting_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn list_idle_or_waiting_workers(&self) -> Result<Vec<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn update_worker_session_id(&self, _: &WorkerId, _: &WorkerSessionId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn remove_worker(&self, _: &WorkerId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn find_supervisor_for_task(&self, _: &TaskId) -> Result<Option<WorkerState>, BoxErr> {
        unimplemented!()
    }
    fn create_job(&self, _: &CreateJobRequest) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn get_job(&self, _: &JobId) -> Result<Option<Job>, BoxErr> {
        unimplemented!()
    }
    fn get_job_by_task_id(&self, _: &TaskId) -> Result<Option<Job>, BoxErr> {
        unimplemented!()
    }
    fn list_jobs(&self, _: &JobFilter) -> Result<Vec<Job>, BoxErr> {
        unimplemented!()
    }
    fn assign_job(&self, _: &JobId, _: &WorkerId, _: JobType) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn update_job_status(&self, _: &JobId, _: JobStatus) -> Result<Job, BoxErr> {
        unimplemented!()
    }
    fn find_assignable_jobs(&self) -> Result<Vec<Job>, BoxErr> {
        unimplemented!()
    }
    fn count_active_members(&self) -> Result<usize, BoxErr> {
        unimplemented!()
    }
    fn submit_review(
        &self,
        _: &JobId,
        _: &SubmitReviewRequest,
    ) -> Result<ReviewSubmission, BoxErr> {
        unimplemented!()
    }
    fn get_review_submissions(&self, _: &JobId) -> Result<Vec<ReviewSubmission>, BoxErr> {
        unimplemented!()
    }
    fn get_review_comments(&self, _: i64) -> Result<Vec<ReviewComment>, BoxErr> {
        unimplemented!()
    }
    fn create_task(&self, _: &CreateTaskRequest) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn get_task_state(&self, _: &TaskId) -> Result<Option<TaskState>, BoxErr> {
        unimplemented!()
    }
    fn get_task_statuses(&self, _: &WorkflowId) -> Result<HashMap<TaskId, TaskStatus>, BoxErr> {
        unimplemented!()
    }
    fn update_task_status(&self, _: &TaskId, _: TaskStatus) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn create_workflow(&self, _: &WorkflowId, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn get_workflow(&self, _: &WorkflowId) -> Result<Option<Workflow>, BoxErr> {
        unimplemented!()
    }
    fn update_workflow_status(&self, _: &WorkflowId, _: WorkflowStatus) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn increment_worker_counter(&self, _: &WorkflowId) -> Result<usize, BoxErr> {
        unimplemented!()
    }
    fn dequeue_message(&self, _: &WorkerId) -> Result<Option<String>, BoxErr> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// MockContainerRuntime
// ---------------------------------------------------------------------------

pub struct MockContainerRuntime {
    pub running_containers: Mutex<std::collections::HashSet<String>>,
    pub started_containers: Mutex<Vec<ContainerId>>,
}

impl MockContainerRuntime {
    pub fn new() -> Self {
        Self {
            running_containers: Mutex::new(std::collections::HashSet::new()),
            started_containers: Mutex::new(Vec::new()),
        }
    }

    pub fn with_running(container_ids: &[&str]) -> Self {
        let mock = Self::new();
        {
            let mut set = mock.running_containers.lock().unwrap();
            for id in container_ids {
                set.insert(id.to_string());
            }
        }
        mock
    }
}

impl ContainerRuntime for MockContainerRuntime {
    fn is_container_running(&self, container_id: &str) -> bool {
        self.running_containers
            .lock()
            .unwrap()
            .contains(container_id)
    }

    fn start_container(&self, container_id: &ContainerId) -> Result<(), BoxErr> {
        self.started_containers
            .lock()
            .unwrap()
            .push(container_id.clone());
        Ok(())
    }

    fn claude_exec_command(
        &self,
        container_id: &ContainerId,
        prompt_file: &str,
        _role: WorkerRole,
    ) -> String {
        format!("mock-exec {container_id} {prompt_file}")
    }

    fn claude_resume_command(
        &self,
        container_id: &ContainerId,
        session_id: &WorkerSessionId,
        _role: WorkerRole,
    ) -> String {
        format!("mock-resume {container_id} {session_id}")
    }

    // -- Methods not used in worker_monitor tests --

    fn create_container(
        &self,
        _: &str,
        _: &str,
        _: WorkerRole,
        _: &str,
        _: Option<WorkspaceVolume>,
        _: Option<PlanDirMount>,
    ) -> Result<ContainerId, BoxErr> {
        unimplemented!()
    }
    fn stop_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn remove_container(&self, _: &ContainerId) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn list_managed_containers(&self) -> Result<Vec<ContainerId>, BoxErr> {
        unimplemented!()
    }
    fn write_settings(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn copy_file_to_container(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn copy_dir_to_container(&self, _: &ContainerId, _: &Path, _: &str) -> Result<(), BoxErr> {
        unimplemented!()
    }
    fn read_container_file(&self, _: &ContainerId, _: &str, _: usize) -> Result<String, BoxErr> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// MockTerminalSession
// ---------------------------------------------------------------------------

pub struct MockTerminalSession {
    pub pane_content: Mutex<HashMap<String, String>>,
    pub sent_keys: Mutex<Vec<(String, String)>>,
}

impl MockTerminalSession {
    pub fn new() -> Self {
        Self {
            pane_content: Mutex::new(HashMap::new()),
            sent_keys: Mutex::new(Vec::new()),
        }
    }

    pub fn set_pane_content(&self, target: &str, content: &str) {
        self.pane_content
            .lock()
            .unwrap()
            .insert(target.to_string(), content.to_string());
    }
}

impl TerminalSession for MockTerminalSession {
    fn capture_pane(&self, target: &TerminalTarget) -> Result<String, BoxErr> {
        Ok(self
            .pane_content
            .lock()
            .unwrap()
            .get(target.as_ref())
            .cloned()
            .unwrap_or_default())
    }

    fn send_keys(&self, target: &TerminalTarget, text: &str) -> Result<(), BoxErr> {
        self.sent_keys
            .lock()
            .unwrap()
            .push((target.as_ref().to_string(), text.to_string()));
        Ok(())
    }

    fn send_keys_no_enter(&self, target: &TerminalTarget, text: &str) -> Result<(), BoxErr> {
        self.sent_keys
            .lock()
            .unwrap()
            .push((target.as_ref().to_string(), text.to_string()));
        Ok(())
    }

    // -- Methods not used in worker_monitor tests --

    fn create_target(&self, _: &str) -> Result<TerminalTarget, BoxErr> {
        unimplemented!()
    }
    fn create_pane(&self, _: &TerminalTarget) -> Result<TerminalTarget, BoxErr> {
        unimplemented!()
    }
    fn kill_session(&self, _: &TerminalSessionName) -> Result<(), BoxErr> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// MockBlueprintReader
// ---------------------------------------------------------------------------

pub struct MockBlueprintReader;

impl BlueprintReader for MockBlueprintReader {
    fn read_blueprint(
        &self,
        _path: &Path,
        _workflow_id: &WorkflowId,
    ) -> Result<palette_domain::task::TaskTree, BoxErr> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

pub fn make_worker(id: &str, role: WorkerRole, status: WorkerStatus) -> WorkerState {
    WorkerState {
        id: WorkerId::new(id),
        workflow_id: WorkflowId::new("wf-test"),
        role,
        supervisor_id: WorkerId::new("sup-1"),
        container_id: ContainerId::new(format!("container-{id}")),
        terminal_target: TerminalTarget::new(format!("pane-{id}")),
        status,
        session_id: None,
        task_id: TaskId::new(format!("task-{id}")),
    }
}
