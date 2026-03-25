use super::{ContainerId, WorkerId, WorkerRole, WorkerSessionId, WorkerStatus};
use crate::task::TaskId;
use crate::terminal::TerminalTarget;
use crate::workflow::WorkflowId;

#[derive(Debug, Clone)]
pub struct WorkerState {
    pub id: WorkerId,
    pub workflow_id: WorkflowId,
    pub role: WorkerRole,
    pub supervisor_id: WorkerId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub status: WorkerStatus,
    pub session_id: Option<WorkerSessionId>,
    pub task_id: TaskId,
}
