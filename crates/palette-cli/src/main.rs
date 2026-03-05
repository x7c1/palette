use anyhow::Context as _;
use palette_core::Config;
use palette_server::AppState;
use palette_tmux::{TmuxManager, TmuxManagerImpl};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config/palette.toml"));

    let config = Config::load(&config_path)?;
    tracing::info!(?config, "loaded config");

    let tmux = TmuxManagerImpl::new(config.tmux.session_name.clone());
    tmux.create_session(&config.tmux.session_name)?;

    let target = tmux
        .create_target("worker")
        .context("failed to create tmux target")?;

    let state = Arc::new(AppState {
        tmux,
        target,
        event_log: tokio::sync::Mutex::new(Vec::new()),
    });

    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
