use palette_domain::job::JobDetail;
use std::path::{Path, PathBuf};

/// Container-side mount point for the Blueprint directory.
pub const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Host-side location of a Blueprint's directory, resolved for container
/// mounting. The Blueprint is always mounted separately from the workspace so
/// that plan files are available to the worker regardless of whether the
/// Operator has committed them to the target repository's origin yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLocation {
    /// Absolute host path of the Blueprint directory.
    pub blueprint_host_dir: PathBuf,
}

impl PlanLocation {
    /// Container-side absolute path to a plan, given its path relative to the
    /// Blueprint directory.
    pub fn container_plan_path(&self, plan_relative: &str) -> String {
        format!("{PLAN_DIR_MOUNT}/{plan_relative}")
    }
}

/// Resolve the [`PlanLocation`] for a job, given the workflow's blueprint host
/// path. The job detail is currently unused but kept in the signature for
/// callers that may later want to vary the location per job type.
pub fn resolve(
    blueprint_path: &Path,
    _job_detail: &JobDetail,
) -> std::io::Result<PlanLocation> {
    let blueprint_dir = blueprint_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let abs_blueprint_dir = std::fs::canonicalize(&blueprint_dir)?;

    Ok(PlanLocation {
        blueprint_host_dir: abs_blueprint_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_plan_path_under_plan_dir_mount() {
        let loc = PlanLocation {
            blueprint_host_dir: PathBuf::from("/tmp/bp"),
        };
        assert_eq!(
            loc.container_plan_path("api/README.md"),
            "/home/agent/plans/api/README.md"
        );
    }
}
