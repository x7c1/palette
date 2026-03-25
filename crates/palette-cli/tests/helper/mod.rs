use palette_db::Database;
use palette_docker::DockerManager;
use palette_domain::agent::AgentId;
use palette_domain::job::JobId;
use palette_domain::rule::RuleEngine;
use palette_domain::terminal::{TerminalSessionName, TerminalTarget};
use palette_orchestrator::{DockerConfig, Orchestrator};
use palette_server::api_types::{CreateJobRequest, JobStatus, JobType, UpdateJobRequest};
use palette_server::{AppState, create_router};
use palette_tmux::TmuxManager;
use std::process::Command;
use std::sync::Arc;

/// Unique session name for each test to avoid conflicts
pub fn test_session_name(test_name: &str) -> TerminalSessionName {
    TerminalSessionName::new(format!("palette-test-{}-{}", test_name, std::process::id()))
}

/// Create a session name and a guard that cleans up the tmux session on drop.
pub fn test_session_name_with_guard(test_name: &str) -> (TerminalSessionName, SessionGuard) {
    let name = test_session_name(test_name);
    let guard = SessionGuard::new(name.clone());
    (name, guard)
}

pub fn test_docker_config() -> DockerConfig {
    DockerConfig {
        palette_url: "http://127.0.0.1:0".to_string(),
        leader_image: "palette-leader:latest".to_string(),
        member_image: "palette-member:latest".to_string(),
        settings_template: "config/hooks/member-settings.json".to_string(),
        leader_prompt: "prompts/leader.md".to_string(),
        review_integrator_image: "palette-leader:latest".to_string(),
        review_integrator_prompt: "prompts/review-integrator.md".to_string(),
        crafter_prompt: "prompts/crafter.md".to_string(),
        reviewer_prompt: "prompts/reviewer.md".to_string(),
        max_members: 3,
    }
}

pub fn aid(s: &str) -> AgentId {
    AgentId::new(s)
}

pub fn jid(s: &str) -> JobId {
    JobId::new(s)
}

pub fn create_craft(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        id: Some(id.to_string()),
        task_id: task_id.to_string(),
        job_type: JobType::Craft,
        title: title.to_string(),
        plan_path: format!("test/{id}"),
        assignee: None,
        priority: None,
        repository: None,
    }
}

pub fn create_review(id: &str, title: &str, task_id: &str) -> CreateJobRequest {
    CreateJobRequest {
        id: Some(id.to_string()),
        task_id: task_id.to_string(),
        job_type: JobType::Review,
        title: title.to_string(),
        plan_path: format!("test/{id}"),
        assignee: None,
        priority: None,
        repository: None,
    }
}

pub fn update_status(id: &str, status: JobStatus) -> UpdateJobRequest {
    UpdateJobRequest {
        id: id.to_string(),
        status,
    }
}

/// Spawn the server on an OS-assigned port and return (addr, state)
pub async fn spawn_server(
    tmux: TmuxManager,
    session_name: &TerminalSessionName,
) -> (String, Arc<AppState>) {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let tmux = Arc::new(tmux);
    let docker = DockerManager::new("http://127.0.0.1:0".to_string());

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let state = Arc::new(AppState {
        tmux: Arc::clone(&tmux),
        db: Arc::clone(&db),
        rules: RuleEngine::new(Arc::clone(&db), 5),
        event_log: tokio::sync::Mutex::new(Vec::new()),
        event_tx,
    });

    // Start orchestrator event loop
    let orchestrator = Arc::new(Orchestrator {
        db: Arc::clone(&db),
        docker,
        docker_config: test_docker_config(),
        plan_dir: String::new(),
        tmux: Arc::clone(&tmux),
        session_name: session_name.to_string(),
    });
    orchestrator.start(event_rx);

    let app = create_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state)
}

/// RAII guard that kills the tmux session on drop (including panic).
pub struct SessionGuard(TerminalSessionName);

impl SessionGuard {
    pub fn new(session: TerminalSessionName) -> Self {
        Self(session)
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", self.0.as_ref()])
            .output();
    }
}

/// Capture the content of a tmux pane (including scrollback buffer)
pub fn capture_pane(target: &TerminalTarget) -> String {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-t",
            target.as_ref(),
            "-p",
            "-J",
            "-S",
            "-200",
        ])
        .output()
        .expect("failed to capture pane");
    String::from_utf8_lossy(&output.stdout).to_string()
}
