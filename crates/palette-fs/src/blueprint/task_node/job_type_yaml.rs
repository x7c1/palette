use palette_domain::job::JobType;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobTypeYaml {
    Craft,
    Review,
}

impl From<JobTypeYaml> for JobType {
    fn from(t: JobTypeYaml) -> Self {
        match t {
            JobTypeYaml::Craft => JobType::Craft,
            JobTypeYaml::Review => JobType::Review,
        }
    }
}
