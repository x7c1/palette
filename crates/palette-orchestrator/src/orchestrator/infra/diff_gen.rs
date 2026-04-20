//! Generate the review diff that reviewers and integrators mount as
//! `/home/agent/diff/`.
//!
//! Two code paths share output format:
//!
//! - **PR review**: base/head are already on origin; we run git directly on
//!   the bare cache (host-only).
//! - **Craft review**: the work branch only exists inside the crafter's
//!   workspace, which has its `alternates` rewritten to a container-side path
//!   and cannot be driven from the host. We spawn a short-lived `--rm`
//!   container (same image as the crafter) to run git inside and write output
//!   back via a bind mount.

use std::path::{Path, PathBuf};
use std::process::Command;

use palette_domain::job::{Job, PullRequest, Repository};

use super::Orchestrator;
use super::workspace::WorkspaceManager;

/// Prefix used for diff-gen container names so orphan cleanup can find them.
pub const DIFF_GEN_CONTAINER_PREFIX: &str = "palette-diff-gen-";

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
        round: u32,
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
            None => self.generate_craft_diff(review_job, &diff_dir, round)?,
        }

        let diff_abs =
            std::fs::canonicalize(&diff_dir).map_err(|e| crate::Error::External(Box::new(e)))?;
        Ok(diff_abs)
    }

    /// PR review: generate diff directly on the bare cache (host-only).
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

        // Ensure the base ref is present in the bare cache even when origin
        // does not mirror it under refs/heads/* (e.g., topic branches that
        // were deleted after the PR opened).
        let base_spec = format!("+{}:refs/palette/base/{}", refs.base_sha, refs.base_sha);
        run_git(
            &cache_path,
            &["fetch", "--no-tags", "origin", &base_spec],
            "fetch PR base sha",
        )?;

        // `gh api pulls/{number}` returns the current head SHA, but fetching
        // the `refs/pull/{n}/head` explicitly keeps us resilient against
        // later force-pushes that overwrite it.
        let pr_ref = format!("refs/pull/{}/head", pr.number);
        run_git(&cache_path, &["fetch", "origin", &pr_ref], "fetch PR head")?;

        let diff_range = format!("{}...{}", refs.base_sha, refs.head_sha);

        write_git_output(
            &cache_path,
            &["diff", &diff_range],
            &diff_dir.join(DIFF_PATCH_FILE),
            "git diff (PR)",
        )?;
        write_git_output(
            &cache_path,
            &["diff", "--name-only", &diff_range],
            &diff_dir.join(CHANGED_FILES_FILE),
            "git diff --name-only (PR)",
        )?;

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

    /// Craft review: spawn a short-lived `--rm` container to run git inside
    /// the crafter workspace (whose alternates only resolve container-side).
    fn generate_craft_diff(
        &self,
        review_job: &Job,
        diff_dir: &Path,
        round: u32,
    ) -> crate::Result<()> {
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
            // In production the workspace is always on disk by the time
            // a review runs (the crafter just finished). This branch exists
            // for test harnesses that simulate the state machine without
            // real workspace setup — write placeholder files so downstream
            // container mounts succeed and log a warning so real misses
            // in prod are visible in the logs.
            tracing::warn!(
                path = %workspace_path.display(),
                "craft workspace missing; writing empty diff placeholder"
            );
            write_empty_diff(diff_dir)?;
            return Ok(());
        }

        let workspace_abs = std::fs::canonicalize(&workspace_path)
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        let cache_abs =
            std::fs::canonicalize(&cache_path).map_err(|e| crate::Error::External(Box::new(e)))?;
        let diff_abs =
            std::fs::canonicalize(diff_dir).map_err(|e| crate::Error::External(Box::new(e)))?;

        let container_name = format!(
            "{DIFF_GEN_CONTAINER_PREFIX}{job_id}-round-{round}",
            job_id = review_job.id,
        );

        // Single shell invocation inside the container:
        //   - run git diff twice against a base `origin/{source_branch}`
        //   - write via tmp + rename so readers never see partial output
        let source_ref = format!("origin/{source_branch}");
        let script = format!(
            "set -eu\n\
             cd /home/agent/workspace\n\
             git diff {sr}...HEAD > /home/agent/diff-out/{patch}.tmp\n\
             git diff --name-only {sr}...HEAD > /home/agent/diff-out/{changed}.tmp\n\
             mv /home/agent/diff-out/{patch}.tmp /home/agent/diff-out/{patch}\n\
             mv /home/agent/diff-out/{changed}.tmp /home/agent/diff-out/{changed}\n",
            sr = source_ref,
            patch = DIFF_PATCH_FILE,
            changed = CHANGED_FILES_FILE,
        );

        let args = [
            "run",
            "--rm",
            "--name",
            &container_name,
            "--label",
            "palette.managed=true",
            "--label",
            "palette.role=diff-gen",
            "-v",
            &format!("{}:/home/agent/workspace:ro", workspace_abs.display()),
            "-v",
            &format!("{}:/home/agent/repo-cache:ro", cache_abs.display()),
            "-v",
            &format!("{}:/home/agent/diff-out", diff_abs.display()),
            &self.docker_config.member_image,
            "sh",
            "-c",
            &script,
        ];

        tracing::info!(
            container = %container_name,
            workspace = %workspace_abs.display(),
            source_ref = %source_ref,
            "spawning diff-gen container"
        );
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| crate::Error::External(Box::new(e)))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::External(
                format!("diff-gen container failed: {stderr}").into(),
            ));
        }
        Ok(())
    }
}

/// Remove any leftover diff-gen containers on startup.
///
/// Normal diff-gen runs exit with `--rm`, but a crashed orchestrator can
/// leave orphans behind that would block later runs via name collision.
pub fn cleanup_orphan_diff_gen_containers() {
    let list = Command::new("docker")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("name={DIFF_GEN_CONTAINER_PREFIX}"),
            "-q",
        ])
        .output();
    let Ok(output) = list else {
        tracing::warn!("failed to list diff-gen containers during startup cleanup");
        return;
    };
    if !output.status.success() {
        return;
    }
    let ids: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        return;
    }
    tracing::info!(count = ids.len(), "removing orphan diff-gen containers");
    let mut args = vec!["rm".to_string(), "-f".to_string()];
    args.extend(ids);
    let _ = Command::new("docker").args(&args).output();
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
