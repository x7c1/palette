use anyhow::Context as _;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_state_path")]
    pub state_path: String,
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub docker: DockerConfig,
}

#[derive(Debug, Deserialize)]
pub struct TmuxConfig {
    pub session_name: String,
}

#[derive(Debug, Deserialize)]
pub struct RulesConfig {
    #[serde(default = "default_max_review_rounds")]
    pub max_review_rounds: u32,
}

#[derive(Debug, Deserialize)]
pub struct DockerConfig {
    #[serde(default = "default_network")]
    pub network: String,
    #[serde(default = "default_palette_url")]
    pub palette_url: String,
}

fn default_db_path() -> String {
    "data/palette.db".to_string()
}

fn default_state_path() -> String {
    "data/state.json".to_string()
}

fn default_max_review_rounds() -> u32 {
    5
}

fn default_network() -> String {
    "host".to_string()
}

fn default_palette_url() -> String {
    "http://host.docker.internal:7100".to_string()
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            max_review_rounds: default_max_review_rounds(),
        }
    }
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            network: default_network(),
            palette_url: default_palette_url(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let config: Config =
            toml::from_str(&content).with_context(|| "failed to parse config file")?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
port = 7100

[tmux]
session_name = "palette"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.port, 7100);
        assert_eq!(config.tmux.session_name, "palette");
        assert_eq!(config.db_path, "data/palette.db");
        assert_eq!(config.state_path, "data/state.json");
        assert_eq!(config.rules.max_review_rounds, 5);
        assert_eq!(
            config.docker.palette_url,
            "http://host.docker.internal:7100"
        );
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
port = 7100
db_path = "custom/path.db"
state_path = "custom/state.json"

[tmux]
session_name = "palette"

[rules]
max_review_rounds = 3

[docker]
network = "bridge"
palette_url = "http://localhost:8080"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.db_path, "custom/path.db");
        assert_eq!(config.rules.max_review_rounds, 3);
        assert_eq!(config.docker.network, "bridge");
        assert_eq!(config.docker.palette_url, "http://localhost:8080");
    }
}
