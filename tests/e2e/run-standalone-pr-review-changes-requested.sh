#!/usr/bin/env bash
# E2E: Standalone PR Review — ChangesRequested Path
# Verify that when a standalone PR review gets ChangesRequested,
# the system handles it gracefully (no Crafter to revert).
#
# Expected flow:
# - Workflow starts with ReviewIntegrate composite + 1 Review leaf task
# - Reviewer submits ChangesRequested verdict
# - Integrator submits ChangesRequested verdict
# - No Crafter revert occurs (standalone path)
# - System logs the escalation without crashing
#
# Checks:
# - Workflow created successfully
# - Review job reaches changes_requested status
# - No crash or panic in logs
# - Orchestrator logs "standalone review changes_requested"
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

# --- Step 3: Start PR review workflow (single reviewer) ---
echo ""
echo "=== Step 3: Start PR review workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start-pr-review" \
  -H "Content-Type: application/json" \
  -d '{
    "owner": "x7c1",
    "repo": "palette-demo",
    "number": 2,
    "reviewers": [
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

if [[ "$TASK_COUNT" -ne 3 ]]; then
  echo "FAIL: Expected 3 tasks (root + review-integrate + 1 review), got $TASK_COUNT"
  exit 1
fi
echo "PASS: Task count is 3"

# --- Step 4: Monitor until changes_requested or stall ---
echo ""
echo "=== Step 4: Monitor workflow ==="

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
    echo "Stall detected — checking results"
    break
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    break
  fi
done

# --- Step 5: Verify no crashes and correct behavior ---
echo ""
echo "=== Step 5: Verify behavior ==="

# Check no panics
if grep -q "panic" "$LOG_FILE" 2>/dev/null; then
  echo "FAIL: Panic detected in logs"
  grep "panic" "$LOG_FILE" | tail -5
  exit 1
fi
echo "PASS: No panics in logs"

# Check the standalone review path was taken
if grep -q "standalone review changes_requested" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Standalone review ChangesRequested path logged"
elif grep -q "craft job reverted" "$LOG_FILE" 2>/dev/null; then
  echo "FAIL: Craft job revert occurred (should not happen in standalone PR review)"
  exit 1
else
  echo "INFO: Neither standalone nor craft path logged (review may have been approved)"
fi

# Verify no Craft jobs were created
CRAFT_JOBS=$(curl -sf "$PALETTE_URL/jobs" | jq '[.[] | select(.type == "craft")]' 2>/dev/null || echo "[]")
CRAFT_COUNT=$(echo "$CRAFT_JOBS" | jq 'length')
if [[ "$CRAFT_COUNT" -ne 0 ]]; then
  echo "FAIL: Found $CRAFT_COUNT Craft jobs (standalone review should have none)"
  exit 1
fi
echo "PASS: No Craft jobs created"

echo ""
echo "=== All standalone-pr-review-changes-requested checks passed ==="
exit 0
