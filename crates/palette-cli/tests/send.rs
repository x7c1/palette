mod helper;

use helper::{capture_pane, spawn_server, test_session_name_with_guard, wid};
use palette_db::InsertWorkerRequest;
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerRole, WorkerStatus};
use palette_domain::workflow::WorkflowId;
use palette_server::api_types::SendRequest;
use palette_tmux::TmuxManager;
use serde_json::json;

fn register_worker(
    state: &palette_server::AppState,
    id: &str,
    terminal_target: &TerminalTarget,
    status: WorkerStatus,
) {
    let wf_id = WorkflowId::new("wf-test");
    let _ = state.db.create_workflow(&wf_id, "test/blueprint.yaml");
    state
        .db
        .insert_worker(&InsertWorkerRequest {
            id: wid(id),
            workflow_id: wf_id,
            role: WorkerRole::Member,
            status,
            supervisor_id: wid(""),
            container_id: ContainerId::new(""),
            terminal_target: terminal_target.clone(),
            session_id: None,
            task_id: TaskId::new(format!("task-{id}")),
        })
        .unwrap();
}

#[tokio::test]
async fn send_keys_delivers_to_tmux_pane() {
    let (session, _guard) = test_session_name_with_guard("send");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();

    // Register the target in DB
    let (base_url, state) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::Idle);

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            member_id: Some("worker".to_string()),
            target: None,
            message: "echo hello-palette-test".to_string(),
            no_enter: false,
        })
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
}

#[tokio::test]
async fn send_keys_with_direct_target() {
    let (session, _guard) = test_session_name_with_guard("direct");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            member_id: None,
            target: Some(target.to_string()),
            message: "echo direct-test".to_string(),
            no_enter: false,
        })
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
}

#[tokio::test]
async fn send_queues_when_member_is_working() {
    let (session, _guard) = test_session_name_with_guard("queue");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, state) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::Working);

    let client = reqwest::Client::new();

    // Send while Working — should be queued
    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            member_id: Some("worker".to_string()),
            target: None,
            message: "queued message".to_string(),
            no_enter: false,
        })
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
}
