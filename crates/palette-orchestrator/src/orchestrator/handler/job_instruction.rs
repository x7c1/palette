use palette_domain::job::{Job, JobDetail};

/// Container-side mount point for the shared plan directory.
const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Container-side mount point for artifacts.
pub(crate) const ARTIFACTS_MOUNT: &str = "/home/agent/artifacts";

/// Format a job into an instruction message for a member.
///
/// `round` is included for review jobs so the reviewer knows which round directory to use.
pub(crate) fn format_job_instruction(job: &Job, round: Option<u32>) -> String {
    let mut msg = format!(
        "## Task: {}\n\nID: {}\nPlan: {}/{}\n",
        job.title, job.id, PLAN_DIR_MOUNT, job.plan_path
    );
    if let JobDetail::Craft { ref repository } = job.detail {
        msg.push_str(&format!(
            "\nRepository: {} (branch: {})\n",
            repository.name, repository.branch
        ));
    }
    if let Some(round) = round {
        msg.push_str(&format!(
            "\nRound: {round}\nArtifacts: {ARTIFACTS_MOUNT}/round-{round}/{}/\n",
            job.id
        ));
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}
