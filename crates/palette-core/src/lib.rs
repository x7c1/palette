mod agent_role;
pub use agent_role::AgentRole;

mod agent_state;
pub use agent_state::AgentState;

mod agent_status;
pub use agent_status::AgentStatus;

mod config;
pub use config::Config;

mod container_id;
pub use container_id::ContainerId;

mod docker_config;
pub use docker_config::DockerConfig;

mod docker_manager;
pub use docker_manager::DockerManager;

pub mod orchestrator;

mod pending_delivery;
pub use pending_delivery::PendingDelivery;

mod persistent_state;
pub use persistent_state::PersistentState;

mod rules_config;
pub use rules_config::RulesConfig;

mod tmux_config;
pub use tmux_config::TmuxConfig;

mod tmux_target;
pub use tmux_target::TmuxTarget;

/// Backward-compatible re-export module for `palette_core::state::*`.
pub mod state {
    pub use crate::agent_role::AgentRole;
    pub use crate::agent_state::AgentState;
    pub use crate::agent_status::AgentStatus;
    pub use crate::container_id::ContainerId;
    pub use crate::persistent_state::PersistentState;
    pub use crate::tmux_target::TmuxTarget;
}

/// Backward-compatible re-export module for `palette_core::docker::*`.
pub mod docker {
    pub use crate::docker_manager::DockerManager;
}
