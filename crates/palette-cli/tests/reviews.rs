mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_domain::task::TaskId;
use palette_domain::workflow::WorkflowId;
use palette_server::api_types::{ReviewCommentInput, SubmitReviewRequest, Verdict};
use palette_tmux::TmuxManager;
use palette_usecase::data_store::CreateTaskRequest;

fn setup_review_task(state: &palette_server::AppState, task_id_str: &str) -> TaskId {
    let task_id = TaskId::parse(task_id_str).unwrap();
    let wf_part = task_id_str.split(':').next().unwrap();
    let wf_id = WorkflowId::parse(wf_part).unwrap();
    let _ = state
        .interactor
        .data_store
        .create_workflow(&wf_id, "test/blueprint.yaml");
    let _ = state.interactor.data_store.create_task(&CreateTaskRequest {
        id: task_id.clone(),
        workflow_id: wf_id,
    });
    task_id
}

#[tokio::test]
async fn review_submit_and_get_submissions() {
    let (session, _guard) = test_session_name_with_guard("review");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType, ReviewStatus};
    use palette_domain::worker::WorkerId;

    let task_id = setup_review_task(&state, "wf-review:task-R-001");
    let review_job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            Some(JobId::parse("R-001").unwrap()),
            task_id,
            JobType::Review,
            palette_domain::job::Title::parse("Review").unwrap(),
            palette_domain::job::PlanPath::parse("test/R-001").unwrap(),
            None,
            None,
            None,
            None,
        ))
        .unwrap();
    helper::setup_worker(&*state.interactor.data_store, "member-b");
    state
        .interactor
        .data_store
        .assign_job(
            &review_job.id,
            &WorkerId::parse("member-b").unwrap(),
            JobType::Review,
        )
        .unwrap();

    let client = reqwest::Client::new();

    // Submit review with changes_requested
    let resp = client
        .post(format!("{base_url}/reviews/R-001/submit"))
        .json(&SubmitReviewRequest {
            verdict: Verdict::ChangesRequested,
            summary: Some("Needs fixes".to_string()),
            comments: vec![ReviewCommentInput {
                file: "src/main.rs".to_string(),
                line: 10,
                body: "Fix this".to_string(),
            }],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let sub: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(sub["round"], 1);
    assert_eq!(sub["verdict"], "changes_requested");

    // Review job should be blocked
    let review = state
        .interactor
        .data_store
        .get_job(&JobId::parse("R-001").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        review.status,
        JobStatus::Review(ReviewStatus::ChangesRequested)
    );

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
}

#[tokio::test]
async fn review_approved_completes_review_job() {
    let (session, _guard) = test_session_name_with_guard("cycle");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;

    use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType, ReviewStatus};
    use palette_domain::worker::WorkerId;

    let task_id = setup_review_task(&state, "wf-review:task-R-001");
    let review_job = state
        .interactor
        .data_store
        .create_job(&CreateJobRequest::new(
            Some(JobId::parse("R-001").unwrap()),
            task_id,
            JobType::Review,
            palette_domain::job::Title::parse("Review").unwrap(),
            palette_domain::job::PlanPath::parse("test/R-001").unwrap(),
            None,
            None,
            None,
            None,
        ))
        .unwrap();
    helper::setup_worker(&*state.interactor.data_store, "member-b");
    state
        .interactor
        .data_store
        .assign_job(
            &review_job.id,
            &WorkerId::parse("member-b").unwrap(),
            JobType::Review,
        )
        .unwrap();

    let client = reqwest::Client::new();

    // Review: approve
    let resp = client
        .post(format!("{base_url}/reviews/R-001/submit"))
        .json(&SubmitReviewRequest {
            verdict: Verdict::Approved,
            summary: Some("LGTM".to_string()),
            comments: vec![],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Review job should be done
    let review = state
        .interactor
        .data_store
        .get_job(&JobId::parse("R-001").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(review.status, JobStatus::Review(ReviewStatus::Done));
}

/// Blueprint with composite review: craft → review-integrate → [review-a, review-b]
const COMPOSITE_REVIEW_YAML: &str = r#"
task:
  key: comp
  children:
    - key: craft
      type: craft
      plan_path: test/craft
      children:
        - key: review-integrate
          type: review
          children:
            - key: review-a
              type: review
            - key: review-b
              type: review
"#;

/// Integrator submit is rejected when child reviewer jobs are not Done.
#[tokio::test]
async fn integrator_submit_rejected_when_children_incomplete() {
    let (session, _guard) = test_session_name_with_guard("integ-reject");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state, _shutdown_tx) = spawn_server(tmux, &session).await;
    let client = reqwest::Client::new();

    // Start workflow with composite review blueprint
    let blueprint_file = {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, COMPOSITE_REVIEW_YAML.as_bytes()).unwrap();
        f
    };
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
    let _wf_id = body["workflow_id"].as_str().unwrap();

    use palette_domain::job::{CraftStatus, JobFilter, JobStatus as JStatus, JobType};
    use palette_domain::rule::RuleEffect;
    use palette_domain::server::ServerEvent;
    let wait = || tokio::time::sleep(tokio::time::Duration::from_millis(200));

    // Craft → InReview to trigger review job creation
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

    // Find the integrator job (review-integrate) and child review jobs
    let all_jobs = state
        .interactor
        .data_store
        .list_jobs(&JobFilter::default())
        .unwrap();
    let integrate_job = all_jobs
        .iter()
        .find(|j| {
            j.task_id.as_ref().contains("review-integrate")
                && !j.task_id.as_ref().contains("review-a")
                && !j.task_id.as_ref().contains("review-b")
        })
        .expect("review-integrate job should exist");

    // Setup worker and assign the integrator job
    helper::setup_worker(&*state.interactor.data_store, "integrator-1");
    state
        .interactor
        .data_store
        .assign_job(
            &integrate_job.id,
            &palette_domain::worker::WorkerId::parse("integrator-1").unwrap(),
            JobType::Review,
        )
        .unwrap();

    // Try to submit as integrator — should be rejected because child reviewers are not Done
    let resp = client
        .post(format!("{base_url}/reviews/{}/submit", integrate_job.id))
        .json(&SubmitReviewRequest {
            verdict: Verdict::Approved,
            summary: Some("Approved".to_string()),
            comments: vec![],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "integrator submit should be rejected when child reviewers incomplete"
    );
    let err: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(err["code"], "child_reviewers_incomplete");

    // Verify no submission was recorded
    let submissions = state
        .interactor
        .data_store
        .get_review_submissions(&integrate_job.id)
        .unwrap();
    assert!(
        submissions.is_empty(),
        "no submission should be recorded when rejected"
    );
}
