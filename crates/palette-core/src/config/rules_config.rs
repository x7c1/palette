use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RulesConfig {
    #[serde(default = "default_max_review_rounds")]
    pub max_review_rounds: u32,
}

fn default_max_review_rounds() -> u32 {
    5
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            max_review_rounds: default_max_review_rounds(),
        }
    }
}
