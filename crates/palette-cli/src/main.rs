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

    let tmux = TmuxManagerImpl::new(config.tmux.session_name.clone());
    tmux.create_session(&config.tmux.session_name)?;

    let db = Database::open(Path::new(&config.db_path))?;
    tracing::info!(db_path = %config.db_path, "database initialized");

    let rules = RuleEngine::new(config.rules.max_review_rounds);
    let docker = DockerManager::new(config.docker.palette_url.clone());

    let state_path = PathBuf::from(&config.state_path);
    let infra = match PersistentState::load(&state_path)? {
        Some(state) => {
            tracing::info!("restored previous state");
            state
        }
        None => bootstrap_leader(&config, &tmux, &docker, &state_path)?,
    };

    let state = Arc::new(AppState {
        tmux,
        db,
        rules,
        docker,
        docker_config: config.docker,
        infra: tokio::sync::Mutex::new(infra),
        state_path: config.state_path.clone(),
        event_log: tokio::sync::Mutex::new(Vec::new()),
    });

    let app = palette_server::create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(%addr, "starting server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

struct AgentSpec<'a> {
    id: &'a str,
    name: &'a str,
    role: &'a str,
    image: &'a str,
    prompt: &'a str,
    leader_id: &'a str,
}

fn spawn_agent(
    spec: &AgentSpec,
    tmux_target: &str,
    docker: &DockerManager,
    tmux: &TmuxManagerImpl,
    session_name: &str,
    settings_template: &Path,
) -> anyhow::Result<MemberState> {
    let container_id = docker.create_container(spec.name, spec.image, spec.role, session_name)?;
    docker.start_container(&container_id)?;
    docker.write_settings(&container_id, settings_template, spec.id)?;
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
    tmux.send_keys(tmux_target, &cmd)?;
    tracing::info!(name = spec.name, role = spec.role, "launched Claude Code");

    Ok(MemberState {
        id: spec.id.to_string(),
        role: spec.role.to_string(),
        leader_id: spec.leader_id.to_string(),
        container_id,
        tmux_target: tmux_target.to_string(),
        status: MemberStatus::Idle,
        session_id: None,
    })
}

/// Bootstrap only the leader. Members are spawned on-demand by the orchestrator.
fn bootstrap_leader(
    config: &Config,
    tmux: &TmuxManagerImpl,
    docker: &DockerManager,
    state_path: &Path,
) -> anyhow::Result<PersistentState> {
    let session_name = &config.tmux.session_name;
    let settings_template = Path::new(&config.docker.settings_template);
    let mut state = PersistentState::new(session_name.clone());

    let leader_target = tmux
        .create_target("leader")
        .context("failed to create leader tmux target")?;

    let leader = spawn_agent(
        &AgentSpec {
            id: "leader-1",
            name: "leader",
            role: "leader",
            image: &config.docker.leader_image,
            prompt: &config.docker.leader_prompt,
            leader_id: "",
        },
        &leader_target,
        docker,
        tmux,
        session_name,
        settings_template,
    )?;
    state.leaders.push(leader);

    state.save(state_path)?;
    tracing::info!("bootstrapped leader (members spawn on-demand)");
    Ok(state)
}
