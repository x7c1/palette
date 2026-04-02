#!/usr/bin/env bash
# E2E: Graceful Shutdown
# Verify that SIGTERM stops all containers, kills the tmux session,
# and removes worker records from the database.
#
# Steps:
#   1. Reset and build
#   2. Start Palette and begin a workflow
#   3. Wait for workers to appear
#   4. Send SIGTERM
#   5. Wait for process to exit
#   6. Verify: no managed containers, no tmux session, no worker records
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/dynamic-supervisor.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
DB_FILE="data/palette.db"
SESSION_NAME="palette"

cleanup() {
  # Fallback cleanup in case the test itself fails before SIGTERM
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

# --- Step 3: Start workflow and wait for workers ---
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

# Wait for at least one worker to appear (max 60 seconds)
echo "Waiting for workers to spawn..."
for i in $(seq 1 30); do
  WORKER_COUNT=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq 'length' 2>/dev/null || echo "0")
  if [[ "$WORKER_COUNT" -gt 0 ]]; then
    echo "Workers active: $WORKER_COUNT"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: No workers appeared after 60 seconds"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# Count containers before shutdown
CONTAINERS_BEFORE=$(docker ps -q --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
echo "Managed containers before shutdown: $CONTAINERS_BEFORE"

# --- Step 4: Send SIGTERM ---
echo ""
echo "=== Step 4: Send SIGTERM ==="
kill "$PALETTE_PID" 2>/dev/null
echo "SIGTERM sent to PID $PALETTE_PID"

# Wait for process to exit (max 60 seconds — docker stop takes up to 10s per container)
for i in $(seq 1 60); do
  if ! kill -0 "$PALETTE_PID" 2>/dev/null; then
    echo "Process exited after ${i}s"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: Process did not exit within 60 seconds"
    kill -9 "$PALETTE_PID" 2>/dev/null || true
    exit 1
  fi
  sleep 1
done
rm -f "$PID_FILE"

# --- Step 5: Verify cleanup ---
echo ""
echo "=== Step 5: Verify cleanup ==="

PASS=true

# Check: no managed containers remaining
CONTAINERS_AFTER=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
if [[ "$CONTAINERS_AFTER" -eq 0 ]]; then
  echo "PASS: No managed containers remaining"
else
  echo "FAIL: $CONTAINERS_AFTER managed containers still exist"
  docker ps -a --filter label=palette.managed=true --format "  {{.ID}} {{.Names}} {{.Status}}" 2>/dev/null
  PASS=false
fi

# Check: tmux session destroyed
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
  echo "FAIL: tmux session '$SESSION_NAME' still exists"
  PASS=false
else
  echo "PASS: tmux session '$SESSION_NAME' does not exist"
fi

# Check: no worker records in DB
# Server has stopped, so we cannot query the API. Instead verify the DB file
# directly. If sqlite3 CLI is available, use it; otherwise skip with a warning.
if [[ -f "$DB_FILE" ]]; then
  if command -v sqlite3 &>/dev/null; then
    WORKER_ROWS=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM workers;" 2>/dev/null || echo "error")
    if [[ "$WORKER_ROWS" == "0" ]]; then
      echo "PASS: No worker records in DB"
    else
      echo "FAIL: $WORKER_ROWS worker records still in DB"
      sqlite3 "$DB_FILE" "SELECT id, status_id, container_id FROM workers;" 2>/dev/null
      PASS=false
    fi
  else
    echo "SKIP: sqlite3 CLI not available, cannot verify DB worker records"
  fi
else
  echo "PASS: DB file does not exist (already cleaned)"
fi

# Check: shutdown log messages
if grep -q "starting graceful shutdown" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Graceful shutdown log message found"
else
  echo "FAIL: No graceful shutdown log message"
  PASS=false
fi

echo ""
if [[ "$PASS" == true ]]; then
  echo "=== All graceful shutdown checks passed ==="
  exit 0
else
  echo "=== FAILED: Some checks did not pass ==="
  echo ""
  echo "--- Palette log (last 30 lines) ---"
  tail -30 "$LOG_FILE"
  exit 1
fi
