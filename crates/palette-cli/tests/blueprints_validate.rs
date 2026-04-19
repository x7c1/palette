mod helper;

use helper::{spawn_server, test_session_name_with_guard, write_blueprint_file};
use palette_tmux::TmuxManager;

async fn post_validate(base_url: &str, path: &str) -> reqwest::Response {
    let client = reqwest::Client::new();
    client
        .post(format!("{base_url}/blueprints/validate"))
        .json(&serde_json::json!({ "blueprint_path": path }))
        .send()
        .await
        .unwrap()
}

async fn validate_yaml_and_expect_invalid(
    yaml: &str,
    expected_reason: &str,
    session_label: &str,
) -> serde_json::Value {
    let (session, _guard) = test_session_name_with_guard(session_label);
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let fx = write_blueprint_file(yaml);
    let resp = post_validate(&base_url, fx.path().to_str().unwrap()).await;
    assert_eq!(resp.status(), 200, "expected 200 invalid response");
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], false, "expected valid:false, got {body}");
    let reasons: Vec<&str> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["reason"].as_str().unwrap())
        .collect();
    assert!(
        reasons.contains(&expected_reason),
        "expected reason {expected_reason} in {reasons:?}"
    );
    body
}

#[tokio::test]
async fn validate_valid_blueprint_returns_summary() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      plan_path: craft-task/README.md
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    let (session, _guard) = test_session_name_with_guard("bp-valid");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let fx = write_blueprint_file(yaml);
    let resp = post_validate(&base_url, fx.path().to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], true);
    let summary = &body["summary"];
    assert_eq!(summary["root_task_key"], "root");
    assert_eq!(summary["task_count"], 3);
    assert_eq!(summary["craft_count"], 1);
    assert_eq!(summary["review_count"], 1);
    assert_eq!(
        summary["referenced_plans"],
        serde_json::json!(["craft-task/README.md"])
    );
}

#[tokio::test]
async fn validate_rejects_relative_path() {
    let (session, _guard) = test_session_name_with_guard("bp-rel");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let resp = post_validate(&base_url, "relative/blueprint.yaml").await;
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "input_validation_failed");
    assert_eq!(body["errors"][0]["reason"], "blueprint_path/not_absolute");
}

#[tokio::test]
async fn validate_returns_404_for_missing_file() {
    let (session, _guard) = test_session_name_with_guard("bp-404");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let resp = post_validate(&base_url, "/nonexistent/path/blueprint.yaml").await;
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "not_found");
    assert_eq!(body["resource"], "blueprint");
}

#[tokio::test]
async fn validate_yaml_parse_error() {
    let (session, _guard) = test_session_name_with_guard("bp-parse");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Intentionally malformed YAML in a valid blueprint directory.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("blueprint.yaml"), "not yaml: [[[").unwrap();
    std::fs::write(dir.path().join("README.md"), "# x").unwrap();
    let path = dir.path().join("blueprint.yaml");

    let resp = post_validate(&base_url, path.to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], false);
    assert_eq!(body["errors"][0]["reason"], "blueprint/yaml_parse_error");
}

#[tokio::test]
async fn validate_parent_plan_missing() {
    let (session, _guard) = test_session_name_with_guard("bp-parent");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Blueprint file present, README.md absent.
    let dir = tempfile::tempdir().unwrap();
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    std::fs::write(dir.path().join("blueprint.yaml"), yaml).unwrap();
    let path = dir.path().join("blueprint.yaml");

    let resp = post_validate(&base_url, path.to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], false);
    assert_eq!(body["errors"][0]["reason"], "blueprint/parent_plan_missing");
}

#[tokio::test]
async fn validate_plan_path_missing() {
    let (session, _guard) = test_session_name_with_guard("bp-plan");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    // Blueprint references plan_path but the file is missing.
    let dir = tempfile::tempdir().unwrap();
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      plan_path: missing/README.md
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    std::fs::write(dir.path().join("blueprint.yaml"), yaml).unwrap();
    std::fs::write(dir.path().join("README.md"), "# parent").unwrap();
    let path = dir.path().join("blueprint.yaml");

    let resp = post_validate(&base_url, path.to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], false);
    let err = &body["errors"][0];
    assert_eq!(err["reason"], "blueprint/plan_path_missing");
    assert_eq!(err["hint"], "tasks[key=craft-task].plan_path");
}

#[tokio::test]
async fn validate_nested_blueprint() {
    let (session, _guard) = test_session_name_with_guard("bp-nested");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let dir = tempfile::tempdir().unwrap();
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    std::fs::write(dir.path().join("blueprint.yaml"), yaml).unwrap();
    std::fs::write(dir.path().join("README.md"), "# parent").unwrap();
    // Nested blueprint.yaml under a subdirectory
    let nested_dir = dir.path().join("sub");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::write(nested_dir.join("blueprint.yaml"), yaml).unwrap();

    let path = dir.path().join("blueprint.yaml");
    let resp = post_validate(&base_url, path.to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], false);
    assert_eq!(body["errors"][0]["reason"], "blueprint/nested_blueprint");
}

#[tokio::test]
async fn validate_invalid_task_key() {
    let yaml = r#"
task:
  key: INVALID
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "invalid_task_key/invalid_format", "bp-invalid-key")
        .await;
}

#[tokio::test]
async fn validate_missing_review_child() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/missing_review_child", "bp-no-review").await;
}

#[tokio::test]
async fn validate_missing_repository() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/missing_repository", "bp-no-repo").await;
}

#[tokio::test]
async fn validate_invalid_repository() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: ""
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "invalid_repository/branch_empty", "bp-bad-repo").await;
}

#[tokio::test]
async fn validate_self_dependency() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      depends_on: [craft-task]
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/self_dependency", "bp-self-dep").await;
}

#[tokio::test]
async fn validate_duplicate_dependency() {
    let yaml = r#"
task:
  key: root
  children:
    - key: step-a
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
    - key: step-b
      type: craft
      depends_on: [step-a, step-a]
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/duplicate_dependency", "bp-dup-dep").await;
}

#[tokio::test]
async fn validate_perspective_on_non_review() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      perspective: rust-review
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/perspective_on_non_review", "bp-pov-bad")
        .await;
}

#[tokio::test]
async fn validate_unknown_perspective() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
          perspective: nonexistent-perspective
"#;
    validate_yaml_and_expect_invalid(yaml, "blueprint/unknown_perspective", "bp-pov-unknown").await;
}

#[tokio::test]
async fn validate_invalid_pull_request() {
    let yaml = r#"
task:
  key: root
  children:
    - key: review-integrate
      type: review_integrate
      pull_request:
        owner: ""
        repo: palette
        number: 1
      children:
        - key: review
          type: review
"#;
    validate_yaml_and_expect_invalid(yaml, "invalid_pull_request/owner_empty", "bp-bad-pr").await;
}

/// Guard against accidental regressions: calling validate must never write
/// workflow rows. A successful validate followed by `list_workflows` returning
/// zero rows confirms the endpoint is side-effect-free.
#[tokio::test]
async fn validate_is_side_effect_free() {
    let yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      plan_path: craft-task/README.md
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    let (session, _guard) = test_session_name_with_guard("bp-noeffect");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    let fx = write_blueprint_file(yaml);
    let resp = post_validate(&base_url, fx.path().to_str().unwrap()).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["valid"], true);

    let workflows = state.interactor.data_store.list_workflows(None).unwrap();
    assert!(
        workflows.is_empty(),
        "validate must not persist workflows, got: {workflows:?}"
    );
}

/// Confirm that apply-blueprint now returns machine-readable reason codes for
/// YAML parse errors, instead of the old `format!("{cause}")` free-form string.
#[tokio::test]
async fn apply_blueprint_returns_machine_readable_parse_error() {
    let (session, _guard) = test_session_name_with_guard("apply-parse");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();
    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Start a workflow first with a valid blueprint.
    let valid_yaml = r#"
task:
  key: root
  children:
    - key: craft-task
      type: craft
      plan_path: craft-task/README.md
      repository:
        name: x7c1/palette-demo
        branch: main
      children:
        - key: review
          type: review
"#;
    let fx = write_blueprint_file(valid_yaml);
    let start = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({ "blueprint_path": fx.path().to_str().unwrap() }))
        .send()
        .await
        .unwrap();
    assert_eq!(start.status(), 201);
    let body: serde_json::Value = start.json().await.unwrap();
    let wf_id = body["workflow_id"].as_str().unwrap();

    // Corrupt the blueprint file in place to introduce a YAML parse error
    // on the next read. apply-blueprint re-reads from the stored path.
    std::fs::write(fx.path(), "not yaml: [[[").unwrap();

    let resp = client
        .post(format!("{base_url}/workflows/{wf_id}/apply-blueprint"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["code"], "blueprint_invalid");
    let reasons: Vec<&str> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["reason"].as_str().unwrap())
        .collect();
    assert!(
        reasons.contains(&"blueprint/yaml_parse_error"),
        "expected yaml_parse_error in {reasons:?}"
    );
}
