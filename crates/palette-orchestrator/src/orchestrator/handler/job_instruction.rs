use crate::perspectives_config::ValidatedPerspectives;
use palette_domain::job::{Job, JobDetail};

/// Container-side mount point for the shared plan directory.
const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Container-side mount point for artifacts.
pub(crate) const ARTIFACTS_MOUNT: &str = "/home/agent/artifacts";

/// Container-side mount point for perspective documents.
const PERSPECTIVE_MOUNT: &str = "/home/agent/perspective";

/// Format a job into an instruction message for a member.
///
/// `round` is included for review jobs so the reviewer knows which round directory to use.
/// `perspectives` provides the server's perspective configuration for including
/// priority paths in the instruction.
pub(crate) fn format_job_instruction(
    job: &Job,
    round: Option<u32>,
    perspectives: &ValidatedPerspectives,
) -> String {
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
    if let Some(perspective_name) = job.detail.perspective() {
        msg.push_str(&format!("\nPerspective: {perspective_name}\n"));
        msg.push_str(&format!("Perspective Documents: {PERSPECTIVE_MOUNT}/\n"));
        if let Some(perspective) = perspectives.find(perspective_name) {
            msg.push_str("Perspective Priority Paths:\n");
            for (i, path) in perspective.paths.iter().enumerate() {
                msg.push_str(&format!("{}. {}\n", i + 1, path.as_config_str()));
            }
        }
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}
