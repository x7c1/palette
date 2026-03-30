mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_tmux::TmuxManager;
use std::io::Write;

use palette_domain::task::TaskId;

/// Write YAML to a temp file and return the path.
fn write_blueprint_file(yaml: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(yaml.as_bytes()).unwrap();
    f
}

/// Build a TaskId from a workflow ID and key path.
fn tid(wf_id: &str, key_path: &str) -> TaskId {
    TaskId::parse(format!("{wf_id}:{key_path}")).unwrap()
}

const TASK_TREE_YAML: &str = r#"
task:
  key: feature-x
  children:
    - key: planning
      children:
        - key: api-plan
          type: craft
          plan_path: 2026/feature-x/planning/api-plan
          children:
            - key: api-plan-review
              type: review

    - key: execution
      depends_on: [planning]
      children:
        - key: api-impl
          type: craft
          plan_path: 2026/feature-x/execution/api-impl
          children:
            - key: api-impl-review
              type: review
"#;

#[tokio::test]
async fn workflow_start_creates_task_tree() {
    let (session, _guard) = test_session_name_with_guard("wf-start");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();
    let blueprint_file = write_blueprint_file(TASK_TREE_YAML);

    // POST /workflows/start
    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = body["workflow_id"].as_str().unwrap();
    assert!(wf_id.starts_with("wf-"));
    // Root + planning + api-plan + api-plan/api-plan-review + execution + api-impl + api-impl/api-impl-review = 7
    assert_eq!(body["task_count"].as_u64().unwrap(), 7);

    use palette_domain::task::TaskStatus;

    let root = state
        .interactor
        .data_store
        .get_task_state(&tid(wf_id, "feature-x"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::InProgress);

    let planning = state
        .interactor
        .data_store
        .get_task_state(&tid(wf_id, "feature-x/planning"))
        .unwrap()
        .unwrap();
    assert_eq!(planning.status, TaskStatus::InProgress);

    let execution = state
        .interactor
        .data_store
        .get_task_state(&tid(wf_id, "feature-x/execution"))
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, TaskStatus::Pending);

    let api_plan = state
        .interactor
        .data_store
        .get_task_state(&tid(wf_id, "feature-x/planning/api-plan"))
        .unwrap()
        .unwrap();
    assert_eq!(api_plan.status, TaskStatus::InProgress);

    let api_plan_review = state
        .interactor
        .data_store
        .get_task_state(&tid(wf_id, "feature-x/planning/api-plan/api-plan-review"))
        .unwrap()
        .unwrap();
    assert_eq!(api_plan_review.status, TaskStatus::Pending);

    // Verify that a Job was created for the craft task (api-plan)
    use palette_domain::job::JobFilter;
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter {
            job_type: None,
            status: None,
            assignee_id: None,
        })
        .unwrap();

    // Only api-plan should have a job (composite craft task with job_type)
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].task_id, tid(wf_id, "feature-x/planning/api-plan"));
    assert_eq!(jobs[0].job_type, palette_domain::job::JobType::Craft);
    assert_eq!(
        jobs[0].status,
        palette_domain::job::JobStatus::Craft(palette_domain::job::CraftStatus::Todo)
    );
}

#[tokio::test]
async fn workflow_start_rejects_invalid_yaml() {
    let (session, _guard) = test_session_name_with_guard("wf-invalid");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();
    let blueprint_file = write_blueprint_file("not valid yaml: [[");

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn workflow_start_rejects_missing_file() {
    let (session, _guard) = test_session_name_with_guard("wf-missing");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": "/nonexistent/path.yaml"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

/// Test the full loop: Job Done → Task Done → sibling Task Ready → new Job
#[tokio::test]
async fn job_completion_cascades_through_task_tree() {
    let (session, _guard) = test_session_name_with_guard("wf-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    let yaml = r#"
task:
  key: cascade-test
  children:
    - key: step-a
      type: craft
      plan_path: test/step-a
      children:
        - key: review
          type: review
    - key: step-b
      type: craft
      plan_path: test/step-b
      depends_on: [step-a]
      children:
        - key: review
          type: review
"#;
    let blueprint_file = write_blueprint_file(yaml);

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let resp_body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = resp_body["workflow_id"].as_str().unwrap();

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::task::TaskStatus;

    // step-a should have a Job in Todo state (craft)
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter {
            job_type: None,
            status: None,
            assignee_id: None,
        })
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].task_id, tid(wf_id, "cascade-test/step-a"));

    // step-b should be Pending (depends on step-a)
    let step_b = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Pending);

    // Simulate Job lifecycle: InProgress → InReview → review approved → Done
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;

    let job_id = &jobs[0].id;
    state
        .interactor
        .data_store
        .update_job_status(job_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(job_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    // CraftReadyForReview creates the review job
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: job_id.clone(),
        }],
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Approve the review
    let all_jobs_a = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_a = all_jobs_a
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "cascade-test/step-a/review").to_string())
        .expect("step-a/review job should exist");

    helper::setup_worker(&*state.interactor.data_store, "reviewer-a");
    state
        .interactor
        .data_store
        .assign_job(&review_a.id, &helper::wid("reviewer-a"), JobType::Review)
        .unwrap();
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_a.id,
            &palette_domain::review::SubmitReviewRequest {
                verdict: palette_domain::review::Verdict::Approved,
                summary: Some("LGTM".into()),
                comments: vec![],
            },
        )
        .unwrap();
    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_a.id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify: step-a task should be Done
    let step_a = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cascade-test/step-a"))
        .unwrap()
        .unwrap();
    assert_eq!(step_a.status, TaskStatus::Completed);

    // Verify: step-b should now be activated (dependency satisfied)
    let step_b = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert!(
        step_b.status == TaskStatus::Ready || step_b.status == TaskStatus::InProgress,
        "step-b should be Ready or InProgress, got: {:?}",
        step_b.status,
    );

    // Find the step-b craft job (auto-created by the orchestrator when step-b activated)
    let all_jobs_b = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let step_b_job = all_jobs_b
        .iter()
        .find(|j| {
            j.task_id.as_ref() == tid(wf_id, "cascade-test/step-b").to_string()
                && j.job_type == JobType::Craft
        })
        .expect("step-b craft job should exist (auto-created)");
    let step_b_job_id = step_b_job.id.clone();

    state
        .interactor
        .data_store
        .update_job_status(&step_b_job_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&step_b_job_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: step_b_job_id.clone(),
        }],
    });
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Approve the review for step-b
    let all_jobs_b2 = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_b = all_jobs_b2
        .iter()
        .find(|j| j.task_id.as_ref() == tid(wf_id, "cascade-test/step-b/review").to_string())
        .expect("step-b/review job should exist");

    helper::setup_worker(&*state.interactor.data_store, "reviewer-b");
    state
        .interactor
        .data_store
        .assign_job(&review_b.id, &helper::wid("reviewer-b"), JobType::Review)
        .unwrap();
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_b.id,
            &palette_domain::review::SubmitReviewRequest {
                verdict: palette_domain::review::Verdict::Approved,
                summary: Some("LGTM".into()),
                comments: vec![],
            },
        )
        .unwrap();
    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_b.id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // step-b task should be Done
    let step_b = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cascade-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Completed);

    // Root task should be Done (all children complete)
    let root = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cascade-test"))
        .unwrap()
        .unwrap();
    assert_eq!(root.status, TaskStatus::Completed);
}

/// Craft InReview should activate child review tasks.
/// The craft task stays InProgress; review tasks become Ready with jobs.
#[tokio::test]
async fn craft_in_review_cascades_to_review_task() {
    let (session, _guard) = test_session_name_with_guard("wf-ir-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Blueprint: craft with review child; step-b depends on craft
    let yaml = r#"
task:
  key: ir-test
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      children:
        - key: review
          type: review
    - key: step-b
      depends_on: [craft]
      children:
        - key: craft
          type: craft
          plan_path: test/step-b-craft
          children:
            - key: review
              type: review
"#;
    let blueprint_file = write_blueprint_file(yaml);

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let resp_body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = resp_body["workflow_id"].as_str().unwrap();

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, ReviewStatus};
    use palette_domain::task::TaskStatus;

    // craft task should have a Job in Todo state (composite craft with children)
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    assert_eq!(jobs.len(), 1, "only craft job should exist");
    let craft_job_id = &jobs[0].id;

    // craft/review should be Pending (not activated until craft job reaches InReview)
    let review_task = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "ir-test/craft/review"))
        .unwrap()
        .unwrap();
    assert_eq!(review_task.status, TaskStatus::Pending);

    // Simulate craft job reaching InReview (member stop hook path)
    state
        .interactor
        .data_store
        .update_job_status(craft_job_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(craft_job_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    // Send StatusChanged(InReview) — this is what the stop hook sends
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_job_id.clone(),
        }],
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Verify: craft task should still be InProgress (not completed yet, has children)
    let craft_task = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "ir-test/craft"))
        .unwrap()
        .unwrap();
    assert_eq!(craft_task.status, TaskStatus::InProgress);

    // Verify: craft/review task should now be Ready (activated by InReview)
    let review_task = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "ir-test/craft/review"))
        .unwrap()
        .unwrap();
    assert_eq!(review_task.status, TaskStatus::Ready);

    // Verify: a review Job was created for craft/review
    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_jobs: Vec<_> = all_jobs
        .iter()
        .filter(|j| j.task_id.to_string() == tid(&wf_id, "ir-test/craft/review").to_string())
        .collect();
    assert_eq!(review_jobs.len(), 1, "review job should be created");
    assert_eq!(
        review_jobs[0].status,
        JStatus::Review(ReviewStatus::Todo),
        "review job should be in Todo status for AutoAssign"
    );

    // step-b should still be Pending (craft task is not completed yet)
    let step_b = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "ir-test/step-b"))
        .unwrap()
        .unwrap();
    assert_eq!(step_b.status, TaskStatus::Pending);
}

/// Full task tree cascade: craft InReview → review child activated → review Done →
/// craft job Done → craft task Completed → sibling step-b Ready → workflow complete.
#[tokio::test]
async fn full_task_tree_cascade_with_review() {
    let (session, _guard) = test_session_name_with_guard("wf-full-cascade");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Insert workers needed for assign_job FK constraints
    helper::setup_worker(&*state.interactor.data_store, "reviewer-a");
    helper::setup_worker(&*state.interactor.data_store, "reviewer-b");

    let yaml = r#"
task:
  key: full
  children:
    - key: step-a
      type: craft
      plan_path: test/a-craft
      children:
        - key: review
          type: review
    - key: step-b
      depends_on: [step-a]
      type: craft
      plan_path: test/b-craft
      children:
        - key: review
          type: review
"#;
    let blueprint_file = write_blueprint_file(yaml);

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let resp_body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = resp_body["workflow_id"].as_str().unwrap();
    let workflow_id = palette_domain::workflow::WorkflowId::parse(wf_id).unwrap();

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    use palette_domain::task::TaskStatus;
    use palette_domain::workflow::WorkflowStatus;

    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));

    // --- Phase 1: step-a craft InReview ---
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    assert_eq!(jobs.len(), 1);
    let craft_a_id = jobs[0].id.clone();

    // step-a is InProgress (composite craft with review child, job created)
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::InProgress
    );

    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_a_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_a_id.clone(),
        }],
    });
    wait().await;

    // step-a stays InProgress (craft job is InReview, review child activated)
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::InProgress
    );

    // step-a/review should be Ready with a review Job
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-a/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Ready
    );

    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_a_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(&wf_id, "full/step-a/review").to_string())
        .expect("review job should exist");
    let review_a_id = review_a_job.id.clone();

    // step-b still Pending
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Pending
    );

    // --- Phase 2: step-a review Done (approved) ---
    state
        .interactor
        .data_store
        .assign_job(
            &review_a_id,
            &palette_domain::worker::WorkerId::parse("reviewer-a").unwrap(),
            JobType::Review,
        )
        .unwrap();

    use palette_domain::review::{SubmitReviewRequest, Verdict};
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_a_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let engine = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    );
    let effects = engine.on_review_submitted(&review_a_id, &sub).unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // step-a/review task should be Completed
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-a/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // step-a (craft task) should be Completed (review child done + own job Done)
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-a"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // step-b should now be InProgress (composite craft, dependency satisfied)
    let step_b_status = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "full/step-b"))
        .unwrap()
        .unwrap()
        .status;
    assert_eq!(step_b_status, TaskStatus::InProgress);

    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let craft_b_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(&wf_id, "full/step-b").to_string())
        .expect("step-b craft job should exist");
    let craft_b_id = craft_b_job.id.clone();

    // --- Phase 3: step-b craft InReview → review → Done → workflow complete ---
    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_b_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_b_id.clone(),
        }],
    });
    wait().await;

    // step-b/review should be Ready with a Job
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-b/review"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Ready
    );

    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let review_b_job = all_jobs
        .iter()
        .find(|j| j.task_id.as_ref() == tid(&wf_id, "full/step-b/review").to_string())
        .expect("step-b review job should exist");
    let review_b_id = review_b_job.id.clone();

    // Approve step-b review
    state
        .interactor
        .data_store
        .assign_job(
            &review_b_id,
            &palette_domain::worker::WorkerId::parse("reviewer-b").unwrap(),
            JobType::Review,
        )
        .unwrap();

    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_b_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let effects = engine.on_review_submitted(&review_b_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // step-b task (craft) should be Completed
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full/step-b"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // Root task Completed
    assert_eq!(
        state
            .interactor
            .data_store
            .get_task_state(&tid(&wf_id, "full"))
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // Workflow completed
    let wf = state
        .interactor
        .data_store
        .get_workflow(&workflow_id)
        .unwrap()
        .unwrap();
    assert_eq!(wf.status, WorkflowStatus::Completed);
}

/// Craft job must NOT be marked Done until ALL child review jobs are Done.
#[tokio::test]
async fn craft_waits_for_all_reviews_before_done() {
    let (session, _guard) = test_session_name_with_guard("wf-multi-review");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Insert workers needed for assign_job FK constraints
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");
    helper::setup_worker(&*state.interactor.data_store, "reviewer-2");

    // Blueprint: craft with two review children
    let yaml = r#"
task:
  key: multi-review
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      children:
        - key: review-1
          type: review
        - key: review-2
          type: review
"#;
    let blueprint_file = write_blueprint_file(yaml);

    let resp = client
        .post(format!("{base_url}/workflows/start"))
        .json(&serde_json::json!({
            "blueprint_path": blueprint_file.path().to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let resp_body: serde_json::Value = resp.json().await.unwrap();
    let wf_id = resp_body["workflow_id"].as_str().unwrap();

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));

    // Craft → InReview
    let jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let craft_id = jobs[0].id.clone();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();

    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_id.clone(),
        }],
    });
    wait().await;

    // Both review jobs should be created (review-1 and review-2 as children of craft)
    let review_prefix = format!("{wf_id}:multi-review/craft/review");
    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let mut review_jobs: Vec<_> = all_jobs
        .iter()
        .filter(|j| j.task_id.as_ref().starts_with(&review_prefix))
        .collect();
    review_jobs.sort_by_key(|j| j.task_id.to_string());
    assert_eq!(review_jobs.len(), 2, "both review jobs should be created");

    let review_1_id = review_jobs[0].id.clone();
    let review_2_id = review_jobs[1].id.clone();

    // Approve only review-1
    state
        .interactor
        .data_store
        .assign_job(
            &review_1_id,
            &palette_domain::worker::WorkerId::parse("reviewer-1").unwrap(),
            JobType::Review,
        )
        .unwrap();

    use palette_domain::review::{SubmitReviewRequest, Verdict};
    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_1_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let engine = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    );
    let effects = engine.on_review_submitted(&review_1_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Craft job should still be InReview (not Done) because review-2 is not yet approved
    let craft_job = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::InReview),
        "craft job must stay InReview until all reviews are Done"
    );

    // Now approve review-2
    state
        .interactor
        .data_store
        .assign_job(
            &review_2_id,
            &palette_domain::worker::WorkerId::parse("reviewer-2").unwrap(),
            JobType::Review,
        )
        .unwrap();

    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_2_id,
            &SubmitReviewRequest {
                verdict: Verdict::Approved,
                summary: Some("LGTM".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let effects = engine.on_review_submitted(&review_2_id, &sub).unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // NOW craft job should be Done
    let craft_job = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::Done),
        "craft job should be Done after all reviews approved"
    );
}

/// Test the full changes_requested flow:
/// 1. Craft job reaches InReview → review task activated
/// 2. Reviewer submits changes_requested → craft job reverts to InProgress
/// 3. Crafter fixes and goes back to InReview → review job reactivated
/// 4. Reviewer approves → craft job Done → craft task Completed
#[tokio::test]
async fn changes_requested_flow() {
    use palette_domain::job::{CraftStatus, JobStatus as JStatus, JobType, ReviewStatus};
    use palette_domain::review::{SubmitReviewRequest, Verdict};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    use palette_domain::task::TaskStatus;

    let yaml = r#"
task:
  key: cr-test
  children:
    - key: impl
      type: craft
      plan_path: test/impl
      children:
        - key: review
          type: review
"#;
    let file = write_blueprint_file(yaml);

    let (session_name, _guard) = test_session_name_with_guard("cr-flow");
    let tmux = TmuxManager::new(session_name.clone());
    tmux.create_session(&session_name).unwrap();
    let (addr, state, _shutdown_tx) = spawn_server(tmux, &session_name).await;

    // Insert worker needed for assign_job FK constraint
    helper::setup_worker(&*state.interactor.data_store, "reviewer-1");

    // Start workflow
    let client = reqwest::Client::new();
    let resp_body: serde_json::Value = client
        .post(format!("{addr}/workflows/start"))
        .json(&serde_json::json!({ "blueprint_path": file.path().to_str().unwrap() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let wf_id = resp_body["workflow_id"].as_str().unwrap();

    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));
    wait().await;

    // Get craft job
    let craft_job = state
        .interactor
        .data_store
        .list_jobs(&palette_domain::job::JobFilter {
            job_type: Some(palette_domain::job::JobType::Craft),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(craft_job.len(), 1);
    let craft_id = craft_job[0].id.clone();

    // Simulate craft: Todo → InProgress → InReview
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InProgress))
        .unwrap();
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_id.clone(),
        }],
    });
    wait().await;

    // Review job should be created
    let review_jobs = state
        .interactor
        .data_store
        .list_jobs(&palette_domain::job::JobFilter {
            job_type: Some(palette_domain::job::JobType::Review),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(review_jobs.len(), 1, "review job should be created");
    let review_id = review_jobs[0].id.clone();

    // Assign reviewer and submit changes_requested
    state
        .interactor
        .data_store
        .assign_job(
            &review_id,
            &palette_domain::worker::WorkerId::parse("reviewer-1").unwrap(),
            JobType::Review,
        )
        .unwrap();

    let sub = state
        .interactor
        .data_store
        .submit_review(
            &review_id,
            &SubmitReviewRequest {
                verdict: Verdict::ChangesRequested,
                summary: Some("Please fix X".to_string()),
                comments: vec![],
            },
        )
        .unwrap();

    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_id, &sub)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Verify: review job should be ChangesRequested
    let review_job = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        review_job.status,
        JStatus::Review(ReviewStatus::ChangesRequested)
    );

    // Verify: craft job should be back to InProgress
    let craft_job = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::InProgress),
        "craft job should revert to InProgress after changes_requested"
    );

    // Crafter fixes and goes back to InReview
    state
        .interactor
        .data_store
        .update_job_status(&craft_id, JStatus::Craft(CraftStatus::InReview))
        .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects {
        effects: vec![RuleEffect::CraftReadyForReview {
            craft_job_id: craft_id.clone(),
        }],
    });
    wait().await;

    // Verify: review job should be reactivated to InProgress
    let review_job = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        review_job.status,
        JStatus::Review(ReviewStatus::InProgress),
        "review job should be reactivated for re-review"
    );

    // Reviewer approves this time
    let sub2 = state
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

    let effects = palette_usecase::RuleEngine::new(
        state.interactor.data_store.as_ref(),
        state.max_review_rounds,
    )
    .on_review_submitted(&review_id, &sub2)
    .unwrap();
    let _ = state.event_tx.send(ServerEvent::ProcessEffects { effects });
    wait().await;

    // Verify: review job Done, craft job Done, craft task Completed
    let review_job = state
        .interactor
        .data_store
        .get_job(&review_id)
        .unwrap()
        .unwrap();
    assert_eq!(review_job.status, JStatus::Review(ReviewStatus::Done));

    let craft_job = state
        .interactor
        .data_store
        .get_job(&craft_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        craft_job.status,
        JStatus::Craft(CraftStatus::Done),
        "craft job should be Done after approval"
    );

    let craft_task = state
        .interactor
        .data_store
        .get_task_state(&tid(&wf_id, "cr-test/impl"))
        .unwrap()
        .unwrap();
    assert_eq!(
        craft_task.status,
        TaskStatus::Completed,
        "craft task should be Completed after job Done + review child Completed"
    );
}
