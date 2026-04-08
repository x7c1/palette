#!/usr/bin/env bash
# E2E: Orphan Container Cleanup
# Verify that Palette detects and removes orphan containers at startup.
#
# Steps:
#   1. Reset and build
#   2. Start Palette and begin a workflow
#   3. Wait for containers to appear
#   4. Force-kill Palette (SIGKILL — no graceful shutdown)
#   5. Confirm orphan containers still exist
#   6. Restart Palette — orphan cleanup should run
#   7. Verify: orphan containers are removed, log records cleanup
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

if [[ "$(uname -s)" == "Darwin" && "${PALETTE_E2E_PREFLIGHT:-1}" == "1" ]]; then
  "$SCRIPT_DIR/check-macos-preflight.sh"
fi

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

# --- Step 3: Start workflow and wait for containers ---
echo ""
echo "=== Step 3: Start workflow and wait for containers ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  exit 1
fi
echo "Workflow started (HTTP $HTTP_CODE)"

# Wait for at least one managed container (max 60 seconds)
echo "Waiting for containers to spawn..."
for i in $(seq 1 30); do
  CONTAINER_COUNT=$(docker ps -q --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$CONTAINER_COUNT" -gt 0 ]]; then
    echo "Managed containers running: $CONTAINER_COUNT"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: No managed containers appeared after 60 seconds"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# --- Step 4: Force-kill Palette (simulate crash) ---
echo ""
echo "=== Step 4: Force-kill Palette (SIGKILL) ==="
kill -9 "$PALETTE_PID" 2>/dev/null || true
sleep 1
rm -f "$PID_FILE"
echo "Process killed with SIGKILL"

# Also kill the tmux session to simulate a clean restart
# (the orphan containers should be the only leftover)
tmux kill-session -t "$SESSION_NAME" 2>/dev/null || true

# --- Step 5: Confirm orphan containers exist ---
echo ""
echo "=== Step 5: Confirm orphan containers exist ==="
ORPHAN_COUNT=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
if [[ "$ORPHAN_COUNT" -eq 0 ]]; then
  echo "FAIL: No orphan containers found after force-kill (expected some)"
  exit 1
fi
echo "PASS: $ORPHAN_COUNT orphan container(s) found after crash"

# Remove DB so workers are unknown to the new instance
# (simulates fresh startup where DB was also lost, or containers outlived DB records)
rm -f data/palette.db data/palette.db-wal data/palette.db-shm

# --- Step 6: Restart Palette ---
echo ""
echo "=== Step 6: Restart Palette (orphan cleanup should run) ==="
: > "$LOG_FILE"
RUST_LOG=info ./target/debug/palette >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"
PALETTE_PID2=$(cat "$PID_FILE")
echo "New PID: $PALETTE_PID2"

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

# Give orphan cleanup a moment to complete
sleep 3

# --- Step 7: Verify orphan cleanup ---
echo ""
echo "=== Step 7: Verify orphan cleanup ==="

PASS=true

# Check: no managed containers remaining
CONTAINERS_AFTER=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
if [[ "$CONTAINERS_AFTER" -eq 0 ]]; then
  echo "PASS: No orphan containers remaining"
else
  echo "FAIL: $CONTAINERS_AFTER managed containers still exist"
  docker ps -a --filter label=palette.managed=true --format "  {{.ID}} {{.Names}} {{.Status}}" 2>/dev/null
  PASS=false
fi

# Check: orphan cleanup logged
if grep -q "removing orphan container" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Orphan cleanup log messages found"
  grep "removing orphan container" "$LOG_FILE" | head -5
else
  echo "FAIL: No orphan cleanup log messages"
  PASS=false
fi

if grep -q "orphan container cleanup complete" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Orphan cleanup completion logged"
else
  echo "WARN: No cleanup completion log (may be 0 orphans by label timing)"
fi

echo ""
if [[ "$PASS" == true ]]; then
  echo "=== All orphan cleanup checks passed ==="
  exit 0
else
  echo "=== FAILED: Some checks did not pass ==="
  echo ""
  echo "--- Palette log (last 30 lines) ---"
  tail -30 "$LOG_FILE"
  exit 1
fi
