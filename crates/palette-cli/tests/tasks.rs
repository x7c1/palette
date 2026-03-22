mod helper;

use helper::{
    create_craft, create_review, spawn_server, test_session_name_with_guard, update_status,
};
use palette_db::CreateTaskRequest;
use palette_domain::task::TaskId;
use palette_domain::workflow::WorkflowId;
use palette_server::api_types::{CreateJobRequest, JobStatus, JobType};
use palette_tmux::TmuxManager;

#[tokio::test]
async fn job_api_create_and_list() {
    let (session, _guard) = test_session_name_with_guard("jobapi");
    let tmux = TmuxManager::new(session.clone());
    tmux.create_session(&session).unwrap();

    let (base_url, state) = spawn_server(tmux, &session).await;

    // Set up workflow and tasks for the jobs
    let wf_id = WorkflowId::new("wf-jobapi");
    state
        .db
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();
    state
        .db
        .create_task(&CreateTaskRequest {
            id: TaskId::new("task-W-001"),
            workflow_id: wf_id.clone(),
        })
        .unwrap();
    state
        .db
        .create_task(&CreateTaskRequest {
            id: TaskId::new("task-R-001"),
            workflow_id: wf_id,
        })
        .unwrap();

    let client = reqwest::Client::new();

    // Create a craft job
    let resp = client
        .post(format!("{base_url}/jobs/create"))
        .json(&CreateJobRequest {
            id: Some("W-001".to_string()),
            task_id: "task-W-001".to_string(),
            job_type: JobType::Craft,
            title: "Implement feature".to_string(),
            plan_path: "test/W-001".to_string(),
            description: Some("Details here".to_string()),
            assignee: Some("member-a".to_string()),
            priority: Some(palette_server::api_types::Priority::High),
            repository: None,
        })
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let job: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(job["id"], "W-001");
    assert_eq!(job["status"], "todo");

    // Create a review job
    let resp = client
        .post(format!("{base_url}/jobs/create"))
        .json(&CreateJobRequest {
            id: Some("R-001".to_string()),
            task_id: "task-R-001".to_string(),
            job_type: JobType::Review,
            title: "Review feature".to_string(),
            plan_path: "test/R-001".to_string(),
            description: None,
            assignee: None,
            priority: None,
            repository: None,
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

    let (base_url, state) = spawn_server(tmux, &session).await;

    // Set up workflow and tasks
    let wf_id = WorkflowId::new("wf-jobrules");
    state
        .db
        .create_workflow(&wf_id, "test/blueprint.yaml")
        .unwrap();
    state
        .db
        .create_task(&CreateTaskRequest {
            id: TaskId::new("task-W-001"),
            workflow_id: wf_id.clone(),
        })
        .unwrap();
    state
        .db
        .create_task(&CreateTaskRequest {
            id: TaskId::new("task-R-001"),
            workflow_id: wf_id,
        })
        .unwrap();

    let client = reqwest::Client::new();

    // Create craft + review
    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_craft("W-001", "Craft", "task-W-001"))
        .send()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/jobs/create"))
        .json(&create_review("R-001", "Review", "task-R-001"))
        .send()
        .await
        .unwrap();

    // Transition W-001: todo -> in_progress -> in_review
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

    // Invalid transition should fail (in_review -> todo)
    let resp = client
        .post(format!("{base_url}/jobs/update"))
        .json(&update_status("W-001", JobStatus::Todo))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
