use crate::agent::AgentId;
use crate::task::{TaskId, TaskStatus};

/// Side effects produced by the rule engine after a state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEffect {
    /// A task's status was changed by the rule engine.
    StatusChanged {
        task_id: TaskId,
        new_status: TaskStatus,
    },
    /// The review loop exceeded the max rounds; escalate.
    Escalated { task_id: TaskId, round: i32 },
    /// A task is ready to be assigned to a member (orchestrator should spawn member).
    AutoAssign { task_id: TaskId },
    /// A member's task is done; orchestrator should destroy its container.
    DestroyMember { member_id: AgentId },
}
