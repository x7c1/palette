use palette_domain::task::Priority;
use serde::Deserialize;

/// Priority as represented in YAML input.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PriorityInput {
    High,
    Medium,
    Low,
}

impl From<PriorityInput> for Priority {
    fn from(p: PriorityInput) -> Self {
        match p {
            PriorityInput::High => Priority::High,
            PriorityInput::Medium => Priority::Medium,
            PriorityInput::Low => Priority::Low,
        }
    }
}
