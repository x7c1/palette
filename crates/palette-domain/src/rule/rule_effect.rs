use crate::agent::{AgentId, AgentRole};
use crate::job::JobId;
use crate::review::Verdict;
use crate::task::TaskId;

/// Side effects produced by the rule engine after a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    // --- Job lifecycle ---
    /// A new job needs a member assigned (orchestrator should spawn member).
    AssignNewJob { job_id: JobId },
    /// A job's existing member should be reactivated (e.g. re-review cycle).
    ReactivateMember { job_id: JobId, member_id: AgentId },
    /// A member's job is done; orchestrator should destroy its container.
    DestroyMember { member_id: AgentId },

    // --- Craft/Review workflow ---
    /// A craft job reached InReview; activate child review tasks.
    CraftReadyForReview { craft_job_id: JobId },
    /// A review verdict was submitted; handle completion or changes_requested.
    ReviewVerdict {
        review_job_id: JobId,
        verdict: Verdict,
    },
    /// A job completed; check if its task can be completed and cascade.
    JobCompleted { job_id: JobId },

    // --- Supervisor lifecycle ---
    /// A composite task needs a supervisor spawned before it becomes InProgress.
    SpawnSupervisor { task_id: TaskId, role: AgentRole },
    /// A composite task completed; destroy its supervisor.
    DestroySupervisor { supervisor_id: AgentId },
}
