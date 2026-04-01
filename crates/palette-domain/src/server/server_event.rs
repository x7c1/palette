use crate::job::JobId;
use crate::rule::RuleEffect;
use crate::worker::WorkerId;
use crate::workflow::WorkflowId;

/// Events emitted by the server for asynchronous processing by the orchestrator.
#[derive(Debug)]
pub enum ServerEvent {
    /// Rule engine produced effects that need orchestrator processing
    /// (auto-assign, destroy member, etc.).
    ProcessEffects { effects: Vec<RuleEffect> },
    /// Deliver queued messages to a specific target.
    DeliverMessages { target_id: WorkerId },
    /// Deliver queued messages to all idle targets.
    NotifyDeliveryLoop,
    /// Resume suspended workers: spawn readiness watchers and deliver messages.
    ResumeWorkers { worker_ids: Vec<WorkerId> },
    /// Suspend workers belonging to the specified workflow.
    SuspendWorkflow { workflow_id: WorkflowId },
    /// Validate that a review artifact exists after a review is submitted.
    /// If validation passes, the `pending_effects` are processed.
    /// If validation fails, the reviewer is re-instructed and effects are discarded.
    ValidateReviewArtifact {
        job_id: JobId,
        worker_id: crate::worker::WorkerId,
        pending_effects: Vec<crate::rule::RuleEffect>,
    },
    /// Validate that integrated-review.md exists after a ReviewIntegrator stops.
    ValidateIntegratedReviewArtifact {
        task_id: crate::task::TaskId,
        worker_id: crate::worker::WorkerId,
    },
    /// Validate an integrator's review submission: check that all child
    /// reviewers have written review.md before processing effects.
    /// If any review.md is missing, re-instruct the reviewer and hold effects.
    ValidateIntegratorSubmission {
        job_id: JobId,
        pending_effects: Vec<crate::rule::RuleEffect>,
    },
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
