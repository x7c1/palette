pub mod config;
pub use config::*;

pub mod models;
pub use models::*;

mod docker_manager;
pub use docker_manager::DockerManager;

pub mod orchestrator;

mod persistent_state;
pub use persistent_state::PersistentState;

/// Backward-compatible re-export module for `palette_core::state::*`.
pub mod state {
    pub use crate::models::AgentRole;
    pub use crate::models::AgentState;
    pub use crate::models::AgentStatus;
    pub use crate::models::ContainerId;
    pub use crate::models::TmuxTarget;
    pub use crate::persistent_state::PersistentState;
}

/// Backward-compatible re-export module for `palette_core::docker::*`.
pub mod docker {
    pub use crate::docker_manager::DockerManager;
}
