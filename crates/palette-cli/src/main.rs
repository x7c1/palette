mod config;

use config::Config;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::terminal::TerminalSessionName;
use palette_fs::FsBlueprintReader;
use palette_orchestrator::Orchestrator;
use palette_server::AppState;
use palette_tmux::TmuxManager;
use palette_usecase::Interactor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let session_name = TerminalSessionName::new(&config.tmux.session_name);
    let tmux = TmuxManager::new(session_name.clone());
    tmux.create_session(&session_name)?;

    let db = Database::open(Path::new(&config.db_path))?;
    tracing::info!(db_path = %config.db_path, "database initialized");

    let docker = DockerManager::new(config.docker.palette_url.clone());

    // Assemble the Interactor with concrete implementations
    let interactor = Arc::new(Interactor {
        container: Box::new(docker),
        terminal: Box::new(tmux),
        data_store: Box::new(db),
        blueprint: Box::new(FsBlueprintReader),
    });

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState {
        interactor: Arc::clone(&interactor),
        max_review_rounds: config.rules.max_review_rounds,
        data_dir: std::path::PathBuf::from("data"),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        event_tx: event_tx.clone(),
    });

    // Ensure plan_dir exists on the host
    std::fs::create_dir_all(&config.plan_dir)?;

    let workspace_manager = palette_orchestrator::workspace::WorkspaceManager::new("data");

    let orchestrator = Arc::new(Orchestrator {
        interactor: Arc::clone(&interactor),
        docker_config: config.docker,
        plan_dir: config.plan_dir.clone(),
        session_name: config.tmux.session_name.clone(),
        cancel_token: tokio_util::sync::CancellationToken::new(),
        workspace_manager,
        event_tx,
    });

    // Clean up orphan containers from previous crash/forced exit
    orchestrator.clean_orphan_containers();

    // Resume readiness watchers for workers that were booting when we last shut down
    orchestrator.resume_booting_watchers();

    // Recover from previous Orchestrator crash (health check, message delivery, consistency)
    orchestrator.recover_from_crash();

    // Start orchestrator event loop with shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let orchestrator_handle = orchestrator.start(event_rx, shutdown_rx);

    // Start HTTP server with graceful shutdown
    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Server has stopped; tell the orchestrator to shut down and wait for completion
    let _ = shutdown_tx.send(());
    let _ = orchestrator_handle.await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received SIGINT, initiating graceful shutdown");
        }
        _ = sigterm.recv() => {
            tracing::info!("received SIGTERM, initiating graceful shutdown");
        }
    }
}
