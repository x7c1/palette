use anyhow::Context as _;
use palette_core::Config;
use palette_core::docker::DockerManager;
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

    // Initialize Docker manager
    let docker = DockerManager::new(config.docker.palette_url.clone());
    let session_name = &config.tmux.session_name;

    // Load or create infrastructure state
    let state_path = PathBuf::from(&config.state_path);
    let infra = match PersistentState::load(&state_path)? {
        Some(state) => {
            tracing::info!("restored previous state");
            state
        }
        None => {
            let mut state = PersistentState::new(session_name.clone());

            // --- Leader ---
            let leader_target = tmux
                .create_target("leader")
                .context("failed to create leader tmux target")?;

            let leader_container_id = docker.create_container(
                "leader",
                &config.docker.leader_image,
                "leader",
                session_name,
            )?;
            docker.start_container(&leader_container_id)?;

            // Write settings.json into leader container
            docker.write_settings(
                &leader_container_id,
                Path::new(&config.docker.settings_template),
                "leader-1",
            )?;

            // Copy leader prompt into container
            DockerManager::copy_file_to_container(
                &leader_container_id,
                Path::new(&config.docker.leader_prompt),
                "/home/agent/prompt.md",
            )?;

            state.leaders.push(MemberState {
                id: "leader-1".to_string(),
                role: "leader".to_string(),
                leader_id: String::new(),
                container_id: leader_container_id.clone(),
                tmux_target: leader_target.clone(),
                status: MemberStatus::Idle,
                session_id: None,
                message_queue: Vec::new(),
            });

            // Launch Claude Code in leader's tmux pane
            let leader_cmd =
                DockerManager::claude_exec_command(&leader_container_id, "/home/agent/prompt.md");
            tmux.send_keys(&leader_target, &leader_cmd)?;
            tracing::info!("launched Claude Code in leader container");

            // --- Member ---
            let member_target = tmux
                .create_target("member-a")
                .context("failed to create member tmux target")?;

            let member_container_id = docker.create_container(
                "member-a",
                &config.docker.member_image,
                "member",
                session_name,
            )?;
            docker.start_container(&member_container_id)?;

            // Write settings.json into member container
            docker.write_settings(
                &member_container_id,
                Path::new(&config.docker.settings_template),
                "member-a",
            )?;

            // Copy member prompt into container
            DockerManager::copy_file_to_container(
                &member_container_id,
                Path::new(&config.docker.member_prompt),
                "/home/agent/prompt.md",
            )?;

            state.members.push(MemberState {
                id: "member-a".to_string(),
                role: "member".to_string(),
                leader_id: "leader-1".to_string(),
                container_id: member_container_id.clone(),
                tmux_target: member_target.clone(),
                status: MemberStatus::Idle,
                session_id: None,
                message_queue: Vec::new(),
            });

            // Launch Claude Code in member's tmux pane
            let member_cmd =
                DockerManager::claude_exec_command(&member_container_id, "/home/agent/prompt.md");
            tmux.send_keys(&member_target, &member_cmd)?;
            tracing::info!("launched Claude Code in member container");

            state.save(&state_path)?;
            tracing::info!("created initial state with containers");
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
