use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DockerConfig {
    #[serde(alias = "palette_url")]
    pub worker_callback_url: String,
    #[serde(default = "default_callback_network")]
    pub callback_network: CallbackNetwork,
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallbackNetwork {
    Auto,
    Host,
    Bridge,
}

fn default_callback_network() -> CallbackNetwork {
    CallbackNetwork::Auto
}

fn default_max_workers() -> usize {
    3
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
