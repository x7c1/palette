use palette_server::{AppState, create_router};
use palette_tmux::{TmuxManager, TmuxManagerImpl};
use serde_json::json;
use std::process::Command;
use std::sync::Arc;

/// Unique session name for each test to avoid conflicts
fn test_session_name(test_name: &str) -> String {
    format!("palette-test-{}-{}", test_name, std::process::id())
}

/// Spawn the server on a random available port and return (addr, state)
async fn spawn_server(
    tmux: TmuxManagerImpl,
    target: String,
) -> (String, Arc<AppState>) {
    let state = Arc::new(AppState {
        tmux,
        target,
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

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, target).await;

    let client = reqwest::Client::new();

    // POST to /hooks/stop
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

    // Verify event was recorded via /events
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
    assert_eq!(events[0]["payload"]["session_id"], "test-session-123");

    cleanup_session(&session);
}

#[tokio::test]
async fn hooks_notification_records_event() {
    let session = test_session_name("notif");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, target).await;

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
    assert_eq!(events[0]["payload"]["notification_type"], "permission_prompt");

    cleanup_session(&session);
}

#[tokio::test]
async fn send_keys_delivers_to_tmux_pane() {
    let session = test_session_name("send");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, target.clone()).await;

    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/send"))
        .json(&json!({"message": "echo hello-palette-test"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Give tmux a moment to process
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Capture the pane content and verify the message was sent
    let content = capture_pane(&target);
    assert!(
        content.contains("hello-palette-test"),
        "pane content should contain the sent message, got: {content}"
    );

    // Verify the send event was also recorded
    let events: Vec<serde_json::Value> = client
        .get(format!("{base_url}/events"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "send");
    assert_eq!(events[0]["payload"]["message"], "echo hello-palette-test");

    cleanup_session(&session);
}

#[tokio::test]
async fn send_keys_special_characters() {
    let session = test_session_name("special");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, target.clone()).await;

    let client = reqwest::Client::new();

    // Test with special characters: quotes, semicolons, pipes
    let message = r#"echo "hello; world" | cat"#;
    let resp = client
        .post(format!("{base_url}/send"))
        .json(&json!({"message": message}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = capture_pane(&target);
    assert!(
        content.contains("hello; world"),
        "pane should contain special chars, got: {content}"
    );

    cleanup_session(&session);
}

#[tokio::test]
async fn full_flow_send_then_hooks() {
    let session = test_session_name("flow");
    let tmux = TmuxManagerImpl::new(session.clone());
    tmux.create_session(&session).unwrap();

    let target = tmux.create_target("worker").unwrap();
    let (base_url, _state) = spawn_server(tmux, target.clone()).await;

    let client = reqwest::Client::new();

    // 1. Send a command via /send
    client
        .post(format!("{base_url}/send"))
        .json(&json!({"message": "echo flow-test"}))
        .send()
        .await
        .unwrap();

    // 2. Simulate stop hook (as if Claude Code finished responding)
    client
        .post(format!("{base_url}/hooks/stop"))
        .json(&json!({"session_id": "flow-session"}))
        .send()
        .await
        .unwrap();

    // 3. Simulate notification hook (permission prompt)
    client
        .post(format!("{base_url}/hooks/notification"))
        .json(&json!({
            "notification_type": "permission_prompt",
            "tool_name": "Write",
        }))
        .send()
        .await
        .unwrap();

    // 4. Send permission response via /send
    client
        .post(format!("{base_url}/send"))
        .json(&json!({"message": "y"}))
        .send()
        .await
        .unwrap();

    // Verify all events were recorded in order
    let events: Vec<serde_json::Value> = client
        .get(format!("{base_url}/events"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(events.len(), 4);
    assert_eq!(events[0]["event_type"], "send");
    assert_eq!(events[1]["event_type"], "stop");
    assert_eq!(events[2]["event_type"], "notification");
    assert_eq!(events[3]["event_type"], "send");

    cleanup_session(&session);
}
