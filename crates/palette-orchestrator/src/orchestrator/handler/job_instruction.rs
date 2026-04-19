use crate::orchestrator::infra::plan_location::PlanLocation;
use crate::perspectives_config::ValidatedPerspectives;
use palette_domain::job::{Job, JobDetail};

/// Container-side mount point for artifacts.
pub(crate) const ARTIFACTS_MOUNT: &str = "/home/agent/artifacts";

/// Container-side mount point for perspective documents.
const PERSPECTIVE_MOUNT: &str = "/home/agent/perspective";

/// Format a job into an instruction message for a member.
///
/// `round` is included for review jobs so the reviewer knows which round directory to use.
/// `perspectives` provides the server's perspective configuration for including
/// priority paths in the instruction.
/// `plan_loc` is consulted to resolve the absolute container-side path of the
/// job's plan, when the job has one.
pub(crate) fn format_job_instruction(
    job: &Job,
    round: Option<u32>,
    perspectives: &ValidatedPerspectives,
    plan_loc: &PlanLocation,
) -> String {
    let mut msg = format!("## Task: {}\n\nID: {}\n", job.title, job.id);
    if let Some(ref plan_path) = job.plan_path {
        msg.push_str(&format!(
            "Plan: {}\n",
            plan_loc.container_plan_path(plan_path.as_ref())
        ));
    }
    if let JobDetail::Craft { ref repository } = job.detail {
        msg.push_str(&format!(
            "\nRepository: {} (branch: {})\n",
            repository.name, repository.branch
        ));
    }
    if let Some(pr) = job.detail.pull_request() {
        msg.push_str(&format!(
            "\nPull Request: {}\nWorkspace: /home/agent/workspace (read-only checkout of PR branch)\n",
            pr,
        ));
    }
    if let Some(round) = round {
        if matches!(job.detail, JobDetail::ReviewIntegrate { .. }) {
            // ReviewIntegrator writes to round-{N}/ directly (not a subdirectory)
            msg.push_str(&format!(
                "\nRound: {round}\nArtifacts: {ARTIFACTS_MOUNT}/round-{round}/\n",
            ));
        } else {
            // Individual reviewers write to round-{N}/{job_id}/
            msg.push_str(&format!(
                "\nRound: {round}\nArtifacts: {ARTIFACTS_MOUNT}/round-{round}/{}/\n",
                job.id
            ));
        }
    }
    if let Some(perspective_name) = job.detail.perspective() {
        msg.push_str(&format!("\nPerspective: {perspective_name}\n"));
        msg.push_str(&format!("Perspective Documents: {PERSPECTIVE_MOUNT}/\n"));
        if let Some(perspective) = perspectives.find(perspective_name.as_ref()) {
            msg.push_str("Perspective Priority Paths:\n");
            for (i, path) in perspective.paths.iter().enumerate() {
                msg.push_str(&format!(
                    "{}. @{PERSPECTIVE_MOUNT}/{}/{}\n",
                    i + 1,
                    path.dir_name,
                    path.relative_path,
                ));
            }
        }
    }
    msg.push_str("\nPlease begin working on this task.");
    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perspectives_config::{PerspectivePath, ValidatedPerspective};
    use palette_domain::job::{
        JobId, JobStatus, JobType, PerspectiveName, PlanPath, Repository, ReviewTarget, Title,
    };
    use palette_domain::task::TaskId;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn plan_loc() -> PlanLocation {
        PlanLocation::OutsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/bp"),
        }
    }

    fn plan_loc_inside() -> PlanLocation {
        PlanLocation::InsideWorkspace {
            blueprint_host_dir: PathBuf::from("/host/ws/docs/plans/001"),
            blueprint_rel_to_workspace: PathBuf::from("docs/plans/001"),
        }
    }

    fn make_review_job(perspective: Option<PerspectiveName>) -> Job {
        let now = chrono::Utc::now();
        Job {
            id: JobId::parse("R-001").unwrap(),
            task_id: TaskId::parse("wf-test:review-a").unwrap(),
            title: Title::parse("Review API").unwrap(),
            plan_path: Some(PlanPath::parse("plans/api").unwrap()),
            assignee_id: None,
            status: JobStatus::todo(JobType::Review),
            priority: None,
            detail: JobDetail::Review {
                perspective,
                target: ReviewTarget::CraftOutput,
            },
            created_at: now,
            updated_at: now,
            assigned_at: None,
        }
    }

    fn make_craft_job() -> Job {
        let now = chrono::Utc::now();
        Job {
            id: JobId::parse("C-001").unwrap(),
            task_id: TaskId::parse("wf-test:craft-a").unwrap(),
            title: Title::parse("Implement API").unwrap(),
            plan_path: Some(PlanPath::parse("plans/api").unwrap()),
            assignee_id: None,
            status: JobStatus::todo(JobType::Craft),
            priority: None,
            detail: JobDetail::Craft {
                repository: Repository::parse("x7c1/demo", "main", None).unwrap(),
            },
            created_at: now,
            updated_at: now,
            assigned_at: None,
        }
    }

    fn empty_perspectives() -> ValidatedPerspectives {
        ValidatedPerspectives {
            dirs: HashMap::new(),
            perspectives: vec![],
        }
    }

    #[test]
    fn review_without_perspective() {
        let job = make_review_job(None);
        let msg = format_job_instruction(&job, Some(1), &empty_perspectives(), &plan_loc());

        assert!(msg.contains("Plan: /home/agent/plans/plans/api"));
        assert!(msg.contains("Round: 1"));
        assert!(msg.contains("Artifacts: /home/agent/artifacts/round-1/R-001/"));
        assert!(!msg.contains("Perspective"));
    }

    #[test]
    fn review_with_perspective_includes_at_prefixed_paths() {
        let job = make_review_job(Some(PerspectiveName::parse("rust-review").unwrap()));
        let perspectives = ValidatedPerspectives {
            dirs: [("team-docs".to_string(), PathBuf::from("/host/team-docs"))].into(),
            perspectives: vec![ValidatedPerspective {
                name: "rust-review".to_string(),
                paths: vec![
                    PerspectivePath {
                        dir_name: "team-docs".to_string(),
                        relative_path: "compass/axioms".to_string(),
                    },
                    PerspectivePath {
                        dir_name: "team-docs".to_string(),
                        relative_path: "compass/principles".to_string(),
                    },
                ],
            }],
        };

        let msg = format_job_instruction(&job, Some(1), &perspectives, &plan_loc());

        assert!(msg.contains("Perspective: rust-review"));
        assert!(msg.contains("Perspective Documents: /home/agent/perspective/"));
        assert!(msg.contains("1. @/home/agent/perspective/team-docs/compass/axioms"));
        assert!(msg.contains("2. @/home/agent/perspective/team-docs/compass/principles"));
    }

    #[test]
    fn craft_job_includes_repository() {
        let job = make_craft_job();
        let msg = format_job_instruction(&job, None, &empty_perspectives(), &plan_loc());

        assert!(msg.contains("Repository: x7c1/demo (branch: main)"));
        assert!(!msg.contains("Round"));
        assert!(!msg.contains("Perspective"));
    }

    #[test]
    fn inside_workspace_plan_path_uses_workspace_mount() {
        let job = make_craft_job();
        let msg = format_job_instruction(&job, None, &empty_perspectives(), &plan_loc_inside());

        assert!(
            msg.contains("Plan: /home/agent/workspace/docs/plans/001/plans/api"),
            "expected workspace-rooted plan path, got: {msg}"
        );
        assert!(!msg.contains("/home/agent/plans/"));
    }
}
