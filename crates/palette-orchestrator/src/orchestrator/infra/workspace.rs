use std::path::{Path, PathBuf};
use std::process::Command;

use palette_domain::job::{PullRequest, Repository};

use super::plan_location;

/// Container-side mount point for the repo cache.
const CONTAINER_REPO_CACHE: &str = "/home/agent/repo-cache";

/// Sentinel pushurl used to block pushes from workspaces that must remain
/// read-only to origin (e.g., PR review clones).
const PUSH_DISABLED: &str = "PUSH_DISABLED";

/// Commit message used when importing the Blueprint directory into a
/// Repo-inside-Plan workspace.
const PLAN_IMPORT_COMMIT_MESSAGE: &str = "chore(plan): import workflow plan";

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
    /// e.g., `data/repos/x7c1/palette-demo.git`
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
            // Modern git's `clone --bare` does not set a fetch refspec on
            // origin, which makes later `git fetch --prune origin` a no-op
            // for branches that were created on the remote after this
            // initial clone. Seed the standard bare-mirror refspec so the
            // cache stays in sync with origin's branches.
            run_git(
                &cache_path,
                &[
                    "config",
                    "remote.origin.fetch",
                    "+refs/heads/*:refs/heads/*",
                ],
                "set fetch refspec",
            )?;
        }

        // Return absolute path so callers don't depend on CWD
        let abs_cache =
            std::fs::canonicalize(&cache_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        Ok(abs_cache)
    }

    /// Create a workspace for a craft job using `git clone --shared`.
    ///
    /// The resulting workspace is always checked out on `repo.work_branch`.
    /// When the remote already has that branch, it is checked out as-is
    /// (resume scenario). When it does not, the work branch is created from
    /// `repo.source_branch` (or the repository's default branch when
    /// `source_branch` is omitted).
    ///
    /// When the Blueprint directory sits inside the workspace
    /// (Repo-inside-Plan mode), its contents are staged and committed on the
    /// work branch so that Plan files participate in the workspace's git
    /// history. The operation is idempotent: nothing is committed when the
    /// tree has no changes.
    pub fn create_workspace(
        &self,
        job_id: &str,
        repo: &Repository,
        blueprint_path: &Path,
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
                "--no-checkout",
                &cache_path.to_string_lossy(),
                &ws_path.file_name().unwrap().to_string_lossy(),
            ],
            "shared clone",
        )?;

        // Branch create-or-checkout.
        // The work branch lives on origin → check it out directly; otherwise
        // derive it from the source branch.
        if remote_has_branch(&ws_path, &repo.work_branch)? {
            run_git(
                &ws_path,
                &["checkout", &repo.work_branch],
                "checkout work branch",
            )?;
        } else {
            let source_branch = match repo.source_branch.as_deref() {
                Some(sb) => sb.to_string(),
                None => resolve_default_branch(&cache_path)?,
            };
            run_git(
                &ws_path,
                &["checkout", &source_branch],
                "checkout source branch",
            )?;
            run_git(
                &ws_path,
                &["checkout", "-b", &repo.work_branch],
                "create work branch",
            )?;
        }

        let cache_abs =
            std::fs::canonicalize(&cache_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        let ws_abs =
            std::fs::canonicalize(&ws_path).map_err(|e| crate::Error::External(Box::new(e)))?;

        // Repo-inside-Plan mode: when the Blueprint lives in a host clone of
        // the same repository that the workspace was just cloned from, copy
        // the Blueprint directory into the workspace and commit it onto the
        // work branch so that Plan files travel with the code.
        //
        // The host clone is authoritative: if the Operator has uncommitted
        // edits in their Blueprint directory, those edits end up in the
        // workspace commit (subject to the usual idempotency — an identical
        // committed copy already on the branch produces no new commit).
        //
        // This must run before the alternates rewrite below — once alternates
        // points at the container-side path, host-side git operations on the
        // workspace can no longer resolve objects.
        let blueprint_dir = blueprint_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        if blueprint_dir.exists() {
            let plan_loc = plan_location::resolve(blueprint_path, Some(&ws_abs))
                .map_err(|e| crate::Error::External(Box::new(e)))?;
            if let plan_location::PlanLocation::InsideWorkspace {
                blueprint_rel_to_workspace,
                ..
            } = plan_loc
            {
                let host_blueprint_dir_abs = std::fs::canonicalize(&blueprint_dir)
                    .map_err(|e| crate::Error::External(Box::new(e)))?;
                let target_dir = ws_abs.join(&blueprint_rel_to_workspace);
                let already_in_place = std::fs::canonicalize(&target_dir)
                    .map(|c| c == host_blueprint_dir_abs)
                    .unwrap_or(false);
                if !already_in_place {
                    copy_dir_contents(&host_blueprint_dir_abs, &target_dir)?;
                }
                sync_blueprint_into_workspace(&ws_abs, &target_dir)?;
            }
        }

        // Rewrite alternates to use the container-side path. From this point
        // on, host-side git commands against the workspace will fail to
        // resolve objects — the workspace is ready for container use only.
        let alternates_path = ws_path.join(".git/objects/info/alternates");
        let container_alternates = format!("{CONTAINER_REPO_CACHE}/objects\n");
        std::fs::write(&alternates_path, &container_alternates)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // Craft workspaces keep the origin pushurl intact so that future
        // push-based follow-ups (Publisher worker, PR creation) can reach the
        // remote. Push is not executed here — the caller chain still has no
        // push step today.

        Ok(WorkspaceInfo {
            host_path: ws_abs.to_string_lossy().to_string(),
            repo_cache_path: cache_abs.to_string_lossy().to_string(),
        })
    }

    /// Create a workspace for a PR review job.
    ///
    /// Fetches the PR head ref into the bare cache, then creates a shared clone
    /// workspace checked out at the PR's head commit.
    pub fn create_pr_workspace(
        &self,
        job_id: &str,
        pr: &PullRequest,
    ) -> crate::Result<WorkspaceInfo> {
        let repo_name = format!("{}/{}", pr.owner, pr.repo);
        let repo = Repository::parse(&repo_name, "main", None)
            .map_err(|e| crate::Error::External(format!("invalid PR repository: {e:?}").into()))?;

        let cache_path = self.ensure_repo_cache(&repo)?;

        // Fetch the PR head ref into the bare cache
        let pr_ref = format!("refs/pull/{}/head", pr.number);
        tracing::info!(pr = %pr, pr_ref = %pr_ref, "fetching PR ref into bare cache");
        run_git(&cache_path, &["fetch", "origin", &pr_ref], "fetch PR ref")?;

        // Get the SHA of FETCH_HEAD
        let sha_output = Command::new("git")
            .args(["rev-parse", "FETCH_HEAD"])
            .current_dir(&cache_path)
            .output()
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        if !sha_output.status.success() {
            let stderr = String::from_utf8_lossy(&sha_output.stderr);
            return Err(crate::Error::External(
                format!("git rev-parse FETCH_HEAD failed: {stderr}").into(),
            ));
        }
        let sha = String::from_utf8_lossy(&sha_output.stdout)
            .trim()
            .to_string();

        // Create shared clone workspace
        let ws_path = self.workspace_path(job_id);
        if ws_path.exists() {
            std::fs::remove_dir_all(&ws_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        }
        let ws_parent = ws_path.parent().expect("workspace path must have parent");
        std::fs::create_dir_all(ws_parent).map_err(|e| crate::Error::External(Box::new(e)))?;

        tracing::info!(
            job_id = %job_id,
            pr = %pr,
            sha = %sha,
            "creating shared clone workspace for PR review"
        );
        run_git(
            ws_parent,
            &[
                "clone",
                "--shared",
                "--no-checkout",
                &cache_path.to_string_lossy(),
                &ws_path.file_name().unwrap().to_string_lossy(),
            ],
            "shared clone for PR",
        )?;

        // Checkout the PR commit (detached HEAD)
        run_git(&ws_path, &["checkout", &sha], "checkout PR commit")?;

        // Rewrite alternates to use the container-side path
        let alternates_path = ws_path.join(".git/objects/info/alternates");
        let container_alternates = format!("{CONTAINER_REPO_CACHE}/objects\n");
        std::fs::write(&alternates_path, &container_alternates)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // PR review workspaces must not push back to origin.
        run_git(
            &ws_path,
            &["config", "remote.origin.pushurl", PUSH_DISABLED],
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

/// Return whether the remote tracking ref for `branch` exists in `ws_path`.
///
/// After `clone --shared --no-checkout`, the workspace's `refs/remotes/origin/*`
/// mirrors the bare cache's `refs/heads/*`; a hit here means the branch is
/// already published to origin (resume scenario).
fn remote_has_branch(ws_path: &Path, branch: &str) -> crate::Result<bool> {
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let status = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &remote_ref])
        .current_dir(ws_path)
        .status()
        .map_err(|e| crate::Error::External(Box::new(e)))?;
    Ok(status.success())
}

/// Read the repository's default branch name.
///
/// Uses `git ls-remote --symref origin HEAD`, which queries origin for its
/// current HEAD symbolic ref and does not depend on any ref-namespace layout
/// in the bare cache. The bare clone produced by `git clone --bare` keeps its
/// tracking refs under `refs/heads/*` (not `refs/remotes/origin/*`), so
/// `git symbolic-ref refs/remotes/origin/HEAD` would fail — `ls-remote`
/// sidesteps that layout entirely.
fn resolve_default_branch(cache_path: &Path) -> crate::Result<String> {
    let output = Command::new("git")
        .args(["ls-remote", "--symref", "origin", "HEAD"])
        .current_dir(cache_path)
        .output()
        .map_err(|e| crate::Error::External(Box::new(e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::External(
            format!("git ls-remote --symref origin HEAD failed: {stderr}").into(),
        ));
    }
    // The first line has the form:
    //   ref: refs/heads/<branch>\tHEAD
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().ok_or_else(|| {
        crate::Error::External("empty output from git ls-remote --symref origin HEAD".into())
    })?;
    let target = first
        .strip_prefix("ref: ")
        .and_then(|rest| rest.split_whitespace().next())
        .ok_or_else(|| {
            crate::Error::External(format!("unexpected ls-remote line: {first}").into())
        })?;
    target
        .strip_prefix("refs/heads/")
        .map(|s| s.to_string())
        .ok_or_else(|| crate::Error::External(format!("unexpected HEAD target: {target}").into()))
}

/// Stage the Blueprint directory and commit if there are changes.
///
/// Idempotent: returns without committing when `git status --porcelain` is
/// empty (e.g., resume scenarios where the plan commit is already on the
/// work branch).
///
/// Before committing we verify the workspace can resolve a git identity —
/// either via the Operator's global `~/.gitconfig` or the workspace's own
/// repo-local config. When neither is available (CI runners, fresh machines
/// without gitconfig), a Palette-owned fallback identity is written to the
/// workspace's repo-local config so the commit can still proceed. The
/// Operator's identity always wins when present because `git config --get`
/// reads repo-local before global, and we only write the fallback when both
/// are absent.
fn sync_blueprint_into_workspace(ws_abs: &Path, blueprint_dir_abs: &Path) -> crate::Result<()> {
    run_git(
        ws_abs,
        &["add", "--", &blueprint_dir_abs.to_string_lossy()],
        "stage blueprint",
    )?;
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(ws_abs)
        .output()
        .map_err(|e| crate::Error::External(Box::new(e)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::Error::External(
            format!("git status --porcelain failed: {stderr}").into(),
        ));
    }
    if String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        tracing::info!(
            workspace = %ws_abs.display(),
            "no blueprint changes to commit"
        );
        return Ok(());
    }
    ensure_commit_identity(ws_abs)?;
    run_git(
        ws_abs,
        &["commit", "-m", PLAN_IMPORT_COMMIT_MESSAGE],
        "commit blueprint",
    )?;
    Ok(())
}

/// Ensure the workspace has a resolvable `user.email` / `user.name` so that
/// `git commit` succeeds. When the Operator's global `~/.gitconfig` already
/// provides an identity, this is a no-op; otherwise a Palette-owned default
/// is written to the workspace's repo-local config so commits can proceed on
/// CI runners and fresh machines without gitconfig.
fn ensure_commit_identity(ws_abs: &Path) -> crate::Result<()> {
    const FALLBACK_EMAIL: &str = "palette-import@localhost";
    const FALLBACK_NAME: &str = "Palette Plan Import";
    for (key, fallback) in [("user.email", FALLBACK_EMAIL), ("user.name", FALLBACK_NAME)] {
        let got = Command::new("git")
            .args(["config", "--get", key])
            .current_dir(ws_abs)
            .output()
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        if got.status.success() && !got.stdout.is_empty() {
            continue;
        }
        run_git(
            ws_abs,
            &["config", key, fallback],
            "set fallback git identity",
        )?;
    }
    Ok(())
}

/// Recursively copy the contents of `src` into `dst`, creating `dst` if it
/// does not exist. Directories are traversed; regular files are overwritten.
/// Symlinks and other special entries are skipped to avoid propagating
/// host-specific filesystem state into the workspace.
fn copy_dir_contents(src: &Path, dst: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| crate::Error::External(Box::new(e)))?;
    let entries = std::fs::read_dir(src).map_err(|e| crate::Error::External(Box::new(e)))?;
    for entry in entries {
        let entry = entry.map_err(|e| crate::Error::External(Box::new(e)))?;
        let file_type = entry
            .file_type()
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    //! Integration-style tests that drive `WorkspaceManager` against a local
    //! fake "origin" repository (a bare repo on disk) so we can verify branch
    //! create-or-checkout, plan sync, and pushurl handling without hitting
    //! GitHub.
    //!
    //! The tests bypass `ensure_repo_cache`'s GitHub clone path by seeding a
    //! bare clone of the fake origin into the expected `data/repos/...`
    //! location before calling `create_workspace`.

    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    struct Harness {
        _tmp: tempfile::TempDir,
        data_dir: PathBuf,
        origin: PathBuf,
        manager: WorkspaceManager,
    }

    fn run(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("git spawn");
        assert!(status.success(), "git {args:?} in {cwd:?} failed");
    }

    fn setup_harness(repo_name: &str, default_branch: &str) -> Harness {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().join("data");
        let origin_dir = tmp.path().join("origin");
        fs::create_dir_all(&origin_dir).unwrap();

        // Build a source working repo, commit an initial file on the default
        // branch, then export it as a bare "origin".
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        run(&src, &["init", "-q", "--initial-branch", default_branch]);
        run(&src, &["config", "user.email", "test@example.com"]);
        run(&src, &["config", "user.name", "Test User"]);
        fs::write(src.join("README.md"), "# source\n").unwrap();
        run(&src, &["add", "README.md"]);
        run(&src, &["commit", "-q", "-m", "initial"]);

        let origin_bare = origin_dir.join(format!("{repo_name}.git"));
        fs::create_dir_all(origin_bare.parent().unwrap()).unwrap();
        run(
            tmp.path(),
            &[
                "clone",
                "--bare",
                "-q",
                src.to_string_lossy().as_ref(),
                origin_bare.to_string_lossy().as_ref(),
            ],
        );
        // Ensure the bare knows its own HEAD points at the default branch.
        run(
            &origin_bare,
            &[
                "symbolic-ref",
                "HEAD",
                &format!("refs/heads/{default_branch}"),
            ],
        );

        // Pre-seed the repo cache location used by `ensure_repo_cache`, then
        // point it at the local fake origin instead of github.com.
        let cache_path = data_dir.join("repos").join(format!("{repo_name}.git"));
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        run(
            tmp.path(),
            &[
                "clone",
                "--bare",
                "-q",
                origin_bare.to_string_lossy().as_ref(),
                cache_path.to_string_lossy().as_ref(),
            ],
        );
        run(&cache_path, &["config", "gc.auto", "0"]);
        run(&cache_path, &["config", "core.fileMode", "false"]);
        run(
            &cache_path,
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/heads/*",
            ],
        );
        run(
            &cache_path,
            &[
                "remote",
                "set-url",
                "origin",
                origin_bare.to_string_lossy().as_ref(),
            ],
        );

        let manager = WorkspaceManager::new(data_dir.clone());
        Harness {
            _tmp: tmp,
            data_dir,
            origin: origin_bare,
            manager,
        }
    }

    fn make_repo(name: &str, branch: &str, source_branch: Option<&str>) -> Repository {
        Repository::parse(name, branch, source_branch.map(String::from)).unwrap()
    }

    fn workspace_branch(ws: &Path) -> String {
        let out = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(ws)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Run `git log -1 --format=%s` against a workspace whose alternates have
    /// already been rewritten to the container path. We temporarily restore
    /// host-side alternates pointing at `cache_objects` so host-side git can
    /// resolve objects, then put back the container path.
    fn workspace_head_message(ws: &Path, cache_objects: &Path) -> String {
        let alternates = ws.join(".git/objects/info/alternates");
        let saved = fs::read_to_string(&alternates).unwrap();
        fs::write(
            &alternates,
            format!("{}\n", cache_objects.to_string_lossy()),
        )
        .unwrap();
        let out = Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .current_dir(ws)
            .output()
            .unwrap();
        fs::write(&alternates, saved).unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn read_alternates(ws: &Path) -> String {
        fs::read_to_string(ws.join(".git/objects/info/alternates")).unwrap()
    }

    fn pushurl(ws: &Path) -> String {
        let out = Command::new("git")
            .args(["config", "--get", "remote.origin.pushurl"])
            .current_dir(ws)
            .output()
            .unwrap();
        if out.status.success() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            String::new()
        }
    }

    #[test]
    fn create_workspace_creates_work_branch_from_default_branch_when_remote_missing() {
        let h = setup_harness("x7c1/demo-a", "main");
        let repo = make_repo("x7c1/demo-a", "feature/new-branch", None);
        let blueprint_path = h._tmp.path().join("outside-plan/README.md");
        fs::create_dir_all(blueprint_path.parent().unwrap()).unwrap();
        fs::write(&blueprint_path, "# plan\n").unwrap();

        let info = h
            .manager
            .create_workspace("C-create-a", &repo, &blueprint_path)
            .unwrap();
        let ws = PathBuf::from(&info.host_path);
        assert_eq!(workspace_branch(&ws), "feature/new-branch");
        assert!(read_alternates(&ws).contains(CONTAINER_REPO_CACHE));
        // Craft workspace must not disable push.
        assert!(pushurl(&ws).is_empty() || pushurl(&ws) != PUSH_DISABLED);
    }

    #[test]
    fn create_workspace_checks_out_existing_work_branch_from_remote() {
        let h = setup_harness("x7c1/demo-b", "main");
        // Push a feature branch to the fake origin so it becomes the resume case.
        let staging = h._tmp.path().join("staging");
        run(
            h._tmp.path(),
            &[
                "clone",
                "-q",
                h.origin.to_string_lossy().as_ref(),
                staging.to_string_lossy().as_ref(),
            ],
        );
        run(&staging, &["config", "user.email", "test@example.com"]);
        run(&staging, &["config", "user.name", "Test User"]);
        run(&staging, &["checkout", "-q", "-b", "feature/existing"]);
        fs::write(staging.join("FEATURE.md"), "# feature\n").unwrap();
        run(&staging, &["add", "FEATURE.md"]);
        run(&staging, &["commit", "-q", "-m", "seed feature"]);
        run(&staging, &["push", "-q", "origin", "feature/existing"]);

        let repo = make_repo("x7c1/demo-b", "feature/existing", None);
        let blueprint_path = h._tmp.path().join("outside-plan/README.md");
        fs::create_dir_all(blueprint_path.parent().unwrap()).unwrap();
        fs::write(&blueprint_path, "# plan\n").unwrap();

        let info = h
            .manager
            .create_workspace("C-create-b", &repo, &blueprint_path)
            .unwrap();
        let ws = PathBuf::from(&info.host_path);
        assert_eq!(workspace_branch(&ws), "feature/existing");
        assert!(
            ws.join("FEATURE.md").exists(),
            "should have upstream commit"
        );
    }

    #[test]
    fn create_workspace_uses_explicit_source_branch_when_present() {
        let h = setup_harness("x7c1/demo-c", "main");
        // Seed a "release" branch with divergent content on origin.
        let staging = h._tmp.path().join("staging-c");
        run(
            h._tmp.path(),
            &[
                "clone",
                "-q",
                h.origin.to_string_lossy().as_ref(),
                staging.to_string_lossy().as_ref(),
            ],
        );
        run(&staging, &["config", "user.email", "test@example.com"]);
        run(&staging, &["config", "user.name", "Test User"]);
        run(&staging, &["checkout", "-q", "-b", "release/1.0"]);
        fs::write(staging.join("RELEASE.md"), "# release\n").unwrap();
        run(&staging, &["add", "RELEASE.md"]);
        run(&staging, &["commit", "-q", "-m", "seed release"]);
        run(&staging, &["push", "-q", "origin", "release/1.0"]);

        let repo = make_repo("x7c1/demo-c", "feature/from-release", Some("release/1.0"));
        let blueprint_path = h._tmp.path().join("plan/README.md");
        fs::create_dir_all(blueprint_path.parent().unwrap()).unwrap();
        fs::write(&blueprint_path, "# plan\n").unwrap();

        let info = h
            .manager
            .create_workspace("C-create-c", &repo, &blueprint_path)
            .unwrap();
        let ws = PathBuf::from(&info.host_path);
        assert_eq!(workspace_branch(&ws), "feature/from-release");
        assert!(
            ws.join("RELEASE.md").exists(),
            "work branch should carry release content"
        );
    }

    #[test]
    fn create_workspace_commits_blueprint_when_inside_workspace() {
        let h = setup_harness("x7c1/demo-d", "main");
        let repo = make_repo("x7c1/demo-d", "feature/with-plan", None);

        // Prepare the workspace path, then drop a Blueprint inside it BEFORE
        // calling create_workspace. create_workspace wipes the workspace as a
        // first step, so staging Blueprint content inside must happen on the
        // real branch after the checkout — we achieve that by pointing the
        // blueprint at a host dir outside the workspace, creating the workspace,
        // THEN writing the Blueprint and re-running create_workspace which
        // wipes. Instead, for this test, we simulate a Repo-inside-Plan layout
        // by placing the Blueprint in a location that will be created under
        // the workspace root via post-creation file placement, then calling a
        // second create_workspace (idempotent on plan sync via fresh clone).
        //
        // To keep the test simple, we instead construct the expected
        // Repo-inside layout manually: create workspace once with an outside
        // Blueprint, then rename the Blueprint so it lives inside the
        // workspace, and call create_workspace a second time — the second
        // call's clone sees the Blueprint inside the workspace path it is
        // about to recreate.
        let ws_path = h.data_dir.join("workspace").join("C-create-d");
        fs::create_dir_all(ws_path.parent().unwrap()).unwrap();
        let blueprint_in_ws = ws_path.join("docs/plans/001");
        fs::create_dir_all(&blueprint_in_ws).unwrap();
        // Seed a README so the post-creation sync has content to stage.
        // create_workspace will wipe and recreate ws_path, so we stash the
        // Blueprint outside first, then re-create it after clone.
        let outside_blueprint = h._tmp.path().join("plan-cache/README.md");
        fs::create_dir_all(outside_blueprint.parent().unwrap()).unwrap();
        fs::write(&outside_blueprint, "# inside-workspace plan\n").unwrap();

        // First pass with an outside Blueprint to create the workspace.
        h.manager
            .create_workspace("C-create-d", &repo, &outside_blueprint)
            .unwrap();
        // Now stage a Blueprint inside the workspace and re-run
        // create_workspace pointing at the inside path. `create_workspace`
        // wipes and re-clones, then syncs the Blueprint directory when it's
        // detected inside the freshly-cloned workspace.
        let inside_blueprint_dir = ws_path.join("docs/plans/001");
        fs::create_dir_all(&inside_blueprint_dir).unwrap();
        let inside_blueprint = inside_blueprint_dir.join("README.md");
        fs::write(&inside_blueprint, "# inside-workspace plan\n").unwrap();

        // Because create_workspace wipes the workspace first, the inside
        // Blueprint written above gets discarded. To emulate a true
        // Repo-inside-Plan where the Blueprint ships with the branch, we
        // commit the Blueprint to the feature branch on origin first.
        let staging = h._tmp.path().join("staging-d");
        run(
            h._tmp.path(),
            &[
                "clone",
                "-q",
                h.origin.to_string_lossy().as_ref(),
                staging.to_string_lossy().as_ref(),
            ],
        );
        run(&staging, &["config", "user.email", "test@example.com"]);
        run(&staging, &["config", "user.name", "Test User"]);
        run(&staging, &["checkout", "-q", "-b", "feature/with-plan"]);
        fs::create_dir_all(staging.join("docs/plans/001")).unwrap();
        fs::write(
            staging.join("docs/plans/001/README.md"),
            "# inside-workspace plan\n",
        )
        .unwrap();
        run(&staging, &["add", "docs"]);
        run(&staging, &["commit", "-q", "-m", "seed plan"]);
        run(&staging, &["push", "-q", "origin", "feature/with-plan"]);

        // Final run: the work branch exists on origin, the Blueprint lives
        // inside the workspace — plan sync should be a no-op (the Blueprint
        // is already in the committed tree), and HEAD message should be the
        // seeded "seed plan" commit, not a plan import commit.
        let info = h
            .manager
            .create_workspace("C-create-d", &repo, &inside_blueprint)
            .unwrap();
        let ws = PathBuf::from(&info.host_path);
        assert!(ws.join("docs/plans/001/README.md").exists());
        let cache_objects = PathBuf::from(&info.repo_cache_path).join("objects");
        assert_eq!(
            workspace_head_message(&ws, &cache_objects),
            "seed plan",
            "plan sync must be idempotent when Blueprint is already committed"
        );
    }

    #[test]
    fn create_workspace_materialises_blueprint_from_host_clone() {
        let h = setup_harness("x7c1/demo-g", "main");
        let repo = make_repo("x7c1/demo-g", "feature/with-host-plan", None);

        // Simulate the Operator's clone of the target repo, authored with a
        // Blueprint that has NOT yet been committed. This mirrors the
        // `/palette:plan` output flow where the Blueprint lives on the
        // Operator's filesystem before the Workflow is ever started.
        let host_clone = h._tmp.path().join("host-clone-g");
        run(
            h._tmp.path(),
            &[
                "clone",
                "-q",
                h.origin.to_string_lossy().as_ref(),
                host_clone.to_string_lossy().as_ref(),
            ],
        );
        let bp_dir = host_clone.join("docs/plans/refresh");
        fs::create_dir_all(&bp_dir).unwrap();
        fs::write(bp_dir.join("README.md"), "# refresh plan\n").unwrap();
        fs::write(bp_dir.join("blueprint.yaml"), "task:\n  key: root\n").unwrap();
        let blueprint_path = bp_dir.join("blueprint.yaml");

        let info = h
            .manager
            .create_workspace("C-create-g", &repo, &blueprint_path)
            .unwrap();
        let ws = PathBuf::from(&info.host_path);

        // Blueprint files must travel with the work branch.
        assert!(ws.join("docs/plans/refresh/README.md").exists());
        assert!(ws.join("docs/plans/refresh/blueprint.yaml").exists());

        // HEAD commit should be the plan import commit.
        let cache_objects = PathBuf::from(&info.repo_cache_path).join("objects");
        assert_eq!(
            workspace_head_message(&ws, &cache_objects),
            "chore(plan): import workflow plan",
        );
    }

    #[test]
    fn create_pr_workspace_keeps_push_disabled() {
        let h = setup_harness("x7c1/demo-e", "main");
        // Stage a PR ref on origin to be fetched.
        let staging = h._tmp.path().join("staging-e");
        run(
            h._tmp.path(),
            &[
                "clone",
                "-q",
                h.origin.to_string_lossy().as_ref(),
                staging.to_string_lossy().as_ref(),
            ],
        );
        run(&staging, &["config", "user.email", "test@example.com"]);
        run(&staging, &["config", "user.name", "Test User"]);
        run(&staging, &["checkout", "-q", "-b", "pr-branch"]);
        fs::write(staging.join("PR.md"), "# pr\n").unwrap();
        run(&staging, &["add", "PR.md"]);
        run(&staging, &["commit", "-q", "-m", "pr change"]);
        let sha = {
            let out = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&staging)
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        // Publish the PR ref directly on origin.
        run(&staging, &["push", "-q", "origin", "HEAD:refs/pull/1/head"]);
        let _ = sha;

        let pr = palette_domain::job::PullRequest::parse("x7c1", "demo-e", 1).unwrap();
        let info = h.manager.create_pr_workspace("R-pr-e", &pr).unwrap();
        let ws = PathBuf::from(&info.host_path);
        assert_eq!(pushurl(&ws), PUSH_DISABLED);
    }

    #[test]
    fn resolve_default_branch_reads_origin_head() {
        let h = setup_harness("x7c1/demo-f", "trunk");
        let cache = h.data_dir.join("repos").join("x7c1/demo-f.git");
        // ensure_repo_cache runs set-head which writes refs/remotes/origin/HEAD.
        let repo = make_repo("x7c1/demo-f", "trunk", None);
        h.manager.ensure_repo_cache(&repo).unwrap();
        assert_eq!(resolve_default_branch(&cache).unwrap(), "trunk");
    }
}
