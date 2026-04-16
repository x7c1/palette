mod helper;

use helper::{capture_pane, simulate_prompt, spawn_server, test_session_name_with_guard, wid};
use palette_domain::task::TaskId;
use palette_domain::terminal::TerminalTarget;
use palette_domain::worker::{ContainerId, WorkerRole, WorkerStatus};
use palette_domain::workflow::WorkflowId;
use palette_server::api_types::{SendPermissionRequest, SendRequest};
use palette_tmux::TmuxManager;
use palette_usecase::InsertWorkerRequest;
use serde_json::json;

fn register_worker(
    state: &palette_server::AppState,
    id: &str,
    terminal_target: &TerminalTarget,
    status: WorkerStatus,
) {
    let wf_id = WorkflowId::parse("wf-test").unwrap();
    let _ = state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml");
    state
        .interactor
        .data_store
        .insert_worker(&InsertWorkerRequest {
            id: wid(id),
            workflow_id: wf_id,
            role: WorkerRole::Member,
            status,
            supervisor_id: None,
            container_id: ContainerId::new("stub"),
            terminal_target: terminal_target.clone(),
            session_id: None,
            task_id: TaskId::parse(format!("wf-test:{id}")).unwrap(),
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
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::Idle);

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            worker_id: Some("worker".to_string()),
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
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            worker_id: None,
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
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::Working);

    let client = reqwest::Client::new();

    // Send while Working — should be queued
    let resp = client
        .post(format!("{base_url}/send"))
        .json(&SendRequest {
            worker_id: Some("worker".to_string()),
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

    // Stop hook should deliver the queued message.
    simulate_prompt(&target);
    let resp = client
        .post(format!("{base_url}/hooks/stop?worker_id=worker"))
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

#[tokio::test]
async fn send_permission_requires_matching_event_id() {
    let (session, _guard) = test_session_name_with_guard("perm-mismatch");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::WaitingPermission);

    state.pending_permission_events.lock().await.insert(
        "worker".to_string(),
        palette_server::PendingPermission {
            event_id: "perm-expected".to_string(),
            created_at: std::time::Instant::now(),
            supervisor_id: None,
            notification: String::new(),
        },
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/send/permission"))
        .json(&SendPermissionRequest {
            worker_id: "worker".to_string(),
            event_id: "perm-wrong".to_string(),
            choice: "2".to_string(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn send_permission_delivers_choice_without_enter() {
    let (session, _guard) = test_session_name_with_guard("perm-ok");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::WaitingPermission);

    state.pending_permission_events.lock().await.insert(
        "worker".to_string(),
        palette_server::PendingPermission {
            event_id: "perm-ok".to_string(),
            created_at: std::time::Instant::now(),
            supervisor_id: None,
            notification: String::new(),
        },
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/send/permission"))
        .json(&SendPermissionRequest {
            worker_id: "worker".to_string(),
            event_id: "perm-ok".to_string(),
            choice: "2".to_string(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let content = capture_pane(&target);
    assert!(
        content.contains("2"),
        "pane should contain permission choice, got: {content}"
    );
}

#[tokio::test]
async fn send_permission_rejects_non_numeric_choice() {
    let (session, _guard) = test_session_name_with_guard("perm-invalid-choice");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    register_worker(&state, "worker", &target, WorkerStatus::WaitingPermission);

    state.pending_permission_events.lock().await.insert(
        "worker".to_string(),
        palette_server::PendingPermission {
            event_id: "perm-choice".to_string(),
            created_at: std::time::Instant::now(),
            supervisor_id: None,
            notification: String::new(),
        },
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/send/permission"))
        .json(&SendPermissionRequest {
            worker_id: "worker".to_string(),
            event_id: "perm-choice".to_string(),
            choice: "yes".to_string(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["errors"][0]["hint"], "choice");
    assert_eq!(
        body["errors"][0]["reason"],
        "send_permission/choice_not_numeric"
    );
}
