mod helper;

use helper::{
    create_craft, create_review, spawn_server, test_session_name_with_guard, update_status,
};
use palette_server::api_types::{JobStatus, ReviewCommentInput, SubmitReviewRequest, Verdict};
use palette_tmux::TmuxManager;

#[tokio::test]
async fn review_api_submit_and_get() {
    let (session, _guard) = test_session_name_with_guard("review");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Setup: create craft + review jobs
    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_craft("W-001", "Craft"))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_review("R-001", "Review", vec!["W-001"]))
        .send()
        .await
        .unwrap();

    // Transition craft to in_review
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::Ready))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::InProgress))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::InReview))
        .send()
        .await
        .unwrap();

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

    // W-001 should be reverted to in_progress by rule engine
    let jobs: Vec<serde_json::Value> = client
        .get(format!("{base_url}/jobs?type=craft"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(jobs[0]["status"], "in_progress");

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
async fn full_cycle_craft_review_approved() {
    let (session, _guard) = test_session_name_with_guard("cycle");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create craft + review
    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_craft("W-001", "Craft"))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_review("R-001", "Review", vec!["W-001"]))
        .send()
        .await
        .unwrap();

    // Craft: draft -> ready -> in_progress -> in_review
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::Ready))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::InProgress))
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::InReview))
        .send()
        .await
        .unwrap();

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

    // W-001 should be done
    let jobs: Vec<serde_json::Value> = client
        .get(format!("{base_url}/jobs?type=craft"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(jobs[0]["status"], "done");
}
