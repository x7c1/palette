#!/usr/bin/env bash
# E2E: Standalone PR Review
# Verify that a PR review workflow can be started without a Crafter or Blueprint,
# and that the resulting structure is correct.
#
# Uses merged PR x7c1/palette#44 as the review target.
# Does NOT spawn worker containers — validates the API and task/job structure only.
#
# Checks:
# - POST /workflows/start-pr-review creates workflow and tasks
# - Two reviewers with different perspectives (architecture, type-safety)
# - Task count is correct (root + review-integrate + 2 reviewers)
# - No Craft jobs are created (standalone)
# - Review and ReviewIntegrate jobs are created
# - A PENDING review with inline comments can be posted to the PR via GitHub API
# - The PENDING review can be deleted (no trace left)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"

# Target: merged PR x7c1/palette#44 (refactor: introduce JobDetail enum)
PR_OWNER="x7c1"
PR_REPO="palette"
PR_NUMBER=44
PR_COMMIT="17270af7c38ad32185d420eb07f2ba0b13407640"
# A file and line known to be in the PR diff
PR_COMMENT_PATH="crates/palette-domain/src/job/job_detail.rs"
PR_COMMENT_LINE=10

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
RUST_LOG="${RUST_LOG:-info}" cargo run >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" > /dev/null 2>&1; then
    echo "Health check passed"
    break
  fi
  if [[ $i -eq 30 ]]; then echo "FAIL: Health check timed out"; exit 1; fi
  sleep 2
done

# --- Step 3: Start PR review workflow ---
echo ""
echo "=== Step 3: Start PR review workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start-pr-review" \
  -H "Content-Type: application/json" \
  -d "{
    \"owner\": \"$PR_OWNER\",
    \"repo\": \"$PR_REPO\",
    \"number\": $PR_NUMBER,
    \"reviewers\": [
      {\"perspective\": \"architecture\"},
      {\"perspective\": \"type-safety\"}
    ]
  }")
RESPONSE=$(cat /tmp/palette-e2e-response.json)

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start-pr-review returned HTTP $HTTP_CODE"
  echo "Response: $RESPONSE"
  exit 1
fi
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
TASK_COUNT=$(echo "$RESPONSE" | jq -r '.task_count')
echo "Workflow ID: $WORKFLOW_ID"
echo "Task count: $TASK_COUNT"

if [[ "$TASK_COUNT" -ne 4 ]]; then
  echo "FAIL: Expected 4 tasks (root + review-integrate + 2 reviews), got $TASK_COUNT"
  exit 1
fi
echo "PASS: Task count is 4"

# --- Step 4: Verify jobs ---
echo ""
echo "=== Step 4: Verify jobs ==="
sleep 3  # allow time for task activation

JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
REVIEW_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "review")] | length')
RI_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "review_integrate")] | length')
CRAFT_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "craft")] | length')

echo "Review jobs: $REVIEW_COUNT, ReviewIntegrate jobs: $RI_COUNT, Craft jobs: $CRAFT_COUNT"

if [[ "$CRAFT_COUNT" -ne 0 ]]; then
  echo "FAIL: Expected 0 Craft jobs (standalone PR review), got $CRAFT_COUNT"
  exit 1
fi
echo "PASS: No Craft jobs (standalone)"

if [[ "$REVIEW_COUNT" -ne 2 ]]; then
  echo "FAIL: Expected 2 Review jobs, got $REVIEW_COUNT"
  exit 1
fi
echo "PASS: 2 Review jobs created"

if [[ "$RI_COUNT" -ne 1 ]]; then
  echo "FAIL: Expected 1 ReviewIntegrate job, got $RI_COUNT"
  exit 1
fi
echo "PASS: 1 ReviewIntegrate job created"

# --- Step 5: Verify PENDING review on GitHub PR ---
echo ""
echo "=== Step 5: Verify PENDING review with inline comment ==="

REVIEW_RESPONSE=$(gh api "repos/$PR_OWNER/$PR_REPO/pulls/$PR_NUMBER/reviews" -X POST \
  --input - <<JSON
{
  "commit_id": "$PR_COMMIT",
  "body": "E2E test: standalone PR review pending review",
  "comments": [
    {
      "path": "$PR_COMMENT_PATH",
      "line": $PR_COMMENT_LINE,
      "body": "[blocking] E2E test inline comment"
    }
  ]
}
JSON
)

REVIEW_ID=$(echo "$REVIEW_RESPONSE" | jq -r '.id')
REVIEW_STATE=$(echo "$REVIEW_RESPONSE" | jq -r '.state')

if [[ "$REVIEW_STATE" != "PENDING" ]]; then
  echo "FAIL: Expected PENDING review, got $REVIEW_STATE"
  exit 1
fi
echo "PASS: Created PENDING review (id: $REVIEW_ID) with inline comment on merged PR #$PR_NUMBER"

# Clean up: delete the PENDING review (leaves no trace)
DELETE_STATE=$(gh api "repos/$PR_OWNER/$PR_REPO/pulls/$PR_NUMBER/reviews/$REVIEW_ID" -X DELETE --jq '.state')
if [[ "$DELETE_STATE" != "PENDING" ]]; then
  echo "WARN: Delete returned unexpected state: $DELETE_STATE"
else
  echo "PASS: Deleted PENDING review (no trace left on PR)"
fi

# --- Step 6: Verify no panics ---
echo ""
echo "=== Step 6: Verify server health ==="

if grep -q "panic" "$LOG_FILE" 2>/dev/null; then
  echo "FAIL: Panic detected in logs"
  grep "panic" "$LOG_FILE" | tail -5
  exit 1
fi
echo "PASS: No panics in logs"

echo ""
echo "=== All standalone-pr-review checks passed ==="
exit 0
