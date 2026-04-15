use crate::config::Config;
use crate::interactor_factory::build_interactor;
use palette_domain::server::ServerEvent;
use palette_orchestrator::workspace::WorkspaceManager;
use palette_orchestrator::{Orchestrator, ValidatedPerspectives};
use palette_server::AppState;
use palette_server::permission_timeout::spawn_permission_timeout_checker;
use palette_usecase::Interactor;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const BUNDLED_CONFIG_PATH: &str = "config/palette.toml";
const USER_CONFIG_RELATIVE: &str = ".config/palette/config.toml";

pub async fn run(config_override: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = match config_override {
        Some(p) => PathBuf::from(p),
        None => resolve_config_path()?,
    };
    let config = Config::load(&config_path)?;
    tracing::info!(?config, path = %config_path.display(), "loaded config");

    let validated_perspectives = config.perspectives.validate()?;
    let interactor = build_interactor(&config, &validated_perspectives)?;
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let state = Arc::new(AppState {
        interactor: Arc::clone(&interactor),
        max_review_rounds: config.rules.max_review_rounds,
        data_dir: PathBuf::from("data"),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        pending_permission_events: tokio::sync::Mutex::new(HashMap::new()),
        event_tx: event_tx.clone(),
        shutdown_notify: Arc::clone(&shutdown_notify),
    });

    let orchestrator = build_orchestrator(&config, &interactor, validated_perspectives, event_tx)?;
    orchestrator.clean_orphan_containers();
    orchestrator.resume_booting_watchers();
    orchestrator.recover_from_crash();

    serve(
        orchestrator,
        event_rx,
        state,
        shutdown_notify,
        &config.server_bind_addr,
        &config.operator_api_url,
    )
    .await
}

fn build_orchestrator(
    config: &Config,
    interactor: &Arc<Interactor>,
    perspectives: ValidatedPerspectives,
    event_tx: tokio::sync::mpsc::UnboundedSender<ServerEvent>,
) -> Result<Arc<Orchestrator>, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&config.plan_dir)?;

    Ok(Arc::new(Orchestrator {
        interactor: Arc::clone(interactor),
        docker_config: config.docker.clone(),
        plan_dir: config.plan_dir.clone(),
        session_name: config.tmux.session_name.clone(),
        cancel_token: CancellationToken::new(),
        workspace_manager: WorkspaceManager::new("data"),
        perspectives,
        event_tx,
    }))
}

async fn serve(
    orchestrator: Arc<Orchestrator>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<ServerEvent>,
    state: Arc<AppState>,
    shutdown_notify: Arc<tokio::sync::Notify>,
    bind_addr: &str,
    operator_api_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let orchestrator_handle = orchestrator.start(event_rx, shutdown_rx);

    spawn_permission_timeout_checker(Arc::clone(&state));

    let app = palette_server::create_router(state);
    tracing::info!(%bind_addr, %operator_api_url, "starting server");

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_notify))
        .await?;

    let _ = shutdown_tx.send(());
    let _ = orchestrator_handle.await;

    Ok(())
}

/// Resolve the config path for `palette start`.
///
/// Uses `~/.config/palette/config.toml` as the user config.
/// If it does not exist, copies the bundled default from `config/palette.toml`.
fn resolve_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").map_err(|e| format!("HOME environment variable: {e}"))?;
    let user_config = PathBuf::from(&home).join(USER_CONFIG_RELATIVE);

    if !user_config.exists() {
        let bundled = Path::new(BUNDLED_CONFIG_PATH);
        if bundled.exists() {
            std::fs::copy(bundled, &user_config)?;
            tracing::info!(
                src = %bundled.display(),
                dest = %user_config.display(),
                "copied default config to user config",
            );
        } else {
            return Err(format!(
                "no config found: {} does not exist and bundled default {} is missing",
                user_config.display(),
                bundled.display(),
            )
            .into());
        }
    }

    Ok(user_config)
}

async fn shutdown_signal(api_notify: Arc<tokio::sync::Notify>) {
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
        _ = api_notify.notified() => {
            tracing::info!("received shutdown request via API, initiating graceful shutdown");
        }
    }
}
