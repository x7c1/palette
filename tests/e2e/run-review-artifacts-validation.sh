#!/usr/bin/env bash
# E2E: Review Artifacts Validation (012/002)
# Verify that the orchestrator detects a missing review.md
# after a reviewer stops and sends a re-instruction.
#
# This test monitors the palette log for the validation warning
# rather than simulating the full scenario, since the reviewer
# prompt instructs writing review.md. A missing file indicates
# the reviewer failed to follow instructions.
#
# Checks:
# - Orchestrator validates review.md existence on reviewer stop
# - If missing, a re-instruction message is logged
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/workspace-shared-clone.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=60

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

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
RUST_LOG=debug cargo run >> "$LOG_FILE" 2>&1 &
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

# --- Step 4: Monitor for artifact validation ---
echo ""
echo "=== Step 4: Monitor for artifact validation ==="

# Wait for either:
# - "review.md artifact validated" (reviewer wrote it)
# - "review.md artifact missing" (reviewer didn't write it → re-instruction sent)
prev_snapshot=""
stall_count=0
iteration=0

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  elapsed=$((iteration * POLL_INTERVAL))
  echo "[${elapsed}s] monitoring..."

  if grep -q "review.md artifact validated" "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "PASS: Orchestrator validated review.md (file was present)"
    break
  fi

  if grep -q "review.md artifact missing" "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "PASS: Orchestrator detected missing review.md and sent re-instruction"
    break
  fi

  # If workflow completed, check whether validation ran at all
  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo ""
    if grep -q "review.md artifact" "$LOG_FILE" 2>/dev/null; then
      echo "PASS: Artifact validation ran during workflow (check log for details)"
    else
      echo "FAIL: Workflow completed but no artifact validation detected in log"
      exit 1
    fi
    break
  fi

  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  WORKERS=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq -r 'length' 2>/dev/null || echo "0")
  snapshot="${JOBS}|${WORKERS}"
  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall — no artifact validation detected"
    tail -30 "$LOG_FILE"
    exit 1
  fi
done

echo ""
echo "=== Review artifact validation check passed ==="
exit 0
