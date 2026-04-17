mod rules_config;
pub use rules_config::RulesConfig;

mod tmux_config;
pub use tmux_config::TmuxConfig;

use palette_orchestrator::{DockerConfig, PerspectivesConfig};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[cfg(test)]
use palette_orchestrator::CallbackNetwork;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[allow(dead_code)]
    pub port: u16,
    pub operator_api_url: String,
    pub server_bind_addr: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    pub tmux: TmuxConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    pub docker: DockerConfig,
    #[serde(default, flatten)]
    pub perspectives: PerspectivesConfig,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

impl Config {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("palette.db")
    }

    pub fn plan_dir(&self) -> PathBuf {
        self.data_dir.join("plans")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml = r#"
port = 7100
operator_api_url = "http://127.0.0.1:7100"
server_bind_addr = "0.0.0.0:7100"

[tmux]
session_name = "palette"

[docker]
worker_callback_url = "http://127.0.0.1:7100"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.port, 7100);
        assert_eq!(config.tmux.session_name, "palette");
        assert_eq!(config.data_dir, PathBuf::from("data"));
        assert_eq!(config.db_path(), PathBuf::from("data/palette.db"));
        assert_eq!(config.plan_dir(), PathBuf::from("data/plans"));
        assert_eq!(config.rules.max_review_rounds, 5);
        assert_eq!(config.operator_api_url, "http://127.0.0.1:7100");
        assert_eq!(config.server_bind_addr, "0.0.0.0:7100");
        assert_eq!(config.docker.worker_callback_url, "http://127.0.0.1:7100");
        assert_eq!(config.docker.callback_network, CallbackNetwork::Auto);
        assert_eq!(config.docker.approver_image, "palette-supervisor:latest");
        assert_eq!(config.docker.member_image, "palette-member:latest");
        assert_eq!(
            config.docker.review_integrator_image,
            "palette-supervisor:latest"
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
    fn missing_operator_api_url_is_error() {
        let toml = r#"
port = 7100
server_bind_addr = "0.0.0.0:7100"

[tmux]
session_name = "palette"

[docker]
worker_callback_url = "http://127.0.0.1:7100"
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(
            result.is_err(),
            "missing operator_api_url should be an error"
        );
    }

    #[test]
    fn missing_server_bind_addr_is_error() {
        let toml = r#"
port = 7100
operator_api_url = "http://127.0.0.1:7100"

[tmux]
session_name = "palette"

[docker]
worker_callback_url = "http://127.0.0.1:7100"
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(
            result.is_err(),
            "missing server_bind_addr should be an error"
        );
    }

    #[test]
    fn missing_worker_callback_url_is_error() {
        let toml = r#"
port = 7100

[tmux]
session_name = "palette"

[docker]
approver_image = "custom:latest"
"#;
        let result: Result<Config, _> = toml::from_str(toml);
        assert!(
            result.is_err(),
            "missing worker_callback_url should be an error"
        );
    }

    #[test]
    fn parse_full_config() {
        let toml = r#"
port = 7100
operator_api_url = "http://palette.local:7100"
server_bind_addr = "127.0.0.1:7100"
data_dir = "/var/lib/palette"

[tmux]
session_name = "palette"

[rules]
max_review_rounds = 3

[docker]
worker_callback_url = "http://localhost:8080"
callback_network = "bridge"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.data_dir, PathBuf::from("/var/lib/palette"));
        assert_eq!(
            config.db_path(),
            PathBuf::from("/var/lib/palette/palette.db")
        );
        assert_eq!(config.plan_dir(), PathBuf::from("/var/lib/palette/plans"));
        assert_eq!(config.rules.max_review_rounds, 3);
        assert_eq!(config.operator_api_url, "http://palette.local:7100");
        assert_eq!(config.server_bind_addr, "127.0.0.1:7100");
        assert_eq!(config.docker.worker_callback_url, "http://localhost:8080");
        assert_eq!(config.docker.callback_network, CallbackNetwork::Bridge);
    }

    #[test]
    fn legacy_palette_url_is_still_accepted() {
        let toml = r#"
port = 7100
operator_api_url = "http://127.0.0.1:7100"
server_bind_addr = "0.0.0.0:7100"

[tmux]
session_name = "palette"

[docker]
palette_url = "http://127.0.0.1:7100"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.docker.worker_callback_url, "http://127.0.0.1:7100");
    }
}
