mod helper;

use helper::{spawn_server, test_session_name_with_guard};
use palette_server::api_types::{ReviewCommentInput, SubmitReviewRequest, Verdict};
use palette_tmux::TmuxManager;

#[tokio::test]
async fn review_submit_and_get_submissions() {
    let (session, _guard) = test_session_name_with_guard("review");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;

    // Create a standalone review job (no Job-level depends_on needed in task tree model)
    use palette_domain::agent::AgentId;
    use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType};

    let review_job = state
        .db
        .create_job(&CreateJobRequest {
            task_id: None,
            id: Some(JobId::new("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();
    state
        .db
        .update_job_status(&review_job.id, JobStatus::Todo)
        .unwrap();
    state
        .db
        .assign_job(&review_job.id, &AgentId::new("member-b"))
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
    let review = state.db.get_job(&JobId::new("R-001")).unwrap().unwrap();
    assert_eq!(review.status, JobStatus::Blocked);

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

    let (base_url, state) = spawn_server(tmux, &session).await;

    use palette_domain::agent::AgentId;
    use palette_domain::job::{CreateJobRequest, JobId, JobStatus, JobType};

    let review_job = state
        .db
        .create_job(&CreateJobRequest {
            task_id: None,
            id: Some(JobId::new("R-001")),
            job_type: JobType::Review,
            title: "Review".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec![],
        })
        .unwrap();
    state
        .db
        .update_job_status(&review_job.id, JobStatus::Todo)
        .unwrap();
    state
        .db
        .assign_job(&review_job.id, &AgentId::new("member-b"))
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
    let review = state.db.get_job(&JobId::new("R-001")).unwrap().unwrap();
    assert_eq!(review.status, JobStatus::Done);
}
