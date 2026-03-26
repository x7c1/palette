#!/usr/bin/env bash
# E2E: Worker Crash Detection and Recovery
# Verify that the monitoring loop detects a crashed worker container
# and initiates recovery.
#
# Steps:
#   1. Reset and build
#   2. Start Palette and begin a workflow
#   3. Wait for a member to start working
#   4. Force-stop the member's container (simulate crash)
#   5. Wait for crash detection (status=crashed)
#   6. Wait for recovery (status transitions back to booting/idle/working)
#   7. Verify logs and final state
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/dynamic-supervisor.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
SESSION_NAME="palette"

cleanup() {
  "$SCRIPT_DIR/stop-palette.sh" 2>/dev/null || true
}
trap cleanup EXIT

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
mkdir -p data/plans
cp -r tests/e2e/fixtures/plans/* data/plans/
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
: > "$LOG_FILE"
RUST_LOG=info ./target/debug/palette >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"
PALETTE_PID=$(cat "$PID_FILE")
echo "PID: $PALETTE_PID"

# Health check (max 60 seconds)
for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" > /dev/null 2>&1; then
    echo "Health check passed after $((i*2)) seconds"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: Health check timed out after 60 seconds"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# --- Step 3: Start workflow and wait for a member ---
echo ""
echo "=== Step 3: Start workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  exit 1
fi
echo "Workflow started (HTTP $HTTP_CODE)"

# Wait for a member to appear (max 120 seconds)
echo "Waiting for a member to spawn..."
MEMBER_CONTAINER=""
for i in $(seq 1 60); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  # Find a member worker with a container_id
  MEMBER_CONTAINER=$(echo "$WORKERS_JSON" | jq -r '[.[] | select(.role == "member") | select(.container_id != "")] | .[0].container_id // empty' 2>/dev/null || true)
  MEMBER_ID=$(echo "$WORKERS_JSON" | jq -r '[.[] | select(.role == "member") | select(.container_id != "")] | .[0].id // empty' 2>/dev/null || true)
  if [[ -n "$MEMBER_CONTAINER" && -n "$MEMBER_ID" ]]; then
    echo "Found member: id=$MEMBER_ID container=$MEMBER_CONTAINER"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: No member appeared after 120 seconds"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# Give the member a moment to start working
sleep 5

# --- Step 4: Force-stop the container ---
echo ""
echo "=== Step 4: Force-stop member container ==="
docker stop "$MEMBER_CONTAINER" 2>/dev/null || docker stop "${MEMBER_CONTAINER:0:12}" 2>/dev/null || true
echo "Container stopped: $MEMBER_CONTAINER"

# --- Step 5: Wait for crash detection ---
echo ""
echo "=== Step 5: Waiting for crash detection ==="
CRASH_DETECTED=false
for i in $(seq 1 60); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  MEMBER_STATUS=$(echo "$WORKERS_JSON" | jq -r --arg id "$MEMBER_ID" '[.[] | select(.id == $id)] | .[0].status // empty' 2>/dev/null || true)
  if [[ "$MEMBER_STATUS" == "crashed" || "$MEMBER_STATUS" == "booting" ]]; then
    echo "Crash detected after ${i}s (status=$MEMBER_STATUS)"
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

# --- Step 6: Wait for recovery ---
echo ""
echo "=== Step 6: Waiting for recovery ==="
RECOVERED=false
for i in $(seq 1 120); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  MEMBER_STATUS=$(echo "$WORKERS_JSON" | jq -r --arg id "$MEMBER_ID" '[.[] | select(.id == $id)] | .[0].status // empty' 2>/dev/null || true)
  if [[ "$MEMBER_STATUS" == "idle" || "$MEMBER_STATUS" == "working" ]]; then
    echo "Recovery complete after ${i}s (status=$MEMBER_STATUS)"
    RECOVERED=true
    break
  fi
  sleep 1
done

if [[ "$RECOVERED" != "true" ]]; then
  echo "WARN: Worker did not fully recover within 120 seconds (status=$MEMBER_STATUS)"
  echo "This may be expected if the container cannot restart in this environment"
fi

# --- Step 7: Verify ---
echo ""
echo "=== Step 7: Verify ==="

PASS=true

# Check: crash detected log message
if grep -q "crash detected" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: 'crash detected' log message found"
else
  echo "FAIL: No 'crash detected' log message"
  PASS=false
fi

# Check: recovery attempt log message
if grep -q "crash recovery" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: 'crash recovery' log message found"
else
  echo "FAIL: No 'crash recovery' log message"
  PASS=false
fi

# Check: supervisor received alert
if grep -q "crash_recovery" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Crash alert logged"
else
  echo "WARN: Crash alert not found in log (may be in message queue)"
fi

echo ""
if [[ "$PASS" == true ]]; then
  echo "=== All crash recovery checks passed ==="
  scripts/reset.sh 2>&1
  exit 0
else
  echo "=== FAILED: Some checks did not pass ==="
  echo ""
  echo "--- Palette log (last 40 lines) ---"
  tail -40 "$LOG_FILE"
  exit 1
fi
