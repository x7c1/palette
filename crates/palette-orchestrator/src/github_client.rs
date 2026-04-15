use palette_usecase::{DiffFile, DiffHunk, GitHubReviewPort, ReviewEvent, ReviewFileComment};
use std::process::Command;

/// GitHub review client that uses the `gh` CLI.
///
/// Relies on the host's `gh` authentication (e.g., `gh auth login`).
/// No explicit token management needed.
#[derive(Default)]
pub struct GhCliReviewClient;

impl GhCliReviewClient {
    pub fn boxed() -> Box<dyn GitHubReviewPort> {
        Box::new(Self)
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

    fn get_diff_files(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<DiffFile>, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("gh")
            .args(["api", &format!("repos/{owner}/{repo}/pulls/{number}/files")])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("gh api failed: {stderr}").into());
        }

        let items: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
        let mut diff_files = Vec::new();
        for item in &items {
            let filename = item["filename"].as_str().unwrap_or_default().to_string();
            let patch = item["patch"].as_str().unwrap_or_default();
            let hunks = parse_hunk_ranges(patch);
            diff_files.push(DiffFile { filename, hunks });
        }
        Ok(diff_files)
    }
}

fn parse_hunk_ranges(patch: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    for line in patch.lines() {
        if !line.starts_with("@@") {
            continue;
        }
        // Parse "@@ -a,b +c,d @@" — extract c and d from the "+" side
        let Some(plus_pos) = line.find('+') else {
            continue;
        };
        let after_plus = &line[plus_pos + 1..];
        let end = after_plus
            .find([' ', '@'])
            .unwrap_or(after_plus.len());
        let range_str = &after_plus[..end];
        let (start, count) = if let Some((s, c)) = range_str.split_once(',') {
            (s.parse::<u64>().ok(), c.parse::<u64>().ok())
        } else {
            (range_str.parse::<u64>().ok(), Some(1))
        };
        if let (Some(start), Some(count)) = (start, count) {
            hunks.push(DiffHunk {
                start_line: start,
                line_count: count,
            });
        }
    }
    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_hunk_header() {
        let patch = "@@ -16,7 +16,7 @@ use something;\n-old\n+new\n@@ -38,6 +38,7 @@ struct Foo {\n+    field,";
        let hunks = parse_hunk_ranges(patch);
        assert_eq!(hunks.len(), 2);
        assert_eq!((hunks[0].start_line, hunks[0].line_count), (16, 7));
        assert_eq!((hunks[1].start_line, hunks[1].line_count), (38, 7));
    }

    #[test]
    fn parse_new_file_hunk() {
        let patch = "@@ -0,0 +1,54 @@\n+line1\n+line2";
        let hunks = parse_hunk_ranges(patch);
        assert_eq!(hunks.len(), 1);
        assert_eq!((hunks[0].start_line, hunks[0].line_count), (1, 54));
    }

    #[test]
    fn parse_single_line_hunk() {
        let patch = "@@ -1 +1 @@\n-old\n+new";
        let hunks = parse_hunk_ranges(patch);
        assert_eq!(hunks.len(), 1);
        assert_eq!((hunks[0].start_line, hunks[0].line_count), (1, 1));
    }

    #[test]
    fn diff_file_contains_line() {
        let file = DiffFile {
            filename: "foo.rs".to_string(),
            hunks: vec![
                DiffHunk {
                    start_line: 10,
                    line_count: 5,
                },
                DiffHunk {
                    start_line: 30,
                    line_count: 3,
                },
            ],
        };
        assert!(file.contains_line(10));
        assert!(file.contains_line(14));
        assert!(!file.contains_line(15));
        assert!(!file.contains_line(20));
        assert!(file.contains_line(30));
        assert!(file.contains_line(32));
        assert!(!file.contains_line(33));
    }
}
