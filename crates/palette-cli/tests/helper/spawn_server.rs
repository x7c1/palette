use super::StubContainerRuntime;
use palette_db::Database;
use palette_domain::terminal::TerminalSessionName;
use palette_fs::FsBlueprintReader;
use palette_orchestrator::{CallbackNetwork, DockerConfig, Orchestrator};
use palette_server::{AppState, create_router};
use palette_tmux::TmuxManager;
use palette_usecase::Interactor;
use std::sync::Arc;

fn test_docker_config() -> DockerConfig {
    DockerConfig {
        worker_callback_url: "http://127.0.0.1:0".to_string(),
        callback_network: CallbackNetwork::Host,
        approver_image: "palette-leader:latest".to_string(),
        member_image: "palette-member:latest".to_string(),
        settings_template: "config/hooks/worker-settings.json".to_string(),
        approver_prompt: "prompts/approver.md".to_string(),
        review_integrator_image: "palette-leader:latest".to_string(),
        review_integrator_prompt: "prompts/review-integrator.md".to_string(),
        crafter_prompt: "prompts/crafter.md".to_string(),
        reviewer_prompt: "prompts/reviewer.md".to_string(),
        max_workers: 3,
    }
}

/// Spawn the server on an OS-assigned port and return (addr, state, shutdown_tx).
///
/// The caller must keep `_shutdown_tx` alive for the duration of the test;
/// dropping it signals the orchestrator event loop to exit.
pub async fn spawn_server(
    tmux: TmuxManager,
    session_name: &TerminalSessionName,
) -> (String, Arc<AppState>, tokio::sync::oneshot::Sender<()>) {
    let db = Database::open_in_memory().unwrap();

    let interactor = Arc::new(Interactor {
        container: Box::new(StubContainerRuntime),
        terminal: Box::new(tmux),
        data_store: Box::new(db),
        blueprint: Box::new(FsBlueprintReader),
    });

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState {
        interactor: Arc::clone(&interactor),
        max_review_rounds: 5,
        data_dir: std::path::PathBuf::from("data"),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        event_tx: event_tx.clone(),
    });

    let orchestrator = Arc::new(Orchestrator {
        interactor: Arc::clone(&interactor),
        docker_config: test_docker_config(),
        plan_dir: String::new(),
        session_name: session_name.to_string(),
        cancel_token: tokio_util::sync::CancellationToken::new(),
        workspace_manager: palette_orchestrator::workspace::WorkspaceManager::new("data"),
        event_tx,
    });
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let _ = orchestrator.start(event_rx, shutdown_rx);

    let app = create_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state, shutdown_tx)
}
