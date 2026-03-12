mod config;

use config::Config;
use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::agent::{AgentId, AgentRole, AgentState, AgentStatus};
use palette_domain::rule::RuleEngine;
use palette_domain::server::PersistentState;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
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
        Some(state) if supervisors_alive(&state, &tmux) => {
            tracing::info!("restored previous state (containers running)");
            state
        }
        Some(stale) => {
            tracing::warn!(
                "previous state found but supervisors are not fully alive, re-bootstrapping"
            );
            cleanup_stale_containers(&stale, &docker);
            bootstrap_supervisors(&config, &tmux, &docker, &state_path)?
        }
        None => bootstrap_supervisors(&config, &tmux, &docker, &state_path)?,
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

struct AgentSpec<'a> {
    id: &'a AgentId,
    name: &'a str,
    role: AgentRole,
    image: &'a str,
    prompt: &'a str,
    supervisor_id: &'a AgentId,
}

fn spawn_agent(
    spec: &AgentSpec,
    terminal_target: &TerminalTarget,
    docker: &DockerManager,
    tmux: &TmuxManager,
    session_name: &str,
    settings_template: &Path,
) -> Result<AgentState, Box<dyn std::error::Error>> {
    let container_id =
        docker.create_container(spec.name, spec.image, spec.role, session_name, None, None)?;
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
        supervisor_id: spec.supervisor_id.clone(),
        container_id,
        terminal_target: terminal_target.clone(),
        status: AgentStatus::Booting,
        session_id: None,
    })
}

/// Check if all supervisor containers and tmux panes from a restored state are still alive.
fn supervisors_alive(state: &PersistentState, tmux: &TmuxManager) -> bool {
    state.supervisors.iter().all(|s| {
        let container_running = palette_docker::is_container_running(s.container_id.as_ref());
        if !container_running {
            tracing::warn!(id = %s.id, container = %s.container_id, "supervisor container not running");
            return false;
        }
        let pane_alive = tmux
            .is_terminal_alive(&s.terminal_target)
            .unwrap_or(false);
        if !pane_alive {
            tracing::warn!(id = %s.id, target = %s.terminal_target, "supervisor tmux pane not found");
        }
        pane_alive
    })
}

/// Stop and remove containers from a stale state before re-bootstrapping.
fn cleanup_stale_containers(state: &PersistentState, docker: &DockerManager) {
    for s in state.supervisors.iter().chain(state.members.iter()) {
        if palette_docker::is_container_running(s.container_id.as_ref()) {
            tracing::info!(id = %s.id, container = %s.container_id, "stopping stale container");
            let _ = docker.stop_container(&s.container_id);
        }
        let _ = docker.remove_container(&s.container_id);
    }
}

/// Bootstrap supervisors (main leader + optional review integrator).
/// Members are spawned on-demand by the orchestrator.
fn bootstrap_supervisors(
    config: &Config,
    tmux: &TmuxManager,
    docker: &DockerManager,
    state_path: &Path,
) -> Result<PersistentState, Box<dyn std::error::Error>> {
    let session_name = &config.tmux.session_name;
    let settings_template = Path::new(&config.docker.settings_template);
    let mut state = PersistentState::new(session_name.clone());
    let empty_supervisor_id = AgentId::new("");

    // Main leader
    let leader_target = tmux.create_target("leader")?;
    let leader_id = AgentId::new("leader-1");
    let leader = spawn_agent(
        &AgentSpec {
            id: &leader_id,
            name: "leader",
            role: AgentRole::Leader,
            image: &config.docker.leader_image,
            prompt: &config.docker.leader_prompt,
            supervisor_id: &empty_supervisor_id,
        },
        &leader_target,
        docker,
        tmux,
        session_name,
        settings_template,
    )?;
    state.supervisors.push(leader);

    // Review integrator
    let ri_target = tmux.create_target("review-integrator")?;
    let ri_id = AgentId::new("review-integrator-1");
    let ri = spawn_agent(
        &AgentSpec {
            id: &ri_id,
            name: "review-integrator",
            role: AgentRole::ReviewIntegrator,
            image: &config.docker.review_integrator_image,
            prompt: &config.docker.review_integrator_prompt,
            supervisor_id: &empty_supervisor_id,
        },
        &ri_target,
        docker,
        tmux,
        session_name,
        settings_template,
    )?;
    state.supervisors.push(ri);

    palette_file_state::save(&state, state_path)?;
    tracing::info!(
        "bootstrapped supervisors: main leader + review integrator (members spawn on-demand)"
    );
    Ok(state)
}
