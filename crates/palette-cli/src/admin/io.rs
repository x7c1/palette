use palette_usecase::{AdminCleanupPlan, AdminDeletedCounts};

pub(super) fn print_plan(mode: &str, plan: &AdminCleanupPlan) {
    println!("admin {} plan:", mode);
    println!("  workflows: {}", plan.workflow_ids.len());
    println!("  tasks: {}", plan.task_ids.len());
    println!("  jobs: {}", plan.job_ids.len());
    println!("  workers: {}", plan.worker_ids.len());
    println!("  filesystem targets: {}", plan.file_paths.len());
    for id in &plan.workflow_ids {
        println!("    - {}", id);
    }
}

pub(super) fn print_deleted(deleted: &AdminDeletedCounts, removed_files: usize) {
    println!("deleted:");
    println!("  workflows: {}", deleted.workflows);
    println!("  tasks: {}", deleted.tasks);
    println!("  jobs: {}", deleted.jobs);
    println!("  workers: {}", deleted.workers);
    println!("  review_submissions: {}", deleted.review_submissions);
    println!("  review_comments: {}", deleted.review_comments);
    println!("  message_queue: {}", deleted.message_queue);
    println!("  filesystem entries removed: {}", removed_files);
}
