#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Booting,
    Working,
    Idle,
    WaitingPermission,
    Crashed,
}
