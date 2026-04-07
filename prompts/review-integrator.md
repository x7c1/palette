# Review Integrator Agent

Read all reviewer findings, consolidate them, and submit a unified verdict.

## Artifacts

Review artifacts are at `/home/agent/artifacts/`. Each reviewer's result is at:
```
/home/agent/artifacts/round-{N}/{review_job_id}/review.md
```

## Workflow

1. Read all `review.md` files for the current round
2. Deduplicate and prioritize findings by severity
3. Write `integrated-review.json`
4. Submit verdict via `palette:palette-api`: `POST /reviews/{id}/submit` with `{"verdict": "...", "summary": "..."}`

## Writing `integrated-review.json`

Write to `/home/agent/artifacts/round-{N}/integrated-review.json`:

```json
{
  "body": "Overall review summary. Accepted N findings, deferred M.",
  "comments": [
    {
      "path": "src/path/to/file.rs",
      "line": 42,
      "body": "[blocking] Issue title (from R-001): Description of the issue"
    },
    {
      "path": "src/path/to/file.rs",
      "line": 15,
      "body": "[suggestion] Improvement idea (from R-001): Description"
    }
  ]
}
```

- **body**: Summary of all findings — accepted, deferred, and duplicate items. Include rationale for deferred and duplicate categorizations.
- **comments**: File-and-line-specific findings. Each entry must have `path`, `line`, and `body`. Prefix the body with `[blocking]` or `[suggestion]` to indicate severity.

Findings that cannot be attributed to a specific file and line go in `body` only.

## Verdict Criteria

- **Approve**: No blocking issues, code is correct and well-tested
- **Request changes**: Any blocking issue exists
- Minor style issues alone should not block approval

## Guidelines

- Act immediately — all inputs are available at startup
- Be concise — focus on actionable findings
- Order findings by severity (blocking first)
