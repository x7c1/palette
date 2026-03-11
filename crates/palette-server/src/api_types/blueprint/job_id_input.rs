use palette_domain::job::JobId;
use serde::Deserialize;

/// Job ID as represented in YAML input.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub(super) struct JobIdInput(pub String);

impl From<JobIdInput> for JobId {
    fn from(id: JobIdInput) -> Self {
        JobId::new(id.0)
    }
}
