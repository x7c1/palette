use super::{ContainerId, WorkerId, WorkerRole, WorkerSessionId, WorkerStatus};
use crate::task::TaskId;
use crate::terminal::TerminalTarget;

#[derive(Debug, Clone)]
pub struct WorkerState {
    pub id: WorkerId,
    pub role: WorkerRole,
    pub supervisor_id: WorkerId,
    pub container_id: ContainerId,
    pub terminal_target: TerminalTarget,
    pub status: WorkerStatus,
    pub session_id: Option<WorkerSessionId>,
    pub task_id: TaskId,
}
