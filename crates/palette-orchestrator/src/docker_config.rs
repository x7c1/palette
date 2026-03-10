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
    #[serde(default = "default_member_prompt")]
    pub member_prompt: String,
    #[serde(default = "default_max_members")]
    pub max_members: usize,
}

fn default_max_members() -> usize {
    3
}

fn default_leader_image() -> String {
    "palette-leader:latest".to_string()
}

fn default_member_image() -> String {
    "palette-member:latest".to_string()
}

fn default_settings_template() -> String {
    "config/hooks/member-settings.json".to_string()
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

fn default_member_prompt() -> String {
    "prompts/member.md".to_string()
}
