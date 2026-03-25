#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Booting,
    Working,
    Idle,
    WaitingPermission,
    Crashed,
}
