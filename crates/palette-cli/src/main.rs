use anyhow::Context as _;
use palette_core::Config;
use palette_core::state::{MemberState, MemberStatus, PersistentState};
use palette_db::{Database, RuleEngine};
use palette_server::AppState;
use palette_tmux::{TmuxManager, TmuxManagerImpl};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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

    // Initialize tmux
    let tmux = TmuxManagerImpl::new(config.tmux.session_name.clone());
    tmux.create_session(&config.tmux.session_name)?;

    // Initialize database
    let db = Database::open(Path::new(&config.db_path))?;
    tracing::info!(db_path = %config.db_path, "database initialized");

    // Initialize rule engine
    let rules = RuleEngine::new(config.rules.max_review_rounds);

    // Load or create infrastructure state
    let state_path = PathBuf::from(&config.state_path);
    let infra = match PersistentState::load(&state_path)? {
        Some(state) => {
            tracing::info!("restored previous state");
            state
        }
        None => {
            let mut state = PersistentState::new(config.tmux.session_name.clone());

            // Create leader target
            let leader_target = tmux
                .create_target("leader")
                .context("failed to create leader tmux target")?;
            state.leaders.push(MemberState {
                id: "leader-1".to_string(),
                role: "leader".to_string(),
                leader_id: String::new(),
                container_id: String::new(),
                tmux_target: leader_target,
                status: MemberStatus::Idle,
                session_id: None,
                message_queue: Vec::new(),
            });

            // Create member target
            let member_target = tmux
                .create_target("member-a")
                .context("failed to create member tmux target")?;
            state.members.push(MemberState {
                id: "member-a".to_string(),
                role: "member".to_string(),
                leader_id: "leader-1".to_string(),
                container_id: String::new(),
                tmux_target: member_target,
                status: MemberStatus::Idle,
                session_id: None,
                message_queue: Vec::new(),
            });

            state.save(&state_path)?;
            tracing::info!("created initial state");
            state
        }
    };

    let state = Arc::new(AppState {
        tmux,
        db,
        rules,
        infra: tokio::sync::Mutex::new(infra),
        state_path: config.state_path.clone(),
        event_log: tokio::sync::Mutex::new(Vec::new()),
    });

    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
