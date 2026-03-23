use crate::agent::{AgentId, AgentRole};
use crate::job::{JobId, JobStatus};
use crate::task::TaskId;

/// Side effects produced by the rule engine after a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// A job's status was changed by the rule engine.
    StatusChanged {
        job_id: JobId,
        new_status: JobStatus,
    },
    /// The review loop exceeded the max rounds; escalate.
    Escalated { job_id: JobId, round: i32 },
    /// A job is ready to be assigned to a member (orchestrator should spawn member).
    AutoAssign { job_id: JobId },
    /// A member's job is done; orchestrator should destroy its container.
    DestroyMember { member_id: AgentId },
    /// A composite task needs a supervisor spawned before it becomes InProgress.
    SpawnSupervisor { task_id: TaskId, role: AgentRole },
    /// A composite task completed; destroy its supervisor.
    DestroySupervisor { supervisor_id: AgentId },
}
