//! Generate the review diff that reviewers and integrators mount as
//! `/home/agent/diff/`.
//!
//! Both PR review and craft review run git on the host-side bare cache.
//! The cache itself has no alternates and is freely driven from the host;
//! all we need is to make sure both sides of the diff range are present as
//! refs in the cache before running `git diff`.
//!
//! - **PR review**: base and head refs come from origin. `GitHubReviewPort::get_pr_base`
//!   resolves the SHAs via `gh api`, then we fetch both from origin into the cache.
//! - **Craft review**: the base branch is already on origin, but the crafter's
//!   work branch is only in the workspace (not yet pushed). We fetch it from
//!   the workspace into a namespaced ref on the bare cache, then diff as usual.

use std::path::{Path, PathBuf};
use std::process::Command;

use palette_domain::job::{Job, PullRequest, Repository};

use super::Orchestrator;
use super::workspace::WorkspaceManager;

/// Required output files inside the diff directory (the reviewer prompt
/// hard-codes these names, so they are a contract).
const DIFF_PATCH_FILE: &str = "diff.patch";
const CHANGED_FILES_FILE: &str = "changed_files.txt";

impl Orchestrator {
    /// Generate diff files for a review job at the given round.
    ///
    /// Writes `diff.patch` and `changed_files.txt` under the host diff
    /// directory. Callers are expected to surface the host directory as a
    /// read-only bind mount at `/home/agent/diff/` in the reviewer/integrator
    /// containers.
    ///
    /// This is synchronous on purpose: the caller spawns the review worker
    /// immediately after, and proceeding with stale or missing diff data
    /// would reintroduce the scope-violation bug this plan is fixing.
    pub(crate) fn generate_review_diff(
        &self,
        review_job: &Job,
        _round: u32,
    ) -> crate::Result<PathBuf> {
        let diff_dir = self.workspace_manager.diff_path(review_job.id.as_ref());
        std::fs::create_dir_all(&diff_dir).map_err(|e| crate::Error::External(Box::new(e)))?;

        let target = review_job.detail.review_target().ok_or_else(|| {
            crate::Error::External(
                format!(
                    "job {} is not a review job; cannot generate diff",
                    review_job.id
                )
                .into(),
            )
        })?;

        match target.pull_request() {
            Some(pr) => self.generate_pr_diff(pr, &diff_dir)?,
            None => self.generate_craft_diff(review_job, &diff_dir)?,
        }

        let diff_abs =
            std::fs::canonicalize(&diff_dir).map_err(|e| crate::Error::External(Box::new(e)))?;
        Ok(diff_abs)
    }

    /// PR review: both refs are on origin, so we just fetch and diff.
    fn generate_pr_diff(&self, pr: &PullRequest, diff_dir: &Path) -> crate::Result<()> {
        let repo_name = format!("{}/{}", pr.owner, pr.repo);
        let repo = Repository::parse(&repo_name, "main", None)
            .map_err(|e| crate::Error::External(format!("invalid PR repository: {e:?}").into()))?;

        let cache_path = self.workspace_manager.ensure_repo_cache(&repo)?;

        let refs = self
            .interactor
            .github_review_port
            .get_pr_base(&pr.owner, &pr.repo, pr.number)
            .map_err(|e| crate::Error::External(format!("get_pr_base failed: {e}").into()))?;

        // Ensure the base SHA is present in the cache even when origin does
        // not mirror the PR's base under refs/heads/* (e.g., a topic branch
        // that was deleted after the PR opened).
        let base_spec = format!("+{}:refs/palette/base/{}", refs.base_sha, refs.base_sha);
        run_git(
            &cache_path,
            &["fetch", "--no-tags", "origin", &base_spec],
            "fetch PR base sha",
        )?;

        // `gh api pulls/{number}` returns the current head SHA, but fetching
        // `refs/pull/{n}/head` explicitly keeps us resilient against later
        // force-pushes that overwrite it.
        let pr_ref = format!("refs/pull/{}/head", pr.number);
        run_git(&cache_path, &["fetch", "origin", &pr_ref], "fetch PR head")?;

        let diff_range = format!("{}...{}", refs.base_sha, refs.head_sha);
        write_diff_outputs(&cache_path, &diff_range, diff_dir, "PR")?;

        tracing::info!(
            owner = %pr.owner,
            repo = %pr.repo,
            number = pr.number,
            base = %refs.base_sha,
            head = %refs.head_sha,
            diff_dir = %diff_dir.display(),
            "generated PR diff"
        );
        Ok(())
    }

    /// Craft review: fetch the crafter's unpushed work branch from the
    /// workspace into the bare cache, then diff against the source branch.
    ///
    /// The crafter workspace has its alternates rewritten to a container-side
    /// path, so running git directly against the workspace fails on the host.
    /// Fetching *from* the workspace still works because `git fetch` only
    /// transfers objects the cache is missing, and since the workspace was
    /// created from that same cache via `clone --shared`, the ancestor
    /// objects are already present — only the crafter's new commits need
    /// to move across.
    fn generate_craft_diff(&self, review_job: &Job, diff_dir: &Path) -> crate::Result<()> {
        let task_state = self
            .interactor
            .data_store
            .get_task_state(&review_job.task_id)?
            .ok_or_else(|| crate::Error::TaskNotFound {
                task_id: review_job.task_id.clone(),
            })?;
        let task_store = self.interactor.create_task_store(&task_state.workflow_id)?;
        let anchor = self
            .find_artifact_anchor(&task_store, &review_job.task_id)
            .ok_or_else(|| {
                crate::Error::External(
                    format!(
                        "no artifact anchor for review job {}; cannot generate craft diff",
                        review_job.id
                    )
                    .into(),
                )
            })?;

        let repository = anchor.detail.repository().ok_or_else(|| {
            crate::Error::External(
                format!(
                    "anchor job {} has no repository; cannot generate craft diff",
                    anchor.id
                )
                .into(),
            )
        })?;

        let source_branch = repository
            .source_branch
            .clone()
            .unwrap_or_else(|| resolve_default_branch(&self.workspace_manager, repository));

        let workspace_path = self.workspace_manager.workspace_path(anchor.id.as_ref());
        let cache_path = self.workspace_manager.repo_cache_path(repository);

        if !workspace_path.exists() {
            // In production the workspace is always on disk by the time a
            // review runs (the crafter just finished). This branch exists
            // for test harnesses that simulate the state machine without
            // real workspace setup — write placeholder files so downstream
            // container mounts succeed and log a warning so real misses in
            // prod are visible in the logs.
            tracing::warn!(
                path = %workspace_path.display(),
                "craft workspace missing; writing empty diff placeholder"
            );
            write_empty_diff(diff_dir)?;
            return Ok(());
        }

        let workspace_abs = std::fs::canonicalize(&workspace_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;

        // Namespace the fetched ref by craft job so concurrent workflows on
        // the same repo don't clobber each other's work branch in the cache.
        let work_ref = format!("refs/palette/work/{}", anchor.id);
        let fetch_spec = format!("+{}:{}", repository.work_branch, work_ref);
        run_git(
            &cache_path,
            &[
                "fetch",
                "--no-tags",
                &workspace_abs.to_string_lossy(),
                &fetch_spec,
            ],
            "fetch craft work branch",
        )?;

        let diff_range = format!("{source_branch}...{work_ref}");
        write_diff_outputs(&cache_path, &diff_range, diff_dir, "craft")?;

        tracing::info!(
            job_id = %review_job.id,
            anchor_id = %anchor.id,
            source = %source_branch,
            work_ref = %work_ref,
            diff_dir = %diff_dir.display(),
            "generated craft diff"
        );
        Ok(())
    }
}

/// Host-side best-effort default branch resolution from the bare cache's
/// `origin/HEAD` symbolic-ref, used only when `Repository.source_branch` is
/// `None`.
fn resolve_default_branch(manager: &WorkspaceManager, repo: &Repository) -> String {
    let cache = manager.repo_cache_path(repo);
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(&cache)
        .output();
    if let Ok(out) = output
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Some(branch) = s.strip_prefix("origin/") {
            return branch.to_string();
        }
    }
    "main".to_string()
}

fn write_diff_outputs(
    cwd: &Path,
    diff_range: &str,
    diff_dir: &Path,
    kind: &str,
) -> crate::Result<()> {
    write_git_output(
        cwd,
        &["diff", diff_range],
        &diff_dir.join(DIFF_PATCH_FILE),
        &format!("git diff ({kind})"),
    )?;
    write_git_output(
        cwd,
        &["diff", "--name-only", diff_range],
        &diff_dir.join(CHANGED_FILES_FILE),
        &format!("git diff --name-only ({kind})"),
    )?;
    Ok(())
}

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

fn write_empty_diff(diff_dir: &Path) -> crate::Result<()> {
    std::fs::write(diff_dir.join(DIFF_PATCH_FILE), "")
        .map_err(|e| crate::Error::External(Box::new(e)))?;
    std::fs::write(diff_dir.join(CHANGED_FILES_FILE), "")
        .map_err(|e| crate::Error::External(Box::new(e)))?;
    Ok(())
}

fn write_git_output(
    cwd: &Path,
    args: &[&str],
    out_path: &Path,
    description: &str,
) -> crate::Result<()> {
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
    let tmp_path = out_path.with_extension("tmp");
    std::fs::write(&tmp_path, &output.stdout).map_err(|e| crate::Error::External(Box::new(e)))?;
    std::fs::rename(&tmp_path, out_path).map_err(|e| crate::Error::External(Box::new(e)))?;
    Ok(())
}
