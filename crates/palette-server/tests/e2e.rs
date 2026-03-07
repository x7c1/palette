use palette_core::DockerConfig;
use palette_core::docker::DockerManager;
use palette_core::state::PersistentState;
use palette_db::{Database, RuleEngine};
use palette_server::{AppState, create_router};
use palette_tmux::{TmuxManager, TmuxManagerImpl};
use serde_json::json;
use std::process::Command;
use std::sync::Arc;

/// Unique session name for each test to avoid conflicts
fn test_session_name(test_name: &str) -> String {
    format!("palette-test-{}-{}", test_name, std::process::id())
}

fn test_docker_config() -> DockerConfig {
    DockerConfig {
        palette_url: "http://127.0.0.1:0".to_string(),
        leader_image: "palette-leader:latest".to_string(),
        member_image: "palette-member:latest".to_string(),
        settings_template: "config/hooks/member-settings.json".to_string(),
        leader_prompt: "prompts/leader.md".to_string(),
        member_prompt: "prompts/member.md".to_string(),
        max_members: 3,
    }
}

/// Spawn the server on an OS-assigned port and return (addr, state)
async fn spawn_server(tmux: TmuxManagerImpl, session_name: &str) -> (String, Arc<AppState>) {
    let db = Database::open_in_memory().unwrap();
    let rules = RuleEngine::new(5);
    let docker = DockerManager::new("http://127.0.0.1:0".to_string());
    let infra = PersistentState::new(session_name.to_string());

    let state = Arc::new(AppState {
        tmux,
        db,
        rules,
        docker,
        docker_config: test_docker_config(),
        infra: tokio::sync::Mutex::new(infra),
        state_path: String::new(),
        event_log: tokio::sync::Mutex::new(Vec::new()),
    });
    let app = create_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state)
}

/// Clean up a tmux session
fn cleanup_session(session: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", session])
        .output();
}

/// Capture the content of a tmux pane
fn capture_pane(target: &str) -> String {
    let output = Command::new("tmux")
        .args(["capture-pane", "-t", target, "-p"])
        .output()
        .expect("failed to capture pane");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[tokio::test]
async fn hooks_stop_records_event() {
    let session = test_session_name("stop");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let _target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    let payload = json!({
        "session_id": "test-session-123",
        "conversation_id": "conv-456"
    });
    let resp = client
        .post(format!("{base_url}/hooks/stop"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events: Vec<serde_json::Value> = client
        .get(format!("{base_url}/events"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "stop");

    cleanup_session(&session);
}

#[tokio::test]
async fn hooks_notification_records_event() {
    let session = test_session_name("notif");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let _target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    let payload = json!({
        "notification_type": "permission_prompt",
        "tool_name": "Bash",
        "tool_input": {"command": "ls"}
    });
    let resp = client
        .post(format!("{base_url}/hooks/notification"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let events: Vec<serde_json::Value> = client
        .get(format!("{base_url}/events"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "notification");

    cleanup_session(&session);
}

#[tokio::test]
async fn send_keys_delivers_to_tmux_pane() {
    let session = test_session_name("send");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();

    // Register the target in infra state
    let (base_url, state) = spawn_server(tmux, &session).await;
    {
        let mut infra = state.infra.lock().await;
        infra.members.push(palette_core::state::MemberState {
            id: "worker".to_string(),
            role: "member".to_string(),
            leader_id: String::new(),
            container_id: String::new(),
            tmux_target: target.clone(),
            status: palette_core::state::MemberStatus::Idle,
            session_id: None,
        });
    }

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&json!({"member_id": "worker", "message": "echo hello-palette-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["queued"], false);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&target);
    assert!(
        content.contains("hello-palette-test"),
        "pane content should contain the sent message, got: {content}"
    );

    cleanup_session(&session);
}

#[tokio::test]
async fn send_keys_with_direct_target() {
    let session = test_session_name("direct");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&json!({"target": target, "message": "echo direct-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&target);
    assert!(
        content.contains("direct-test"),
        "pane should contain the message, got: {content}"
    );

    cleanup_session(&session);
}

#[tokio::test]
async fn task_api_create_and_list() {
    let session = test_session_name("taskapi");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create a work task
    let resp = client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({
            "id": "W-001",
            "type": "work",
            "title": "Implement feature",
            "description": "Details here",
            "assignee": "member-a",
            "priority": "high",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let task: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(task["id"], "W-001");
    assert_eq!(task["status"], "draft");

    // Create a review task depending on W-001
    let resp = client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({
            "id": "R-001",
            "type": "review",
            "title": "Review feature",
            "depends_on": ["W-001"],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let review: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(review["status"], "todo");

    // List all tasks
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks.len(), 2);

    // List work tasks only
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks?type=work"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks.len(), 1);

    cleanup_session(&session);
}

#[tokio::test]
async fn task_api_update_with_rules() {
    let session = test_session_name("taskrules");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create work + review
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "W-001", "type": "work", "title": "Work"}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "R-001", "type": "review", "title": "Review", "depends_on": ["W-001"]}))
        .send()
        .await
        .unwrap();

    // Transition W-001: draft -> ready -> in_progress -> in_review
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "ready"}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_progress"}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_review"}))
        .send()
        .await
        .unwrap();

    // Invalid transition should fail (in_review -> draft)
    let resp = client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "draft"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    cleanup_session(&session);
}

#[tokio::test]
async fn review_api_submit_and_get() {
    let session = test_session_name("review");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Setup: create work + review tasks
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "W-001", "type": "work", "title": "Work"}))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "R-001", "type": "review", "title": "Review", "depends_on": ["W-001"]}))
        .send()
        .await
        .unwrap();

    // Transition work to in_review
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "ready"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_progress"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_review"}))
        .send()
        .await
        .unwrap();

    // Submit review with changes_requested
    let resp = client
        .post(format!("{base_url}/reviews/R-001/submit"))
        .json(&json!({
            "verdict": "changes_requested",
            "summary": "Needs fixes",
            "comments": [
                {"file": "src/main.rs", "line": 10, "body": "Fix this"}
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let sub: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(sub["round"], 1);
    assert_eq!(sub["verdict"], "changes_requested");

    // W-001 should be reverted to in_progress by rule engine
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks?type=work"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks[0]["status"], "in_progress");

    // Get submissions
    let submissions: Vec<serde_json::Value> = client
        .get(format!("{base_url}/reviews/R-001/submissions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(submissions.len(), 1);

    cleanup_session(&session);
}

#[tokio::test]
async fn full_cycle_work_review_approved() {
    let session = test_session_name("cycle");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create work + review
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "W-001", "type": "work", "title": "Work"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "R-001", "type": "review", "title": "Review", "depends_on": ["W-001"]}))
        .send()
        .await
        .unwrap();

    // Work: draft -> ready -> in_progress -> in_review
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "ready"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_progress"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/update"))
        .json(&json!({"id": "W-001", "status": "in_review"}))
        .send()
        .await
        .unwrap();

    // Review: approve
    let resp = client
        .post(format!("{base_url}/reviews/R-001/submit"))
        .json(&json!({"verdict": "approved", "summary": "LGTM"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // W-001 should be done
    let tasks: Vec<serde_json::Value> = client
        .get(format!("{base_url}/tasks?type=work"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(tasks[0]["status"], "done");

    cleanup_session(&session);
}

#[tokio::test]
async fn send_queues_when_member_is_working() {
    let session = test_session_name("queue");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, state) = spawn_server(tmux, &session).await;
    {
        let mut infra = state.infra.lock().await;
        infra.members.push(palette_core::state::MemberState {
            id: "worker".to_string(),
            role: "member".to_string(),
            leader_id: String::new(),
            container_id: String::new(),
            tmux_target: target.clone(),
            status: palette_core::state::MemberStatus::Working,
            session_id: None,
        });
    }

    let client = reqwest::Client::new();

    // Send while Working — should be queued
    let resp = client
        .post(format!("{base_url}/send"))
        .json(&json!({"member_id": "worker", "message": "queued message"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["queued"], true);

    // Stop hook should deliver the queued message
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=worker"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&target);
    assert!(
        content.contains("queued message"),
        "pane should contain the queued message after stop, got: {content}"
    );

    cleanup_session(&session);
}

/// Scenario 3: Multiple members stop while leader is working.
/// Event notifications are queued and delivered one at a time on each leader stop.
#[tokio::test]
async fn scenario3_message_queuing_to_leader() {
    let session = test_session_name("scenario3");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let leader_pane = tmux.create_target("leader").unwrap();
    let _member_a_pane = tmux.create_target("member-a").unwrap();
    let _member_b_pane = tmux.create_target("member-b").unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;
    {
        let mut infra = state.infra.lock().await;
        infra.leaders.push(palette_core::state::MemberState {
            id: "leader-1".to_string(),
            role: "leader".to_string(),
            leader_id: String::new(),
            container_id: String::new(),
            tmux_target: leader_pane.clone(),
            status: palette_core::state::MemberStatus::Working,
            session_id: None,
        });
        infra.members.push(palette_core::state::MemberState {
            id: "member-a".to_string(),
            role: "member".to_string(),
            leader_id: "leader-1".to_string(),
            container_id: String::new(),
            tmux_target: _member_a_pane.clone(),
            status: palette_core::state::MemberStatus::Working,
            session_id: None,
        });
        infra.members.push(palette_core::state::MemberState {
            id: "member-b".to_string(),
            role: "member".to_string(),
            leader_id: "leader-1".to_string(),
            container_id: String::new(),
            tmux_target: _member_b_pane.clone(),
            status: palette_core::state::MemberStatus::Working,
            session_id: None,
        });
    }

    let client = reqwest::Client::new();

    // Create tasks and assign them (simulating auto-assign)
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "W-A", "type": "work", "title": "Task A"}))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/tasks/create"))
        .json(&json!({"id": "W-B", "type": "work", "title": "Task B"}))
        .send()
        .await
        .unwrap();

    // Manually assign tasks (simulating what auto-assign does)
    state.db.update_task_status("W-A", palette_db::TaskStatus::Ready).unwrap();
    state.db.assign_task("W-A", "member-a").unwrap();
    state.db.update_task_status("W-B", palette_db::TaskStatus::Ready).unwrap();
    state.db.assign_task("W-B", "member-b").unwrap();

    // --- Both members stop while leader is Working ---

    // member-a stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-a"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // member-b stops
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=member-b"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Leader is Working, so both notifications should be queued
    assert!(
        state.db.has_pending_messages("leader-1").unwrap(),
        "leader should have pending messages"
    );

    // Leader pane should NOT contain any event yet (leader is Working)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let content = capture_pane(&leader_pane);
    assert!(
        !content.contains("[event]"),
        "leader pane should not have events while Working, got: {content}"
    );

    // --- Leader stops (first time) → first queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=leader-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let content = capture_pane(&leader_pane);
    assert!(
        content.contains("[event] member=member-a type=stop"),
        "first stop should deliver member-a event, got: {content}"
    );

    // Leader should still have pending messages (member-b event)
    assert!(
        state.db.has_pending_messages("leader-1").unwrap(),
        "leader should still have pending message for member-b"
    );

    // --- Leader stops (second time) → second queued message delivered ---
    let resp = client
        .post(format!("{base_url}/hooks/stop?member_id=leader-1"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let content = capture_pane(&leader_pane);
    assert!(
        content.contains("[event] member=member-b type=stop"),
        "second stop should deliver member-b event, got: {content}"
    );

    // Queue should now be empty
    assert!(
        !state.db.has_pending_messages("leader-1").unwrap(),
        "leader queue should be empty after all deliveries"
    );

    cleanup_session(&session);
}
