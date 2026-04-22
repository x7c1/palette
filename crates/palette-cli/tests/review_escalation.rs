mod helper;

use helper::{
    spawn_server_with_rules, test_session_name_with_guard, tid, wid, write_blueprint_file,
};
use palette_domain::job::{CraftStatus, JobDetail, JobFilter, JobStatus, JobType, ReviewStatus};
use palette_domain::review::{SubmitReviewRequest, Verdict};
use palette_domain::server::ServerEvent;
use palette_domain::task::TaskStatus;
use palette_tmux::TmuxManager;

const CRAFT_REVIEW_YAML: &str = r#"
task:
  key: esc-test
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      repository:
        name: x7c1/palette-demo
        work_branch: main
      children:
        - key: review
          type: review
"#;

/// Drive a workflow up to a live Review Job and return the pieces needed for
/// escalation assertions: (workflow_id_str, craft_job_id, review_job_id).
///
/// The caller owns the `blueprint_path` (via a `BlueprintFixture`) so the
/// on-disk blueprint stays alive for the whole test — the orchestrator keeps
/// reading it as it rebuilds the task tree on each event.
async fn boot_to_review_ready(
    state: &palette_server::AppState,
    client: &reqwest::Client,
    base_url: &str,
    blueprint_path: &std::path::Path,
) -> (
    String,
    palette_domain::job::JobId,
    palette_domain::job::JobId,
) {
    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(300));

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_path.to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = body["workflow_id"].as_str().unwrap().to_string();
    wait().await;

    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let craft_id = jobs
        .iter()
        .find(|j| matches!(j.detail, JobDetail::Craft { .. }))
        .expect("craft job should exist")
        .id
        .clone();

    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JobStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JobStatus::Craft(CraftStatus::InReview))
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::CraftReadyForReview {
        craft_job_id: craft_id.clone(),
    });
    wait().await;

    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_id = all_jobs
        .iter()
        .find(|j| matches!(j.detail, JobDetail::Review { .. }))
        .expect("review job should exist after CraftReadyForReview")
        .id
        .clone();

    state
        .interactor
        .data_store
        .assign_job(&review_id, &wid("reviewer-1"), JobType::Review)
        .unwrap();

    (wf_id, craft_id, review_id)
}

/// With `max_review_rounds = 1`, the very first ChangesRequested verdict raises
/// an Escalation: the Review and parent Craft jobs transition to Escalated and
/// the Craft Task is suspended.
#[tokio::test]
async fn escalation_raised_when_max_rounds_reached() {
    let (session, _guard) = test_session_name_with_guard("esc-max");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server_with_rules(tmux, &session, 1).await;
    let client = reqwest::Client::new();
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");

    let blueprint_file = write_blueprint_file(CRAFT_REVIEW_YAML);
    let (wf_id, craft_id, review_id) =
        boot_to_review_ready(&state, &client, &base_url, blueprint_file.path()).await;

    state
        .interactor
        .data_store
        .submit_review(
            &review_id,
            &SubmitReviewRequest {
                verdict: Verdict::ChangesRequested,
                summary: Some("not yet".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_id.clone(),
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let review = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        review.status,
        JobStatus::Review(ReviewStatus::Escalated),
        "review job should be Escalated when max_review_rounds is reached"
    );

    let craft = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft.status,
        JobStatus::Craft(CraftStatus::Escalated),
        "parent craft job should be Escalated alongside the review"
    );

    let craft_task_state = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "esc-test/craft"))
        .unwrap()
        .expect("craft task state must exist");
    assert_eq!(
        craft_task_state.status,
        TaskStatus::Suspended,
        "craft task should be Suspended when escalation is raised"
    );
}

/// With `max_review_rounds = 3`, a single ChangesRequested verdict must not
/// escalate: the existing review flow reverts the craft job to InProgress so
/// the next craft round can start.
#[tokio::test]
async fn escalation_not_raised_before_max_rounds() {
    let (session, _guard) = test_session_name_with_guard("esc-below");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server_with_rules(tmux, &session, 3).await;
    let client = reqwest::Client::new();
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");

    let blueprint_file = write_blueprint_file(CRAFT_REVIEW_YAML);
    let (wf_id, craft_id, review_id) =
        boot_to_review_ready(&state, &client, &base_url, blueprint_file.path()).await;

    state
        .interactor
        .data_store
        .submit_review(
            &review_id,
            &SubmitReviewRequest {
                verdict: Verdict::ChangesRequested,
                summary: Some("needs changes".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_id.clone(),
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let review = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        review.status,
        JobStatus::Review(ReviewStatus::ChangesRequested),
        "review should enter ChangesRequested, not Escalated"
    );

    let craft = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft.status,
        JobStatus::Craft(CraftStatus::InProgress),
        "craft should revert to InProgress for the next round"
    );

    let craft_task_state = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "esc-test/craft"))
        .unwrap()
        .expect("craft task state must exist");
    assert_ne!(
        craft_task_state.status,
        TaskStatus::Suspended,
        "craft task must not be suspended before max rounds"
    );
}

/// Approve verdicts never raise an Escalation, even when submissions have
/// reached `max_review_rounds`: the usual completion path applies.
#[tokio::test]
async fn approve_at_max_rounds_does_not_escalate() {
    let (session, _guard) = test_session_name_with_guard("esc-approve");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server_with_rules(tmux, &session, 1).await;
    let client = reqwest::Client::new();
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");

    let blueprint_file = write_blueprint_file(CRAFT_REVIEW_YAML);
    let (_wf_id, craft_id, review_id) =
        boot_to_review_ready(&state, &client, &base_url, blueprint_file.path()).await;

    state
        .interactor
        .data_store
        .submit_review(
            &review_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ReviewSubmitted {
        review_job_id: review_id.clone(),
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let review = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        review.status,
        JobStatus::Review(ReviewStatus::Done),
        "approved review should be Done"
    );

    let craft = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_ne!(
        craft.status,
        JobStatus::Craft(CraftStatus::Escalated),
        "craft must not be Escalated when the verdict is Approved"
    );
}
