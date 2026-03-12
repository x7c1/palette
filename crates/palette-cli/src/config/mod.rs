mod rules_config;
pub use rules_config::RulesConfig;

mod tmux_config;
pub use tmux_config::TmuxConfig;

use palette_orchestrator::DockerConfig;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_state_path")]
    pub state_path: String,
    #[serde(default = "default_plan_dir")]
    pub plan_dir: String,
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    pub docker: DockerConfig,
}

fn default_db_path() -> String {
    "data/palette.db".to_string()
}

fn default_state_path() -> String {
    "data/state.json".to_string()
}

fn default_plan_dir() -> String {
    "data/plans".to_string()
}

impl Config {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
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

[docker]
palette_url = "http://127.0.0.1:7100"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.port, 7100);
        assert_eq!(config.tmux.session_name, "palette");
        assert_eq!(config.db_path, "data/palette.db");
        assert_eq!(config.state_path, "data/state.json");
        assert_eq!(config.rules.max_review_rounds, 5);
        assert_eq!(config.docker.palette_url, "http://127.0.0.1:7100");
        assert_eq!(config.docker.leader_image, "palette-leader:latest");
        assert_eq!(config.docker.member_image, "palette-member:latest");
        assert_eq!(
            config.docker.review_integrator_image,
            "palette-leader:latest"
        );
        assert_eq!(
            config.docker.review_integrator_prompt,
            "prompts/review-integrator.md"
        );
    }

    #[test]
    fn missing_docker_section_is_error() {
        let toml = r#"
port = 7100

[tmux]
session_name = "palette"
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(
            result.is_err(),
            "missing [docker] section should be an error"
        );
    }

    #[test]
    fn missing_palette_url_is_error() {
        let toml = r#"
port = 7100

[tmux]
session_name = "palette"

[docker]
leader_image = "custom:latest"
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(result.is_err(), "missing palette_url should be an error");
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
palette_url = "http://localhost:8080"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.db_path, "custom/path.db");
        assert_eq!(config.rules.max_review_rounds, 3);
        assert_eq!(config.docker.palette_url, "http://localhost:8080");
    }
}
