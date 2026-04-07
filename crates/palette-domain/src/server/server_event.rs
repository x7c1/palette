use crate::job::JobId;
use crate::task::TaskId;
use crate::worker::WorkerId;
use crate::workflow::WorkflowId;

/// Events emitted by the server for asynchronous processing by the orchestrator.
///
/// Each variant represents a domain event — "what happened" — rather than
/// an instruction. The orchestrator decides how to react.
#[derive(Debug)]
pub enum ServerEvent {
    // --- Domain events (craft / review cycle) ---
    /// A craft job has been marked Done. Orchestrator should destroy the
    /// crafter member and cascade task completion.
    CraftDone { job_id: JobId },
    /// A craft job has reached InReview. Orchestrator should activate
    /// child review tasks.
    CraftReadyForReview { craft_job_id: JobId },
    /// A review submission has been recorded in the DB. Orchestrator should
    /// validate artifacts and handle the verdict.
    ReviewSubmitted { review_job_id: JobId },
    /// A ReviewIntegrator worker has stopped. Orchestrator should validate
    /// that integrated-review.json was written.
    ReviewIntegratorStopped {
        task_id: TaskId,
        worker_id: WorkerId,
    },

    // --- Workflow lifecycle ---
    /// A new workflow has been created and tasks registered.
    /// Orchestrator should activate the root task and spawn initial workers.
    ActivateWorkflow { workflow_id: WorkflowId },
    /// A blueprint has been re-applied and new tasks added.
    /// Orchestrator should activate newly Ready tasks.
    ActivateNewTasks { workflow_id: WorkflowId },

    // --- Infrastructure events ---
    /// Deliver queued messages to a specific target.
    DeliverMessages { target_id: WorkerId },
    /// Deliver queued messages to all idle targets.
    NotifyDeliveryLoop,
    /// Resume suspended workers: spawn readiness watchers and deliver messages.
    ResumeWorkers { worker_ids: Vec<WorkerId> },
    /// Suspend workers belonging to the specified workflow.
    SuspendWorkflow { workflow_id: WorkflowId },
    /// An orchestrator task's command has completed.
    OrchestratorTaskCompleted {
        job_id: JobId,
        success: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
        duration_ms: u64,
    },
}
