use palette_domain::job::JobDetail;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Container-side mount point for the workspace (target repo clone).
const WORKSPACE_MOUNT: &str = "/home/agent/workspace";

/// Container-side mount point for an externally-mounted Blueprint directory.
pub const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Where the Blueprint and its plans are located, from the worker container's
/// perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLocation {
    /// Blueprint lives in the same git repository the job operates on.
    /// The workspace mount already exposes the Blueprint at the same relative
    /// path, so no additional mount is needed.
    InWorkspace {
        /// Blueprint dir's path relative to the repo's git root.
        blueprint_relative: PathBuf,
    },
    /// Blueprint lives outside any matching repo. The orchestrator mounts the
    /// Blueprint directory separately at `PLAN_DIR_MOUNT`.
    External {
        /// Absolute host path of the Blueprint directory.
        blueprint_host_dir: PathBuf,
    },
}

impl PlanLocation {
    /// Container-side absolute path to a plan, given the plan's relative path
    /// within the Blueprint directory.
    pub fn container_plan_path(&self, plan_relative: &str) -> String {
        match self {
            PlanLocation::InWorkspace { blueprint_relative } => {
                if blueprint_relative.as_os_str().is_empty() {
                    format!("{WORKSPACE_MOUNT}/{plan_relative}")
                } else {
                    format!(
                        "{WORKSPACE_MOUNT}/{}/{plan_relative}",
                        blueprint_relative.display()
                    )
                }
            }
            PlanLocation::External { .. } => {
                format!("{PLAN_DIR_MOUNT}/{plan_relative}")
            }
        }
    }
}

/// Resolve the [`PlanLocation`] for a job, given the workflow's blueprint host
/// path and the job's detail.
///
/// In-repo detection: if the Blueprint directory is inside a git repo whose
/// `origin` URL matches the job's target repository name, the Blueprint is
/// reachable via the workspace mount.
pub fn resolve(blueprint_path: &Path, job_detail: &JobDetail) -> PlanLocation {
    let blueprint_dir = blueprint_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    let abs_blueprint_dir =
        std::fs::canonicalize(&blueprint_dir).unwrap_or_else(|_| blueprint_dir.clone());

    if let Some(repo_name) = repo_name_for(job_detail)
        && let Some(rel) = try_in_repo(&abs_blueprint_dir, &repo_name)
    {
        return PlanLocation::InWorkspace {
            blueprint_relative: rel,
        };
    }

    PlanLocation::External {
        blueprint_host_dir: abs_blueprint_dir,
    }
}

fn repo_name_for(job_detail: &JobDetail) -> Option<String> {
    if let Some(repo) = job_detail.repository() {
        return Some(repo.name.clone());
    }
    if let Some(pr) = job_detail.pull_request() {
        return Some(format!("{}/{}", pr.owner, pr.repo));
    }
    None
}

fn try_in_repo(blueprint_dir: &Path, repo_name: &str) -> Option<PathBuf> {
    let git_root = git_toplevel(blueprint_dir)?;
    let origin = git_origin(&git_root)?;
    if !origin_matches(&origin, repo_name) {
        return None;
    }
    blueprint_dir
        .strip_prefix(&git_root)
        .ok()
        .map(|p| p.to_path_buf())
}

fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if path.is_empty() {
        return None;
    }
    std::fs::canonicalize(&path)
        .ok()
        .or(Some(PathBuf::from(path)))
}

fn git_origin(git_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(git_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Match a git remote URL (HTTPS or SSH form) against an `owner/repo` string.
fn origin_matches(origin: &str, repo_name: &str) -> bool {
    let normalized = origin.trim().trim_end_matches(".git");
    let suffix = format!("/{repo_name}");
    let ssh_suffix = format!(":{repo_name}");
    normalized.ends_with(&suffix) || normalized.ends_with(&ssh_suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn external_container_plan_path() {
        let loc = PlanLocation::External {
            blueprint_host_dir: PathBuf::from("/tmp/bp"),
        };
        assert_eq!(
            loc.container_plan_path("api/README.md"),
            "/home/agent/plans/api/README.md"
        );
    }

    #[test]
    fn in_workspace_container_plan_path_with_relative() {
        let loc = PlanLocation::InWorkspace {
            blueprint_relative: PathBuf::from("docs/plans/2026/0417-foo"),
        };
        assert_eq!(
            loc.container_plan_path("api/README.md"),
            "/home/agent/workspace/docs/plans/2026/0417-foo/api/README.md"
        );
    }

    #[test]
    fn in_workspace_container_plan_path_at_repo_root() {
        let loc = PlanLocation::InWorkspace {
            blueprint_relative: PathBuf::new(),
        };
        assert_eq!(
            loc.container_plan_path("api/README.md"),
            "/home/agent/workspace/api/README.md"
        );
    }

    #[test]
    fn origin_matches_https_with_dot_git() {
        assert!(origin_matches(
            "https://github.com/x7c1/atelier.git",
            "x7c1/atelier"
        ));
    }

    #[test]
    fn origin_matches_https_without_dot_git() {
        assert!(origin_matches(
            "https://github.com/x7c1/atelier",
            "x7c1/atelier"
        ));
    }

    #[test]
    fn origin_matches_ssh() {
        assert!(origin_matches(
            "git@github.com:x7c1/atelier.git",
            "x7c1/atelier"
        ));
    }

    #[test]
    fn origin_does_not_match_different_repo() {
        assert!(!origin_matches(
            "https://github.com/x7c1/palette.git",
            "x7c1/atelier"
        ));
    }

    #[test]
    fn origin_does_not_match_partial_owner() {
        // "1/atelier" suffix-matches "x1/atelier" without the slash boundary —
        // the leading "/" or ":" boundary check prevents this
        assert!(!origin_matches(
            "https://github.com/x1/atelier.git",
            "1/atelier"
        ));
    }
}
