use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRole {
    PermissionSupervisor,
    ReviewIntegrator,
    Member,
}

impl WorkerRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkerRole::PermissionSupervisor => "permission_supervisor",
            WorkerRole::ReviewIntegrator => "review_integrator",
            WorkerRole::Member => "member",
        }
    }

    /// Returns true if this role acts as a supervisor (can receive member events).
    pub fn is_supervisor(&self) -> bool {
        matches!(self, WorkerRole::PermissionSupervisor)
    }

    /// Returns true if this role should bypass Claude Code's permission system.
    /// Both supervisors and integrators run autonomously without human approval.
    pub fn skip_permissions(&self) -> bool {
        matches!(
            self,
            WorkerRole::PermissionSupervisor | WorkerRole::ReviewIntegrator
        )
    }
}

impl fmt::Display for WorkerRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
