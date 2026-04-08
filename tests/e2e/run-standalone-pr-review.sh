#!/usr/bin/env bash
# E2E: Standalone PR Review (full cycle)
# Verify that a PR review workflow runs end-to-end without a Crafter.
# Uses merged PR x7c1/palette#44 as the review target.
#
# Two reviewers with different perspectives (architecture, type-safety)
# review the PR, then the ReviewIntegrator consolidates findings,
# and the orchestrator posts a pending review to the PR via GitHub API.
#
# Checks:
# - Workflow created via start-pr-review endpoint
# - Task count: root + review-integrate + 2 reviewers = 4
# - No Craft jobs created (standalone)
# - review.md files created per reviewer
# - integrated-review.json created and is valid JSON
# - Pending review posted to GitHub PR
# - Workflow completes
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

if [[ "${PALETTE_E2E_IMAGE_CHECK:-1}" == "1" ]]; then
  "$SCRIPT_DIR/check-required-images.sh"
fi

if [[ "${PALETTE_E2E_SYNC_AUTH_BUNDLE:-1}" == "1" ]]; then
  "$ROOT_DIR/scripts/sync-bootstrap-auth-bundle.sh"
fi

PALETTE_URL="http://127.0.0.1:7100"
CONFIG_PATH="$ROOT_DIR/tests/e2e/fixtures/palette-pr-review.toml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD="${STALL_THRESHOLD:-60}"

# Target: merged PR x7c1/palette#44 (refactor: introduce JobDetail enum)
PR_OWNER="x7c1"
PR_REPO="palette"
PR_NUMBER=44

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

# --- Helpers ---
worker_summary() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq -r '[.[] | "\(.id):\(.status)"] | join(" ")' 2>/dev/null \
    || echo ""
}

# --- Step 0: Clean up stale pending reviews on PR ---
echo "=== Step 0: Clean up stale pending reviews ==="
PENDING_REVIEWS=$(gh api "repos/$PR_OWNER/$PR_REPO/pulls/$PR_NUMBER/reviews" --jq '[.[] | select(.state == "PENDING")] | .[].id' 2>/dev/null || true)
for review_id in $PENDING_REVIEWS; do
  echo "Deleting stale pending review $review_id"
  gh api "repos/$PR_OWNER/$PR_REPO/pulls/$PR_NUMBER/reviews/$review_id" -X DELETE > /dev/null 2>&1 || true
done

# --- Step 1: Reset and build ---
echo ""
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
cargo build 2>&1

# --- Step 2: Start Palette with PR review config ---
echo ""
echo "=== Step 2: Start Palette ==="
RUST_LOG="${RUST_LOG:-info,palette_server::permission_timeout=debug}" cargo run -- "$CONFIG_PATH" >> "$LOG_FILE" 2>&1 &
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

# --- Step 4: Verify initial jobs ---
echo ""
echo "=== Step 4: Verify initial jobs ==="

for i in $(seq 1 30); do
  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  REVIEW_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "review")] | length')
  RI_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "review_integrate")] | length')
  CRAFT_COUNT=$(echo "$JOBS" | jq '[.[] | select(.type == "craft")] | length')
  if [[ "$REVIEW_COUNT" -ge 2 ]] && [[ "$RI_COUNT" -ge 1 ]]; then
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: Timed out waiting for jobs (review=$REVIEW_COUNT, ri=$RI_COUNT, craft=$CRAFT_COUNT)"
    exit 1
  fi
  sleep 2
done

echo "Review jobs: $REVIEW_COUNT, ReviewIntegrate jobs: $RI_COUNT, Craft jobs: $CRAFT_COUNT"

if [[ "$CRAFT_COUNT" -ne 0 ]]; then
  echo "FAIL: Expected 0 Craft jobs (standalone), got $CRAFT_COUNT"
  exit 1
fi
echo "PASS: No Craft jobs (standalone)"
echo "PASS: 2 Review jobs created"
echo "PASS: 1 ReviewIntegrate job created"

# --- Step 5: Wait for workflow completion ---
echo ""
echo "=== Step 5: Wait for workflow completion ==="

prev_snapshot=""
stall_count=0
iteration=0

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  WORKERS=$(worker_summary)
  snapshot="${JOBS}|${WORKERS}"

  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "no jobs")
  echo "[${elapsed}s] jobs: ${job_summary} | workers: ${WORKERS:-none} | stall: ${stall_count}/${STALL_THRESHOLD}"

  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall detected — workflow did not complete"
    tail -30 "$LOG_FILE"
    exit 1
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo "Workflow completed"
    break
  fi
done

# --- Step 6: Verify artifacts ---
echo ""
echo "=== Step 6: Verify artifacts ==="

RI_JOB_ID=$(curl -sf "$PALETTE_URL/jobs" | jq -r '.[] | select(.type == "review_integrate") | .id' | head -1)
ARTIFACTS_DIR="data/artifacts/$WORKFLOW_ID/$RI_JOB_ID"

if [[ ! -d "$ARTIFACTS_DIR" ]]; then
  echo "FAIL: Artifacts directory not found: $ARTIFACTS_DIR"
  exit 1
fi
echo "PASS: Artifacts directory exists (anchored to ReviewIntegrate job)"

echo "Artifacts directory contents:"
find "$ARTIFACTS_DIR" -type f | sort | while read -r f; do
  echo "  $f"
done

if [[ ! -d "$ARTIFACTS_DIR/round-1" ]]; then
  echo "FAIL: round-1 directory not found"
  exit 1
fi
echo "PASS: round-1 directory exists"

REVIEW_MD_COUNT=$(find "$ARTIFACTS_DIR/round-1" -name "review.md" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$REVIEW_MD_COUNT" -ge 2 ]]; then
  echo "PASS: Found $REVIEW_MD_COUNT review.md files in round-1"
else
  echo "FAIL: Expected at least 2 review.md files, found $REVIEW_MD_COUNT"
  exit 1
fi

INTEGRATED_JSON="$ARTIFACTS_DIR/round-1/integrated-review.json"
if [[ -f "$INTEGRATED_JSON" ]]; then
  echo "PASS: integrated-review.json exists"
  if python3 -c "import json; json.load(open('$INTEGRATED_JSON'))" 2>/dev/null; then
    echo "PASS: integrated-review.json is valid JSON"
  else
    echo "FAIL: integrated-review.json is not valid JSON"
    exit 1
  fi
else
  echo "FAIL: integrated-review.json not found at $INTEGRATED_JSON"
  echo "Artifact contents:"
  find "$ARTIFACTS_DIR" -type f | sort
  exit 1
fi

# --- Step 7: Verify pending review on GitHub PR ---
echo ""
echo "=== Step 7: Verify pending review on PR ==="

PENDING_REVIEWS=$(gh api "repos/$PR_OWNER/$PR_REPO/pulls/$PR_NUMBER/reviews" --jq '[.[] | select(.state == "PENDING")] | length' 2>/dev/null || echo "0")
if [[ "$PENDING_REVIEWS" -ge 1 ]]; then
  echo "PASS: Found $PENDING_REVIEWS pending review(s) on PR #$PR_NUMBER"
else
  echo "FAIL: No pending review found on PR #$PR_NUMBER"
  grep -E "post.*pr.*review|posted PR|pending|gh api|github" "$LOG_FILE" | tail -10
  exit 1
fi

# --- Step 8: Verify no panics ---
echo ""
echo "=== Step 8: Verify server health ==="

if grep -q "panic" "$LOG_FILE" 2>/dev/null; then
  echo "FAIL: Panic detected in logs"
  grep "panic" "$LOG_FILE" | tail -5
  exit 1
fi
echo "PASS: No panics in logs"

echo ""
echo "=== All standalone-pr-review checks passed ==="
exit 0
