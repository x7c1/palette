use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DockerConfig {
    pub palette_url: String,
    #[serde(default = "default_leader_image")]
    pub leader_image: String,
    #[serde(default = "default_member_image")]
    pub member_image: String,
    #[serde(default = "default_settings_template")]
    pub settings_template: String,
    #[serde(default = "default_leader_prompt")]
    pub leader_prompt: String,
    #[serde(default = "default_review_integrator_image")]
    pub review_integrator_image: String,
    #[serde(default = "default_review_integrator_prompt")]
    pub review_integrator_prompt: String,
    #[serde(default = "default_crafter_prompt")]
    pub crafter_prompt: String,

    #[serde(default = "default_reviewer_prompt")]
    pub reviewer_prompt: String,
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
}

fn default_max_workers() -> usize {
    50
}

fn default_leader_image() -> String {
    "palette-leader:latest".to_string()
}

fn default_member_image() -> String {
    "palette-member:latest".to_string()
}

fn default_settings_template() -> String {
    "config/hooks/worker-settings.json".to_string()
}

fn default_leader_prompt() -> String {
    "prompts/leader.md".to_string()
}

fn default_review_integrator_image() -> String {
    "palette-leader:latest".to_string()
}

fn default_review_integrator_prompt() -> String {
    "prompts/review-integrator.md".to_string()
}

fn default_crafter_prompt() -> String {
    "prompts/crafter.md".to_string()
}

fn default_reviewer_prompt() -> String {
    "prompts/reviewer.md".to_string()
}
