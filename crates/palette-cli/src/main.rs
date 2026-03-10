mod config;

use config::Config;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::{
    AgentId, AgentRole, AgentState, AgentStatus, PersistentState, RuleEngine, TerminalSessionName,
    TerminalTarget,
};
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

    let rules = RuleEngine::new(config.rules.max_review_rounds);
    let docker = DockerManager::new(config.docker.palette_url.clone());

    let state_path = PathBuf::from(&config.state_path);
    let infra = match palette_file_state::load(&state_path)? {
        Some(state) => {
            tracing::info!("restored previous state");
            state
        }
        None => bootstrap_leader(&config, &tmux, &docker, &state_path)?,
    };
    let infra = Arc::new(tokio::sync::Mutex::new(infra));

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState {
        tmux: Arc::clone(&tmux),
        db: Arc::clone(&db),
        rules: RuleEngine::new(config.rules.max_review_rounds),
        infra: Arc::clone(&infra),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        event_tx,
    });

    // Start orchestrator event loop
    let orchestrator = Arc::new(Orchestrator {
        db: Arc::clone(&db),
        docker,
        docker_config: config.docker,
        tmux: Arc::clone(&tmux),
        infra: Arc::clone(&infra),
        state_path: config.state_path.clone(),
        rules,
    });

    // Resume readiness watchers for agents that were booting when we last shut down
    {
        let infra_guard = infra.lock().await;
        Orchestrator::resume_booting_watchers(&orchestrator, &infra_guard);
    }

    orchestrator.start(event_rx);

    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

struct AgentSpec<'a> {
    id: &'a AgentId,
    name: &'a str,
    role: AgentRole,
    image: &'a str,
    prompt: &'a str,
    leader_id: &'a AgentId,
}

fn spawn_agent(
    spec: &AgentSpec,
    terminal_target: &TerminalTarget,
    docker: &DockerManager,
    tmux: &TmuxManager,
    session_name: &str,
    settings_template: &Path,
) -> Result<AgentState, Box<dyn std::error::Error>> {
    let container_id = docker.create_container(spec.name, spec.image, spec.role, session_name)?;
    docker.start_container(&container_id)?;
    docker.write_settings(&container_id, settings_template, spec.id.as_ref())?;
    DockerManager::copy_file_to_container(
        &container_id,
        Path::new(spec.prompt),
        "/home/agent/prompt.md",
    )?;
    DockerManager::copy_dir_to_container(
        &container_id,
        Path::new("claude-code-plugin"),
        "/home/agent/claude-code-plugin",
    )?;

    let cmd = DockerManager::claude_exec_command(&container_id, "/home/agent/prompt.md", spec.role);
    tmux.send_keys(terminal_target, &cmd)?;
    tracing::info!(name = spec.name, role = %spec.role, "launched Claude Code");

    Ok(AgentState {
        id: spec.id.clone(),
        role: spec.role,
        leader_id: spec.leader_id.clone(),
        container_id,
        terminal_target: terminal_target.clone(),
        status: AgentStatus::Booting,
        session_id: None,
    })
}

/// Bootstrap only the leader. Members are spawned on-demand by the orchestrator.
fn bootstrap_leader(
    config: &Config,
    tmux: &TmuxManager,
    docker: &DockerManager,
    state_path: &Path,
) -> Result<PersistentState, Box<dyn std::error::Error>> {
    let session_name = &config.tmux.session_name;
    let settings_template = Path::new(&config.docker.settings_template);
    let mut state = PersistentState::new(session_name.clone());

    let leader_target = tmux.create_target("leader")?;

    let leader_id = AgentId::new("leader-1");
    let empty_leader_id = AgentId::new("");
    let leader = spawn_agent(
        &AgentSpec {
            id: &leader_id,
            name: "leader",
            role: AgentRole::Leader,
            image: &config.docker.leader_image,
            prompt: &config.docker.leader_prompt,
            leader_id: &empty_leader_id,
        },
        &leader_target,
        docker,
        tmux,
        session_name,
        settings_template,
    )?;
    state.leaders.push(leader);

    palette_file_state::save(&state, state_path)?;
    tracing::info!("bootstrapped leader (members spawn on-demand)");
    Ok(state)
}
