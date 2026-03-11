mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_tmux::TmuxManager;

const BLUEPRINT_YAML: &str = r#"
task:
  id: 2026/feature-x
  title: Add feature X

repositories:
  - name: x7c1/palette
    branch: feature/test

jobs:
  - id: C-A
    type: craft
    title: Implement API
    priority: high

  - id: R-A
    type: review
    title: Review API
    depends_on: [C-A]
"#;

#[tokio::test]
async fn blueprint_submit_stores_and_returns() {
    let (session, _guard) = test_session_name_with_guard("bp-submit");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // POST /blueprints/submit
    let resp = client
        .post(format!("{base_url}/blueprints/submit"))
        .header("Content-Type", "text/plain")
        .body(BLUEPRINT_YAML)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["task_id"], "2026/feature-x");
    assert_eq!(body["title"], "Add feature X");
    assert!(body["yaml"].as_str().unwrap().contains("feature-x"));
}

#[tokio::test]
async fn blueprint_list_returns_all() {
    let (session, _guard) = test_session_name_with_guard("bp-list");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Empty initially
    let blueprints: Vec<serde_json::Value> = client
        .get(format!("{base_url}/blueprints"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(blueprints.is_empty());

    // Submit one
    client
        .post(format!("{base_url}/blueprints/submit"))
        .header("Content-Type", "text/plain")
        .body(BLUEPRINT_YAML)
        .send()
        .await
        .unwrap();

    // Now has one
    let blueprints: Vec<serde_json::Value> = client
        .get(format!("{base_url}/blueprints"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(blueprints.len(), 1);
    assert_eq!(blueprints[0]["task_id"], "2026/feature-x");
}

#[tokio::test]
async fn blueprint_get_by_task_id() {
    let (session, _guard) = test_session_name_with_guard("bp-get");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Submit
    client
        .post(format!("{base_url}/blueprints/submit"))
        .header("Content-Type", "text/plain")
        .body(BLUEPRINT_YAML)
        .send()
        .await
        .unwrap();

    // GET /blueprints/{task_id} (URL-encoded slash)
    let resp = client
        .get(format!("{base_url}/blueprints/{}", "2026%2Ffeature-x"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["task_id"], "2026/feature-x");
    assert_eq!(body["title"], "Add feature X");

    // Not found
    let resp = client
        .get(format!("{base_url}/blueprints/{}", "nonexistent"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn blueprint_load_creates_jobs() {
    let (session, _guard) = test_session_name_with_guard("bp-load");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Submit
    client
        .post(format!("{base_url}/blueprints/submit"))
        .header("Content-Type", "text/plain")
        .body(BLUEPRINT_YAML)
        .send()
        .await
        .unwrap();

    // POST /blueprints/{task_id}/load
    let resp = client
        .post(format!("{base_url}/blueprints/{}/load", "2026%2Ffeature-x"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let jobs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(jobs.len(), 2);

    // Craft job should be ready (auto-transitioned from draft)
    let craft = jobs.iter().find(|j| j["id"] == "C-A").unwrap();
    assert_eq!(craft["type"], "craft");
    assert_eq!(craft["title"], "Implement API");
    assert_eq!(craft["status"], "ready");

    // Review job should be todo
    let review = jobs.iter().find(|j| j["id"] == "R-A").unwrap();
    assert_eq!(review["type"], "review");
    assert_eq!(review["title"], "Review API");
    assert_eq!(review["status"], "todo");

    // GET /jobs should also show them
    let all_jobs: Vec<serde_json::Value> = client
        .get(format!("{base_url}/jobs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all_jobs.len(), 2);

    // Load nonexistent blueprint
    let resp = client
        .post(format!("{base_url}/blueprints/{}/load", "nonexistent"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
