use super::Orchestrator;
use palette_domain::job::{JobDetail, JobId, ReviewTarget};
use palette_usecase::github_review::{ReviewEvent, ReviewFileComment};

impl Orchestrator {
    /// Post PR review comments after a ReviewIntegrate verdict for a PR review workflow.
    ///
    /// Reads `integrated-review.json` from the artifacts directory and posts
    /// comments to the GitHub PR via the configured GitHubReviewPort.
    pub(crate) fn post_pr_review_comments(&self, review_job_id: &JobId) {
        let Some(github) = self.github_review.as_ref() else {
            return;
        };

        let job = match self.interactor.data_store.get_job(review_job_id) {
            Ok(Some(j)) => j,
            _ => return,
        };

        // Only for ReviewIntegrate jobs with a PullRequest target
        let pr = match &job.detail {
            JobDetail::ReviewIntegrate {
                target: ReviewTarget::PullRequest(pr),
            } => pr,
            _ => return,
        };

        let task_state = match self.interactor.data_store.get_task_state(&job.task_id) {
            Ok(Some(s)) => s,
            _ => return,
        };
        let task_store = match self.interactor.create_task_store(&task_state.workflow_id) {
            Ok(s) => s,
            Err(_) => return,
        };

        let anchor_job = match self.find_artifact_anchor(&task_store, &job.task_id) {
            Some(j) => j,
            None => return,
        };

        // Determine round from latest submission
        let submissions = match self
            .interactor
            .data_store
            .get_review_submissions(review_job_id)
        {
            Ok(s) => s,
            Err(_) => return,
        };
        let round = submissions.last().map(|s| s.round as u32).unwrap_or(1);
        let verdict = submissions.last().map(|s| s.verdict);

        let artifacts_base = self
            .workspace_manager
            .artifacts_path(task_state.workflow_id.as_ref(), anchor_job.id.as_ref());
        let json_path = artifacts_base
            .join(format!("round-{round}"))
            .join("integrated-review.json");

        if !json_path.exists() {
            tracing::warn!(
                job_id = %review_job_id,
                path = %json_path.display(),
                "integrated-review.json not found, skipping PR comment posting"
            );
            return;
        }

        let content = match std::fs::read_to_string(&json_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "failed to read integrated-review.json");
                return;
            }
        };

        let review: IntegratedReview = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "failed to parse integrated-review.json");
                return;
            }
        };

        let event = match verdict {
            Some(palette_domain::review::Verdict::Approved) => ReviewEvent::Approve,
            _ => ReviewEvent::RequestChanges,
        };

        // Get PR diff files to filter inline comments
        let diff_files: std::collections::HashSet<String> = match github
            .get_diff_files(&pr.owner, &pr.repo, pr.number)
        {
            Ok(files) => files.into_iter().collect(),
            Err(e) => {
                tracing::warn!(error = %e, "failed to get PR diff files, all comments go to body");
                std::collections::HashSet::new()
            }
        };

        // Split comments: diff-eligible go as inline, others fold into body
        let mut inline_comments = Vec::new();
        let mut body_extra = String::new();

        for c in &review.comments {
            if diff_files.contains(&c.path) {
                inline_comments.push(ReviewFileComment {
                    path: c.path.clone(),
                    line: c.line,
                    body: c.body.clone(),
                });
            } else {
                body_extra.push_str(&format!("\n\n**{}:{}** — {}", c.path, c.line, c.body));
            }
        }

        let body = if body_extra.is_empty() {
            review.body.clone()
        } else {
            format!(
                "{}\n\n---\n\nComments on files outside this PR's diff:{body_extra}",
                review.body
            )
        };

        if !body_extra.is_empty() {
            tracing::info!(
                inline = inline_comments.len(),
                folded = review.comments.len() - inline_comments.len(),
                "split comments: inline vs body fallback"
            );
        }

        if let Err(e) = github.post_review(
            &pr.owner,
            &pr.repo,
            pr.number,
            &body,
            &inline_comments,
            event,
        ) {
            tracing::error!(
                error = %e,
                pr = %pr,
                "failed to post PR review comments"
            );
        } else {
            tracing::info!(
                pr = %pr,
                inline = inline_comments.len(),
                "posted pending review"
            );
        }
    }
}

#[derive(serde::Deserialize)]
struct IntegratedReview {
    body: String,
    #[serde(default)]
    comments: Vec<IntegratedReviewComment>,
}

#[derive(serde::Deserialize)]
struct IntegratedReviewComment {
    path: String,
    line: u64,
    body: String,
}
