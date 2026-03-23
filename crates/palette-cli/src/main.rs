mod config;

use config::Config;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::rule::RuleEngine;
use palette_domain::server::PersistentState;
use palette_domain::terminal::TerminalSessionName;
use palette_orchestrator::Orchestrator;
use palette_server::AppState;
use palette_tmux::TmuxManager;
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
    let tmux = Arc::new(TmuxManager::new(session_name.clone()));
    tmux.create_session(&session_name)?;

    let db = Arc::new(Database::open(Path::new(&config.db_path))?);
    tracing::info!(db_path = %config.db_path, "database initialized");

    let docker = DockerManager::new(config.docker.palette_url.clone());

    let state_path = PathBuf::from(&config.state_path);
    let infra = match palette_file_state::load(&state_path)? {
        Some(state) => {
            tracing::info!("restored previous state");
            state
        }
        None => {
            tracing::info!(
                "no previous state, starting fresh (supervisors spawn on workflow start)"
            );
            PersistentState::new(config.tmux.session_name.clone())
        }
    };
    let infra = Arc::new(tokio::sync::Mutex::new(infra));

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState {
        tmux: Arc::clone(&tmux),
        db: Arc::clone(&db),
        rules: RuleEngine::new(Arc::clone(&db), config.rules.max_review_rounds),
        infra: Arc::clone(&infra),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        event_tx,
    });

    // Start orchestrator event loop
    // Ensure plan_dir exists on the host
    std::fs::create_dir_all(&config.plan_dir)?;

    let orchestrator = Arc::new(Orchestrator {
        db: Arc::clone(&db),
        docker,
        docker_config: config.docker,
        plan_dir: config.plan_dir.clone(),
        tmux: Arc::clone(&tmux),
        infra: Arc::clone(&infra),
        state_path: config.state_path.clone(),
    });

    // Resume readiness watchers for agents that were booting when we last shut down
    {
        let infra_guard = infra.lock().await;
        orchestrator.resume_booting_watchers(&infra_guard);
    }

    orchestrator.start(event_rx);

    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
