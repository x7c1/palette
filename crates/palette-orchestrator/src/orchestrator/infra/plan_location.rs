use std::path::{Path, PathBuf};

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
/// When `workspace_host_path` is `Some` and the Blueprint directory sits
/// under that workspace root, the result is `InsideWorkspace`. Otherwise the
/// Blueprint is considered external and must be mounted separately.
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
    }

    Ok(PlanLocation::OutsideWorkspace {
        blueprint_host_dir: abs_blueprint_dir,
    })
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
}
