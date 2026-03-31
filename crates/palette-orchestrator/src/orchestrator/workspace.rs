use std::path::{Path, PathBuf};
use std::process::Command;

use palette_domain::job::Repository;

/// Container-side mount point for the repo cache.
const CONTAINER_REPO_CACHE: &str = "/home/agent/repo-cache";

/// Manages host-side repository caches and workspaces.
///
/// Repository caches are bare clones stored under `data/repos/{org}/{repo}.git`.
/// Workspaces are `git clone --shared` copies stored under `data/workspace/{job_id}`.
pub struct WorkspaceManager {
    /// Root directory for data (typically `data/`).
    data_dir: PathBuf,
}

impl WorkspaceManager {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    /// Return the host path to the bare cache for a repository.
    /// e.g., `data/repos/x7c1/palette.git`
    pub fn repo_cache_path(&self, repo: &Repository) -> PathBuf {
        self.data_dir
            .join("repos")
            .join(format!("{}.git", repo.name))
    }

    /// Return the host path to a workspace directory.
    /// e.g., `data/workspace/C-a3f2b7e1`
    pub fn workspace_path(&self, job_id: &str) -> PathBuf {
        self.data_dir.join("workspace").join(job_id)
    }

    /// Ensure a bare cache exists for the repository.
    /// Creates it with `git clone --bare` if absent, otherwise runs `git fetch`.
    pub fn ensure_repo_cache(&self, repo: &Repository) -> crate::Result<PathBuf> {
        let cache_path = self.repo_cache_path(repo);
        if cache_path.exists() {
            tracing::info!(repo = %repo.name, path = %cache_path.display(), "updating repo cache");
            run_git(
                &cache_path,
                &["fetch", "--prune", "origin"],
                "fetch repo cache",
            )?;
        } else {
            let parent = cache_path.parent().expect("cache path must have parent");
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::External(Box::new(e)))?;

            let url = format!("https://github.com/{}.git", repo.name);
            tracing::info!(repo = %repo.name, url = %url, path = %cache_path.display(), "creating bare clone");
            run_git(
                parent,
                &[
                    "clone",
                    "--bare",
                    &url,
                    &cache_path.file_name().unwrap().to_string_lossy(),
                ],
                "bare clone",
            )?;

            // Disable automatic GC to prevent workspace corruption
            run_git(&cache_path, &["config", "gc.auto", "0"], "disable gc.auto")?;
            // Disable fileMode to avoid permission diffs between macOS and Linux
            run_git(
                &cache_path,
                &["config", "core.fileMode", "false"],
                "set core.fileMode",
            )?;
        }
        Ok(cache_path)
    }

    /// Create a workspace for a job using `git clone --shared`.
    /// Returns the absolute host path to the workspace.
    pub fn create_workspace(
        &self,
        job_id: &str,
        repo: &Repository,
    ) -> crate::Result<WorkspaceInfo> {
        let cache_path = self.ensure_repo_cache(repo)?;
        let ws_path = self.workspace_path(job_id);

        if ws_path.exists() {
            tracing::warn!(job_id = %job_id, path = %ws_path.display(), "workspace already exists, removing");
            std::fs::remove_dir_all(&ws_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        }

        let ws_parent = ws_path.parent().expect("workspace path must have parent");
        std::fs::create_dir_all(ws_parent).map_err(|e| crate::Error::External(Box::new(e)))?;

        tracing::info!(
            job_id = %job_id,
            cache = %cache_path.display(),
            workspace = %ws_path.display(),
            "creating shared clone workspace"
        );
        run_git(
            ws_parent,
            &[
                "clone",
                "--shared",
                &cache_path.to_string_lossy(),
                &ws_path.file_name().unwrap().to_string_lossy(),
            ],
            "shared clone",
        )?;

        // Rewrite alternates to use the container-side path
        let alternates_path = ws_path.join(".git/objects/info/alternates");
        let container_alternates = format!("{CONTAINER_REPO_CACHE}/objects\n");
        std::fs::write(&alternates_path, &container_alternates)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // Disable push: set pushurl to a sentinel value
        run_git(
            &ws_path,
            &["config", "remote.origin.pushurl", "PUSH_DISABLED"],
            "disable pushurl",
        )?;

        let cache_abs =
            std::fs::canonicalize(&cache_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        let ws_abs =
            std::fs::canonicalize(&ws_path).map_err(|e| crate::Error::External(Box::new(e)))?;

        Ok(WorkspaceInfo {
            host_path: ws_abs.to_string_lossy().to_string(),
            repo_cache_path: cache_abs.to_string_lossy().to_string(),
        })
    }

    /// Return the host path to an artifacts directory for a craft job.
    /// e.g., `data/artifacts/{workflow_id}/{craft_job_id}`
    pub fn artifacts_path(&self, workflow_id: &str, craft_job_id: &str) -> PathBuf {
        self.data_dir
            .join("artifacts")
            .join(workflow_id)
            .join(craft_job_id)
    }

    /// Remove a workspace directory.
    pub fn remove_workspace(&self, job_id: &str) {
        let ws_path = self.workspace_path(job_id);
        if ws_path.exists() {
            tracing::info!(job_id = %job_id, path = %ws_path.display(), "removing workspace");
            if let Err(e) = std::fs::remove_dir_all(&ws_path) {
                tracing::warn!(
                    job_id = %job_id,
                    error = %e,
                    "failed to remove workspace"
                );
            }
        }
    }
}

/// Information about a created workspace, containing absolute host paths.
pub struct WorkspaceInfo {
    pub host_path: String,
    pub repo_cache_path: String,
}

/// Run a git command in the given directory.
fn run_git(cwd: &Path, args: &[&str], description: &str) -> crate::Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| crate::Error::External(Box::new(e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::External(
            format!("git {description} failed: {stderr}").into(),
        ));
    }
    Ok(())
}
