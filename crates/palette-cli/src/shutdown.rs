use crate::config::Config;
use std::path::PathBuf;

const USER_CONFIG_RELATIVE: &str = ".config/palette/config.toml";

pub async fn run(config_override: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = match config_override {
        Some(p) => PathBuf::from(p),
        None => resolve_config_path()?,
    };
    let config = Config::load(&config_path)?;
    let base_url = &config.operator_api_url;

    let client = reqwest::Client::new();

    // Send shutdown request
    let shutdown_url = format!("{base_url}/shutdown");
    match client.post(&shutdown_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("shutdown request accepted, waiting for Orchestrator to stop");
        }
        Ok(resp) => {
            return Err(format!("unexpected response: {}", resp.status()).into());
        }
        Err(e) if is_connection_refused(&e) => {
            println!("Orchestrator is not running.");
            return Ok(());
        }
        Err(e) => {
            return Err(format!("failed to send shutdown request: {e}").into());
        }
    }

    // Poll /health until connection refused
    let health_url = format!("{base_url}/health");
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        match client.get(&health_url).send().await {
            Ok(_) => continue,
            Err(e) if is_connection_refused(&e) => break,
            Err(_) => break,
        }
    }

    println!("Orchestrator stopped.");
    Ok(())
}

fn resolve_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|e| format!("HOME environment variable: {e}"))?;
    let user_config = PathBuf::from(&home).join(USER_CONFIG_RELATIVE);
    if user_config.exists() {
        Ok(user_config)
    } else {
        Err(format!("config not found: {}", user_config.display()).into())
    }
}

fn is_connection_refused(e: &reqwest::Error) -> bool {
    if e.is_connect() {
        return true;
    }
    let chain = format!("{e:?}");
    chain.contains("Connection refused") || chain.contains("ConnectError")
}
