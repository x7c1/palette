use palette_domain::task::Task;

/// Format a task into an instruction message for a member.
pub(super) fn format_task_instruction(task: &Task) -> String {
    let mut msg = format!("## Task: {}\n\nID: {}\n", task.title, task.id);
    if let Some(ref desc) = task.description {
        msg.push_str(&format!("\n{desc}\n"));
    }
    if let Some(ref repos) = task.repositories {
        msg.push('\n');
        for repo in repos {
            if let Some(ref branch) = repo.branch {
                msg.push_str(&format!("- {} (branch: {branch})\n", repo.name));
            } else {
                msg.push_str(&format!("- {}\n", repo.name));
            }
        }
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}
