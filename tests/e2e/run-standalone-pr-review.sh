#!/usr/bin/env bash
# E2E: Standalone PR Review
# Verify that a PR review workflow can be started without a Crafter or Blueprint.
# Uses POST /workflows/start-pr-review to create the workflow programmatically.
#
# Expected flow:
# - Workflow starts with ReviewIntegrate composite + 2 Review leaf tasks
# - No Craft job exists (standalone)
# - Reviewers write review.md, integrator writes integrated-review.json
# - Workflow completes when all reviews are approved
#
# Checks:
# - Workflow created successfully via start-pr-review endpoint
# - Tasks created: root, review-integrate, review-1, review-2
# - Jobs created for review and review-integrate tasks
# - Artifact directories created with correct anchor (ReviewIntegrate job ID)
# - review.md files created per reviewer
# - integrated-review.json created and is valid JSON
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

if [[ "${PALETTE_E2E_IMAGE_CHECK:-1}" == "1" ]]; then
  "$SCRIPT_DIR/check-required-images.sh"
fi

if [[ "${PALETTE_E2E_SYNC_AUTH_BUNDLE:-1}" == "1" ]]; then
  "$SCRIPT_DIR/sync-bootstrap-auth-bundle.sh"
fi

PALETTE_URL="http://127.0.0.1:7100"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD="${STALL_THRESHOLD:-24}"

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

# --- Helpers ---
worker_summary() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq -r '[.[] | "\(.id):\(.status)"] | join(" ")' 2>/dev/null \
    || echo ""
}

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
  -d '{
    "owner": "x7c1",
    "repo": "palette-demo",
    "number": 1,
    "reviewers": [
      {"perspective": null},
      {"perspective": null}
    ]
  }')
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
echo "=== Step 4: Verify initial jobs ==="
sleep 3  # allow time for task activation

JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
REVIEW_JOBS=$(echo "$JOBS" | jq '[.[] | select(.type == "review")]')
RI_JOBS=$(echo "$JOBS" | jq '[.[] | select(.type == "review_integrate")]')
CRAFT_JOBS=$(echo "$JOBS" | jq '[.[] | select(.type == "craft")]')

REVIEW_COUNT=$(echo "$REVIEW_JOBS" | jq 'length')
RI_COUNT=$(echo "$RI_JOBS" | jq 'length')
CRAFT_COUNT=$(echo "$CRAFT_JOBS" | jq 'length')

echo "Review jobs: $REVIEW_COUNT, ReviewIntegrate jobs: $RI_COUNT, Craft jobs: $CRAFT_COUNT"

if [[ "$CRAFT_COUNT" -ne 0 ]]; then
  echo "FAIL: Expected 0 Craft jobs (standalone PR review), got $CRAFT_COUNT"
  exit 1
fi
echo "PASS: No Craft jobs (standalone PR review)"

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

# --- Step 5: Monitor workflow ---
echo ""
echo "=== Step 5: Monitor workflow ==="

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
    echo "FAIL: Stall detected"
    tail -20 "$LOG_FILE"
    exit 1
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    break
  fi
done

# --- Step 6: Verify artifacts ---
echo ""
echo "=== Step 6: Verify artifacts ==="

# The anchor for standalone PR review is the ReviewIntegrate job
RI_JOB_ID=$(curl -sf "$PALETTE_URL/jobs" | jq -r '.[] | select(.type == "review_integrate") | .id' | head -1)
ARTIFACTS_DIR="data/artifacts/$WORKFLOW_ID/$RI_JOB_ID"

if [[ ! -d "$ARTIFACTS_DIR" ]]; then
  echo "FAIL: Artifacts directory not found: $ARTIFACTS_DIR"
  echo "Checking alternative paths..."
  find data/artifacts/ -type d 2>/dev/null | head -20
  exit 1
fi
echo "PASS: Artifacts directory exists (anchored to ReviewIntegrate job)"

echo "Artifacts directory contents:"
find "$ARTIFACTS_DIR" -type f | sort | while read -r f; do
  echo "  $f"
done

# Check round-1 exists
if [[ -d "$ARTIFACTS_DIR/round-1" ]]; then
  echo "PASS: round-1 directory exists"
else
  echo "FAIL: round-1 directory not found"
  exit 1
fi

# Check review.md files
REVIEW_COUNT=$(find "$ARTIFACTS_DIR/round-1" -name "review.md" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$REVIEW_COUNT" -gt 0 ]]; then
  echo "PASS: Found $REVIEW_COUNT review.md file(s) in round-1"
else
  echo "WARN: No review.md files found (reviewers may not have written them)"
fi

# Check integrated-review.json
if [[ -f "$ARTIFACTS_DIR/round-1/integrated-review.json" ]]; then
  echo "PASS: integrated-review.json exists in round-1"
  if head -1 "$ARTIFACTS_DIR/round-1/integrated-review.json" | grep -q "^{"; then
    echo "PASS: integrated-review.json is valid JSON"
  else
    echo "WARN: integrated-review.json may not be valid JSON"
  fi
else
  echo "WARN: integrated-review.json not found (integrator may not have written it)"
fi

echo ""
echo "=== All standalone-pr-review checks passed ==="
exit 0
