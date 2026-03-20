use palette_domain::job::JobType;
use serde::Deserialize;

/// Job type as represented in YAML input.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobTypeInput {
    Craft,
    Review,
}

impl From<JobTypeInput> for JobType {
    fn from(t: JobTypeInput) -> Self {
        match t {
            JobTypeInput::Craft => JobType::Craft,
            JobTypeInput::Review => JobType::Review,
        }
    }
}
