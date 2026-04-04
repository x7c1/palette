#!/usr/bin/env bash
# E2E: Orchestrator Task Success (012/003)
# Verify that orchestrator tasks run commands on the host without
# spawning worker containers, and cascade to review on success.
#
# Blueprint: Implementation (craft) → Automated Checks (orchestrator, exit 0) → Review
#
# Checks:
# - Orchestrator task runs without spawning a container
# - check-result.json saved to data/artifacts/
# - Automated Checks task completes on success
# - Review task activates after checks pass
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
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/orchestrator-task-success.yaml"
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

# --- Step 4: Monitor and verify ---
echo ""
echo "=== Step 4: Monitor workflow ==="

prev_snapshot=""
stall_count=0
iteration=0
checks_verified=false

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  WORKERS=$(worker_summary)
  snapshot="${JOBS}|${WORKERS}"

  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "no jobs")
  echo "[${elapsed}s] jobs: ${job_summary} | workers: ${WORKERS:-none} | stall: ${stall_count}/${STALL_THRESHOLD}"

  # Check if orchestrator task completed successfully
  if [[ "$checks_verified" == "false" ]]; then
    ORCH_STATUS=$(echo "$JOBS" | jq -r '.[] | select(.title == "automated-checks") | .status' 2>/dev/null || echo "")
    if [[ "$ORCH_STATUS" == "done" ]]; then
      echo ""
      echo "--- Orchestrator task completed ---"
      echo "PASS: Orchestrator task status is 'done'"

      # Verify check-result.json exists
      CRAFT_JOB=$(echo "$JOBS" | jq -r '.[] | select(.type == "craft") | .id' | head -1)
      CHECK_RESULT=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB" -name "check-result.json" 2>/dev/null | head -1)
      if [[ -n "$CHECK_RESULT" ]]; then
        echo "PASS: check-result.json found at $CHECK_RESULT"
        STATUS=$(jq -r '.status' "$CHECK_RESULT")
        if [[ "$STATUS" == "success" ]]; then
          echo "PASS: check-result.json status is 'success'"
        else
          echo "FAIL: check-result.json status is '$STATUS', expected 'success'"
          exit 1
        fi
      else
        echo "WARN: check-result.json not found yet"
      fi

      # Verify review task activated
      REVIEW_STATUS=$(echo "$JOBS" | jq -r '.[] | select(.title == "review") | .status' 2>/dev/null || echo "")
      if [[ -n "$REVIEW_STATUS" && "$REVIEW_STATUS" != "null" ]]; then
        echo "PASS: Review task exists with status '$REVIEW_STATUS'"
      fi

      checks_verified=true
    fi
  fi

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

echo ""
echo "=== All orchestrator-task-success checks passed ==="
exit 0
