use palette_usecase::github_review::{GitHubReviewPort, ReviewEvent, ReviewFileComment};
use std::process::Command;

/// GitHub review client that uses the `gh` CLI.
pub struct GhCliReviewClient {
    token: String,
}

impl GhCliReviewClient {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

impl GitHubReviewPort for GhCliReviewClient {
    fn post_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
        comments: &[ReviewFileComment],
        _event: ReviewEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Build the JSON payload for GitHub API
        // event is omitted to create a PENDING review (not submitted)
        let comments_json: Vec<serde_json::Value> = comments
            .iter()
            .map(|c| {
                serde_json::json!({
                    "path": c.path,
                    "line": c.line,
                    "body": c.body,
                })
            })
            .collect();

        let payload = serde_json::json!({
            "body": body,
            "comments": comments_json,
        });

        let output = Command::new("gh")
            .args([
                "api",
                &format!("repos/{owner}/{repo}/pulls/{number}/reviews"),
                "-X",
                "POST",
                "--input",
                "-",
            ])
            .env("GH_TOKEN", &self.token)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(payload.to_string().as_bytes())?;
                }
                child.wait_with_output()
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("gh api failed: {stderr}").into());
        }

        let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let review_id = response["id"].as_u64();
        let state = response["state"].as_str().unwrap_or("unknown");

        tracing::info!(
            owner,
            repo,
            number,
            review_id,
            state,
            comments = comments.len(),
            "created pending review via gh CLI"
        );

        Ok(())
    }
}
