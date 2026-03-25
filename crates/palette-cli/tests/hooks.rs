mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_tmux::TmuxManager;
use serde_json::json;

#[tokio::test]
async fn hooks_stop_records_event() {
    let (session, _guard) = test_session_name_with_guard("stop");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let _target = tmux.create_target("worker").unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

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
}

#[tokio::test]
async fn hooks_notification_records_event() {
    let (session, _guard) = test_session_name_with_guard("notif");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let _target = tmux.create_target("worker").unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

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
}
