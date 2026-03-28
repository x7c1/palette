#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStatus {
    Booting,
    Working,
    Idle,
    WaitingPermission,
    Crashed,
    Suspended,
}

impl WorkerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerStatus::Booting => "booting",
            WorkerStatus::Working => "working",
            WorkerStatus::Idle => "idle",
            WorkerStatus::WaitingPermission => "waiting_permission",
            WorkerStatus::Crashed => "crashed",
            WorkerStatus::Suspended => "suspended",
        }
    }
}
