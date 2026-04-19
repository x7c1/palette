use std::path::{Path, PathBuf};
use std::process::Command;

/// Container-side mount point for a Blueprint directory that is mounted
/// separately from the workspace (Repo-outside-Plan mode).
pub const PLAN_DIR_MOUNT: &str = "/home/agent/plans";

/// Container-side mount point for the workspace.
const WORKSPACE_MOUNT: &str = "/home/agent/workspace";

/// Where and how the Blueprint is reachable from inside a worker container.
///
/// The orchestrator decides between two modes at workspace-creation time:
///
/// - `InsideWorkspace`: the Blueprint directory lives under the workspace root,
///   so the Plan can be read via the workspace mount and no separate plan
///   mount is required.
/// - `OutsideWorkspace`: the Blueprint lives outside any workspace (typical
///   for Repo-outside-Plan setups like `atelier → palette`), so the Blueprint
///   directory is mounted read-only at [`PLAN_DIR_MOUNT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanLocation {
    InsideWorkspace {
        /// Absolute host path of the Blueprint directory.
        blueprint_host_dir: PathBuf,
        /// Blueprint directory relative to the workspace root (empty when
        /// the Blueprint sits directly at the workspace root).
        blueprint_rel_to_workspace: PathBuf,
    },
    OutsideWorkspace {
        /// Absolute host path of the Blueprint directory.
        blueprint_host_dir: PathBuf,
    },
}

impl PlanLocation {
    /// Container-side absolute path to a plan, given its path relative to the
    /// Blueprint directory.
    pub fn container_plan_path(&self, plan_relative: &str) -> String {
        match self {
            Self::InsideWorkspace {
                blueprint_rel_to_workspace,
                ..
            } => {
                let rel = blueprint_rel_to_workspace.to_string_lossy();
                if rel.is_empty() {
                    format!("{WORKSPACE_MOUNT}/{plan_relative}")
                } else {
                    format!("{WORKSPACE_MOUNT}/{rel}/{plan_relative}")
                }
            }
            Self::OutsideWorkspace { .. } => {
                format!("{PLAN_DIR_MOUNT}/{plan_relative}")
            }
        }
    }

    /// Host-side Blueprint directory that must be bind-mounted into the
    /// container. Returns `None` for `InsideWorkspace`, where the workspace
    /// mount already carries the Blueprint.
    pub fn plan_dir_host_path(&self) -> Option<&Path> {
        match self {
            Self::InsideWorkspace { .. } => None,
            Self::OutsideWorkspace { blueprint_host_dir } => Some(blueprint_host_dir),
        }
    }
}

/// Resolve the [`PlanLocation`] for a job given the Blueprint file path and,
/// when available, the absolute host path of the workspace the job will run
/// in.
///
/// Two detection strategies are tried in order:
///
/// 1. **Direct containment**: the Blueprint directory already lives under the
///    workspace path on disk. Primarily hit by test harnesses that pre-stage
///    the workspace layout.
/// 2. **Git-root equivalence**: the Blueprint lives in a host clone of the
///    same repository that the workspace was cloned from. Both clones share
///    the same `remote.origin.url`, so the Blueprint's path relative to its
///    host git toplevel is also its path inside the workspace. This is the
///    normal production path: Operators author Blueprints under their own
///    clone of the target repo (e.g., `~/repos/palette/docs/plans/...`)
///    rather than inside the data-directory workspace.
///
/// If neither strategy matches, the Blueprint is external and must be mounted
/// separately at [`PLAN_DIR_MOUNT`].
pub fn resolve(
    blueprint_path: &Path,
    workspace_host_path: Option<&Path>,
) -> std::io::Result<PlanLocation> {
    let blueprint_dir = blueprint_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let abs_blueprint_dir = std::fs::canonicalize(&blueprint_dir)?;

    if let Some(ws) = workspace_host_path {
        let abs_ws = std::fs::canonicalize(ws)?;

        if let Ok(rel) = abs_blueprint_dir.strip_prefix(&abs_ws) {
            return Ok(PlanLocation::InsideWorkspace {
                blueprint_host_dir: abs_blueprint_dir.clone(),
                blueprint_rel_to_workspace: rel.to_path_buf(),
            });
        }

        if let Some(rel) = resolve_via_git_root(&abs_blueprint_dir, &abs_ws)? {
            return Ok(PlanLocation::InsideWorkspace {
                blueprint_host_dir: abs_ws.join(&rel),
                blueprint_rel_to_workspace: rel,
            });
        }
    }

    Ok(PlanLocation::OutsideWorkspace {
        blueprint_host_dir: abs_blueprint_dir,
    })
}

/// Map the host-side Blueprint directory into its equivalent path inside the
/// workspace clone by matching `remote.origin.url` on both sides.
///
/// Returns `None` when either side is not a git working tree, the origins
/// differ, the Blueprint sits at the git root (copying the entire working
/// tree would not make sense), or a git command fails.
fn resolve_via_git_root(
    abs_blueprint_dir: &Path,
    abs_ws: &Path,
) -> std::io::Result<Option<PathBuf>> {
    let Some(host_top) = git_toplevel(abs_blueprint_dir)? else {
        return Ok(None);
    };
    let Some(ws_top) = git_toplevel(abs_ws)? else {
        return Ok(None);
    };

    let host_origin = git_origin_url(&host_top)?;
    let ws_origin = git_origin_url(&ws_top)?;
    let matches = match (host_origin, ws_origin) {
        (Some(h), Some(w)) => origin_urls_match(&h, &w),
        _ => false,
    };
    if !matches {
        return Ok(None);
    }

    let Ok(rel) = abs_blueprint_dir.strip_prefix(&host_top) else {
        return Ok(None);
    };
    if rel.as_os_str().is_empty() {
        return Ok(None);
    }
    Ok(Some(rel.to_path_buf()))
}

fn git_toplevel(dir: &Path) -> std::io::Result<Option<PathBuf>> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if line.is_empty() {
        return Ok(None);
    }
    std::fs::canonicalize(PathBuf::from(line)).map(Some)
}

fn git_origin_url(dir: &Path) -> std::io::Result<Option<String>> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        Ok(None)
    } else {
        Ok(Some(url))
    }
}

/// Decide whether two `remote.origin.url` values refer to the same repository.
///
/// Workspaces are always cloned from a local bare cache, so their
/// `remote.origin.url` is a filesystem path like
/// `<data_dir>/repos/<owner>/<repo>.git`. Host-side clones usually point at
/// the upstream URL (`https://github.com/<owner>/<repo>.git`,
/// `git@github.com:<owner>/<repo>.git`, etc.). The matcher therefore extracts
/// the trailing `<owner>/<repo>` segment from each URL and compares those —
/// equivalent paths like GitHub, a local bare, and the workspace's own cache
/// path all normalise to the same `<owner>/<repo>` string.
fn origin_urls_match(a: &str, b: &str) -> bool {
    canonicalize_origin(a) == canonicalize_origin(b)
}

fn canonicalize_origin(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let segs: Vec<&str> = trimmed
        .split(['/', ':'])
        .filter(|s| !s.is_empty())
        .collect();
    if segs.len() >= 2 {
        format!("{}/{}", segs[segs.len() - 2], segs[segs.len() - 1])
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn outside_workspace_uses_plan_dir_mount() {
        let loc = PlanLocation::OutsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/bp"),
        };
        assert_eq!(
            loc.container_plan_path("api/README.md"),
            "/home/agent/plans/api/README.md",
        );
    }

    #[test]
    fn inside_workspace_uses_workspace_mount() {
        let loc = PlanLocation::InsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/ws/docs/plans/001"),
            blueprint_rel_to_workspace: PathBuf::from("docs/plans/001"),
        };
        assert_eq!(
            loc.container_plan_path("README.md"),
            "/home/agent/workspace/docs/plans/001/README.md",
        );
    }

    #[test]
    fn inside_workspace_at_root_omits_leading_rel() {
        let loc = PlanLocation::InsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/ws"),
            blueprint_rel_to_workspace: PathBuf::new(),
        };
        assert_eq!(
            loc.container_plan_path("README.md"),
            "/home/agent/workspace/README.md",
        );
    }

    #[test]
    fn plan_dir_host_path_only_for_outside_workspace() {
        let inside = PlanLocation::InsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/ws/plans"),
            blueprint_rel_to_workspace: PathBuf::from("plans"),
        };
        assert!(inside.plan_dir_host_path().is_none());

        let outside = PlanLocation::OutsideWorkspace {
            blueprint_host_dir: PathBuf::from("/tmp/bp"),
        };
        assert_eq!(outside.plan_dir_host_path(), Some(Path::new("/tmp/bp")),);
    }

    #[test]
    fn resolve_detects_inside_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        let bp_dir = ws.join("docs/plans/001");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        match loc {
            PlanLocation::InsideWorkspace {
                blueprint_rel_to_workspace,
                ..
            } => {
                assert_eq!(blueprint_rel_to_workspace, PathBuf::from("docs/plans/001"));
            }
            other => panic!("expected InsideWorkspace, got {other:?}"),
        }
    }

    #[test]
    fn resolve_returns_outside_when_blueprint_diverges() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        let bp_dir = tmp.path().join("plans/002");
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        match loc {
            PlanLocation::OutsideWorkspace { blueprint_host_dir } => {
                let expected = std::fs::canonicalize(&bp_dir).unwrap();
                assert_eq!(blueprint_host_dir, expected);
            }
            other => panic!("expected OutsideWorkspace, got {other:?}"),
        }
    }

    #[test]
    fn resolve_returns_outside_when_no_workspace_given() {
        let tmp = tempfile::tempdir().unwrap();
        let bp_dir = tmp.path().join("plans/003");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let loc = resolve(&bp_file, None).unwrap();
        assert!(matches!(loc, PlanLocation::OutsideWorkspace { .. }));
    }

    #[test]
    fn resolve_detects_blueprint_at_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        let bp_file = ws.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        match loc {
            PlanLocation::InsideWorkspace {
                blueprint_rel_to_workspace,
                ..
            } => {
                assert_eq!(blueprint_rel_to_workspace, PathBuf::new());
            }
            other => panic!("expected InsideWorkspace, got {other:?}"),
        }
    }

    fn setup_git_dir(dir: &Path, origin_url: &str) {
        fs::create_dir_all(dir).unwrap();
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git init failed in {dir:?}");
        let status = Command::new("git")
            .args(["remote", "add", "origin", origin_url])
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git remote add failed in {dir:?}");
    }

    #[test]
    fn resolve_detects_via_git_root_when_host_and_workspace_share_origin() {
        let tmp = tempfile::tempdir().unwrap();
        let host = tmp.path().join("host-clone");
        setup_git_dir(&host, "https://github.com/x7c1/palette.git");
        let bp_dir = host.join("docs/plans/001");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let ws = tmp.path().join("workspace");
        setup_git_dir(&ws, "https://github.com/x7c1/palette.git");

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        match loc {
            PlanLocation::InsideWorkspace {
                blueprint_host_dir,
                blueprint_rel_to_workspace,
            } => {
                assert_eq!(blueprint_rel_to_workspace, PathBuf::from("docs/plans/001"));
                let ws_abs = fs::canonicalize(&ws).unwrap();
                assert_eq!(blueprint_host_dir, ws_abs.join("docs/plans/001"));
            }
            other => panic!("expected InsideWorkspace, got {other:?}"),
        }
    }

    #[test]
    fn resolve_matches_origin_urls_across_scheme_forms() {
        let tmp = tempfile::tempdir().unwrap();
        let host = tmp.path().join("host-clone");
        setup_git_dir(&host, "git@github.com:x7c1/palette.git");
        let bp_dir = host.join("docs/plans/002");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let ws = tmp.path().join("workspace");
        setup_git_dir(&ws, "https://github.com/x7c1/palette");

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        assert!(
            matches!(loc, PlanLocation::InsideWorkspace { .. }),
            "expected InsideWorkspace across ssh/https form mismatch"
        );
    }

    #[test]
    fn resolve_returns_outside_when_origins_differ() {
        let tmp = tempfile::tempdir().unwrap();
        let host = tmp.path().join("host-clone");
        setup_git_dir(&host, "https://github.com/x7c1/atelier.git");
        let bp_dir = host.join("docs/plans/001");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let ws = tmp.path().join("workspace");
        setup_git_dir(&ws, "https://github.com/x7c1/palette.git");

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        assert!(matches!(loc, PlanLocation::OutsideWorkspace { .. }));
    }

    #[test]
    fn resolve_returns_outside_when_blueprint_has_no_git() {
        let tmp = tempfile::tempdir().unwrap();
        let bp_dir = tmp.path().join("plans/001");
        fs::create_dir_all(&bp_dir).unwrap();
        let bp_file = bp_dir.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let ws = tmp.path().join("workspace");
        setup_git_dir(&ws, "https://github.com/x7c1/palette.git");

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        assert!(matches!(loc, PlanLocation::OutsideWorkspace { .. }));
    }

    #[test]
    fn resolve_returns_outside_when_blueprint_at_host_git_root() {
        let tmp = tempfile::tempdir().unwrap();
        let host = tmp.path().join("host-clone");
        setup_git_dir(&host, "https://github.com/x7c1/palette.git");
        let bp_file = host.join("README.md");
        fs::write(&bp_file, "# plan").unwrap();

        let ws = tmp.path().join("workspace");
        setup_git_dir(&ws, "https://github.com/x7c1/palette.git");

        let loc = resolve(&bp_file, Some(&ws)).unwrap();
        assert!(matches!(loc, PlanLocation::OutsideWorkspace { .. }));
    }

    #[test]
    fn canonicalize_origin_extracts_owner_repo_suffix() {
        let canonical = "x7c1/palette";
        assert_eq!(
            canonicalize_origin("https://github.com/x7c1/palette.git"),
            canonical
        );
        assert_eq!(
            canonicalize_origin("git@github.com:x7c1/palette.git"),
            canonical
        );
        assert_eq!(
            canonicalize_origin("ssh://git@github.com/x7c1/palette.git"),
            canonical
        );
        assert_eq!(
            canonicalize_origin("https://github.com/x7c1/palette/"),
            canonical
        );
        // Local bare path (used by workspace clones and test harness).
        assert_eq!(
            canonicalize_origin("/tmp/data/repos/x7c1/palette.git"),
            canonical
        );
    }
}
