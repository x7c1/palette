mod error;
pub use error::{Error, Result};

mod docker_config;
pub use docker_config::{CallbackNetwork, DockerConfig};

mod perspectives_config;
pub use perspectives_config::{
    PerspectiveEntry, PerspectivePath, PerspectivesConfig, PerspectivesValidationError,
    ValidatedPerspective, ValidatedPerspectives,
};

mod orchestrator;
pub use orchestrator::Orchestrator;
pub use orchestrator::infra::workspace;

#[cfg(test)]
pub(crate) mod testing;
