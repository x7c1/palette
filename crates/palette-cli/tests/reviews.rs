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
