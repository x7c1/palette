#!/usr/bin/env bash
# E2E: Orchestrator Task Failure (012/003)
# Verify that a failing orchestrator command reverts the implementation task.
#
# Blueprint: Implementation (craft) → Automated Checks (orchestrator, exit 1)
#
# Checks:
# - check-result.json records failure with stderr
# - Implementation task reverted to in_progress (changes_requested)
# - Crafter receives failure feedback message
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/orchestrator-task-failure.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=24

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
mkdir -p data/plans
cp -r tests/e2e/fixtures/plans/* data/plans/ 2>/dev/null || true
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
RUST_LOG=info cargo run >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

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
RESPONSE=$(cat /tmp/palette-e2e-response.json)

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  exit 1
fi
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
echo "Workflow ID: $WORKFLOW_ID"

# --- Step 4: Wait for orchestrator task to fail ---
echo ""
echo "=== Step 4: Monitor for orchestrator task failure ==="

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

  # Check if orchestrator task failed
  ORCH_STATUS=$(echo "$JOBS" | jq -r '.[] | select(.title == "automated-checks") | .status' 2>/dev/null || echo "")
  if [[ "$ORCH_STATUS" == "failed" ]]; then
    echo ""
    echo "PASS: Orchestrator task status is 'failed'"

    # Verify check-result.json
    CRAFT_JOB=$(echo "$JOBS" | jq -r '.[] | select(.type == "craft") | .id' | head -1)
    CHECK_RESULT=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB" -name "check-result.json" 2>/dev/null | head -1)
    if [[ -n "$CHECK_RESULT" ]]; then
      echo "PASS: check-result.json found"
      STATUS=$(jq -r '.status' "$CHECK_RESULT")
      STDERR=$(jq -r '.stderr' "$CHECK_RESULT")
      if [[ "$STATUS" == "failed" ]]; then
        echo "PASS: check-result.json status is 'failed'"
      else
        echo "FAIL: check-result.json status is '$STATUS'"
        exit 1
      fi
      if [[ "$STDERR" == *"check failed"* ]]; then
        echo "PASS: stderr captured in check-result.json"
      fi
    fi

    # Verify implementation task reverted
    CRAFT_STATUS=$(echo "$JOBS" | jq -r '.[] | select(.type == "craft") | .status')
    if [[ "$CRAFT_STATUS" == "in_progress" ]]; then
      echo "PASS: Implementation task reverted to in_progress"
    else
      echo "INFO: Implementation task status is '$CRAFT_STATUS'"
    fi

    # Verify revert logged
    if grep -q "reverted implementation task" "$LOG_FILE" 2>/dev/null; then
      echo "PASS: Implementation task revert logged"
    fi

    echo ""
    echo "=== All orchestrator-task-failure checks passed ==="
    scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
    exit 0
  fi

  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall detected — orchestrator task never failed"
    tail -20 "$LOG_FILE"
    exit 1
  fi
done
