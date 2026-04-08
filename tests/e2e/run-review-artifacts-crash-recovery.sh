#!/usr/bin/env bash
# E2E: Review Artifacts Crash Recovery (012/002)
# Verify that review artifacts survive a ReviewIntegrator crash
# and the recovered integrator can read existing review.md files.
#
# Blueprint: Craft + 2 Reviewers (with ReviewIntegrator)
#
# Steps:
#   1. Start workflow, wait for reviewers to write review.md
#   2. Force-stop the ReviewIntegrator container
#   3. Wait for crash detection and recovery
#   4. Verify review.md files survived the crash
#   5. Wait for workflow completion
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
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/review-artifacts.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=60

cleanup() {
  "$SCRIPT_DIR/stop-palette.sh" 2>/dev/null || true
}
trap cleanup EXIT

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
mkdir -p data/plans
cp -r tests/e2e/fixtures/plans/* data/plans/ 2>/dev/null || true
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
: > "$LOG_FILE"
RUST_LOG=info ./target/debug/palette >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"
echo "PID: $(cat "$PID_FILE")"

for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" > /dev/null 2>&1; then
    echo "Health check passed"
    break
  fi
  if [[ $i -eq 30 ]]; then echo "FAIL: Health check timed out"; exit 1; fi
  sleep 2
done

# --- Step 3: Start workflow ---
echo ""
echo "=== Step 3: Start workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  cat /tmp/palette-e2e-response.json
  exit 1
fi
WORKFLOW_ID=$(cat /tmp/palette-e2e-response.json | jq -r '.workflow_id')
echo "Workflow ID: $WORKFLOW_ID"

# --- Step 4: Wait for all review.md files ---
# The blueprint has 2 reviewers. Wait for both to write review.md before
# proceeding, because the RI is only spawned after all reviewers complete.
echo ""
echo "=== Step 4: Wait for all review.md files ==="

EXPECTED_REVIEWS=2
CRAFT_JOB=""
for i in $(seq 1 180); do
  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  CRAFT_JOB=$(echo "$JOBS" | jq -r '.[] | select(.type == "craft") | .id' 2>/dev/null | head -1)

  if [[ -n "$CRAFT_JOB" ]]; then
    REVIEW_COUNT="$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB" -name "review.md" 2>/dev/null | wc -l)" || true
    if [[ "$REVIEW_COUNT" -ge "$EXPECTED_REVIEWS" ]]; then
      echo "Found $REVIEW_COUNT review.md file(s) after $((i*2))s"
      break
    fi
  fi

  if [[ $i -eq 180 ]]; then
    echo "FAIL: Expected $EXPECTED_REVIEWS review.md files, found ${REVIEW_COUNT:-0} after 360 seconds"
    exit 1
  fi
  sleep 2
done

# Record which review.md files exist before crash
PRE_CRASH_REVIEWS=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB" -name "review.md" 2>/dev/null | sort)
echo "Pre-crash review.md files:"
echo "$PRE_CRASH_REVIEWS" | sed 's/^/  /'

# --- Step 5: Wait for ReviewIntegrator and crash it ---
# After the Approver/RI separation, the RI is spawned only after all reviewers
# complete. Poll until the RI appears in the workers list, then crash it.
echo ""
echo "=== Step 5: Wait for ReviewIntegrator and crash it ==="

INTEGRATOR_CONTAINER=""
INTEGRATOR_ID=""
for i in $(seq 1 120); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  INTEGRATOR_CONTAINER=$(echo "$WORKERS_JSON" | jq -r '[.[] | select(.role == "review_integrator")] | .[0].container_id // empty' 2>/dev/null || true)
  INTEGRATOR_ID=$(echo "$WORKERS_JSON" | jq -r '[.[] | select(.role == "review_integrator")] | .[0].id // empty' 2>/dev/null || true)

  if [[ -n "$INTEGRATOR_CONTAINER" ]]; then
    echo "ReviewIntegrator found after ${i}s: id=$INTEGRATOR_ID"
    break
  fi

  if [[ $i -eq 120 ]]; then
    echo "FAIL: ReviewIntegrator not spawned within 120 seconds"
    tail -30 "$LOG_FILE"
    exit 1
  fi
  sleep 1
done

echo "Crashing integrator: id=$INTEGRATOR_ID container=$INTEGRATOR_CONTAINER"
docker stop "$INTEGRATOR_CONTAINER" 2>/dev/null || true
echo "Container stopped"

# --- Step 6: Wait for crash detection ---
echo ""
echo "=== Step 6: Wait for crash detection ==="
CRASH_DETECTED=false
for i in $(seq 1 60); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  STATUS=$(echo "$WORKERS_JSON" | jq -r --arg id "$INTEGRATOR_ID" '[.[] | select(.id == $id)] | .[0].status // empty' 2>/dev/null || true)
  if [[ "$STATUS" == "crashed" || "$STATUS" == "booting" ]]; then
    echo "Crash detected after ${i}s (status=$STATUS)"
    CRASH_DETECTED=true
    break
  fi
  sleep 1
done

if [[ "$CRASH_DETECTED" != "true" ]]; then
  echo "FAIL: Crash not detected within 60 seconds"
  tail -30 "$LOG_FILE"
  exit 1
fi

# --- Step 7: Verify review.md files survived ---
echo ""
echo "=== Step 7: Verify artifacts survived crash ==="

POST_CRASH_REVIEWS=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB" -name "review.md" 2>/dev/null | sort)
if [[ "$PRE_CRASH_REVIEWS" == "$POST_CRASH_REVIEWS" ]]; then
  echo "PASS: review.md files survived the crash"
else
  echo "FAIL: review.md files changed after crash"
  echo "Before: $PRE_CRASH_REVIEWS"
  echo "After:  $POST_CRASH_REVIEWS"
  exit 1
fi

# --- Step 8: Wait for recovery and workflow completion ---
echo ""
echo "=== Step 8: Monitor for recovery and completion ==="

prev_snapshot=""
stall_count=0
iteration=0

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  WORKERS=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq -r '[.[] | "\(.id):\(.status)"] | join(" ")' 2>/dev/null || echo "")
  snapshot="${JOBS}|${WORKERS}"

  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "")
  echo "[${elapsed}s] jobs: ${job_summary} | workers: ${WORKERS:-none} | stall: ${stall_count}/${STALL_THRESHOLD}"

  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall detected after crash recovery"
    tail -30 "$LOG_FILE"
    exit 1
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "PASS: Workflow completed after integrator crash recovery"
    break
  fi
done

echo ""
echo "=== All review-artifacts-crash-recovery checks passed ==="
exit 0
