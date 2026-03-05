use anyhow::Context as _;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub port: u16,
    pub tmux: TmuxConfig,
}

#[derive(Debug, Deserialize)]
pub struct TmuxConfig {
    pub session_name: String,
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
    }
}
