use palette_domain::job::Priority;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityYaml {
    High,
    Medium,
    Low,
}

impl From<PriorityYaml> for Priority {
    fn from(p: PriorityYaml) -> Self {
        match p {
            PriorityYaml::High => Priority::High,
            PriorityYaml::Medium => Priority::Medium,
            PriorityYaml::Low => Priority::Low,
        }
    }
}
