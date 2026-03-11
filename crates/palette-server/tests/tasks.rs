mod helper;

use helper::{
    create_craft, create_review, spawn_server, test_session_name_with_guard, update_status,
};
use palette_server::api_types::{CreateJobRequest, JobStatus, JobType};
use palette_tmux::TmuxManager;

#[tokio::test]
async fn job_api_create_and_list() {
    let (session, _guard) = test_session_name_with_guard("jobapi");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, _state) = spawn_server(tmux, &session).await;

    let client = reqwest::Client::new();

    // Create a craft job
    let resp = client
        .post(format!("{base_url}/jobs/create"))
        .json(&CreateJobRequest {
            id: Some("W-001".to_string()),
            job_type: JobType::Craft,
            title: "Implement feature".to_string(),
            description: Some("Details here".to_string()),
            assignee: Some("member-a".to_string()),
            priority: Some(palette_server::api_types::Priority::High),
            repository: None,
            depends_on: vec![],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let job: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(job["id"], "W-001");
    assert_eq!(job["status"], "draft");

    // Create a review job depending on W-001
    let resp = client
        .post(format!("{base_url}/jobs/create"))
        .json(&CreateJobRequest {
            id: Some("R-001".to_string()),
            job_type: JobType::Review,
            title: "Review feature".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
            depends_on: vec!["W-001".to_string()],
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let review: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(review["status"], "todo");

    // List all jobs
    let jobs: Vec<serde_json::Value> = client
        .get(format!("{base_url}/jobs"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(jobs.len(), 2);

    // List craft jobs only
    let jobs: Vec<serde_json::Value> = client
        .get(format!("{base_url}/jobs?type=craft"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
}

#[tokio::test]
async fn job_api_update_with_rules() {
    let (session, _guard) = test_session_name_with_guard("jobrules");
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

    // Transition W-001: draft -> ready -> in_progress -> in_review
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

    // Invalid transition should fail (in_review -> draft)
    let resp = client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::Draft))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
