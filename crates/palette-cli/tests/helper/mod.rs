#![allow(dead_code, unused_imports)]

mod fixtures;
mod session_guard;
mod spawn_server;
mod stub_container_runtime;
mod tmux;

pub use fixtures::*;
pub use session_guard::SessionGuard;
pub use spawn_server::spawn_server;
pub use stub_container_runtime::StubContainerRuntime;
pub use tmux::*;

// Re-export commonly used domain types for test convenience.
pub use palette_domain::job::{CreateJobRequest, JobDetail, JobStatus, JobType, ReviewStatus};
pub use palette_domain::task::TaskId;
pub use palette_domain::terminal::TerminalTarget;
pub use palette_domain::worker::{ContainerId, WorkerRole, WorkerStatus};
pub use palette_domain::workflow::WorkflowId;
pub use palette_usecase::{CreateTaskRequest, InsertWorkerRequest};
