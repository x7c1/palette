mod error;
pub use error::{Error, Result};

mod docker_config;
pub use docker_config::{CallbackNetwork, DockerConfig};

mod perspectives_config;
pub use perspectives_config::{
    PerspectiveEntry, PerspectivePath, PerspectivesConfig, PerspectivesConfigError,
    PerspectivesValidationError, ValidatedPerspective, ValidatedPerspectives,
};

pub mod github_client;

mod orchestrator;
pub use orchestrator::Orchestrator;
pub use orchestrator::infra::diff_gen::cleanup_orphan_diff_gen_containers;
pub use orchestrator::infra::workspace;

#[cfg(test)]
pub(crate) mod testing;
