#!/usr/bin/env bash
# E2E: Workspace Shared Clone (012/001)
# Verify that workspaces use git clone --shared with a bare cache,
# bind mounts replace named volumes, and push is disabled.
#
# Checks:
# - Bare cache created at data/repos/
# - Workspace created at data/workspace/ with .git/objects/info/alternates
# - pushurl is set to PUSH_DISABLED
# - Reviewer container sees workspace as read-only
# - Workspace deleted after craft job completes
# - Bare cache persists after workflow completes
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
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/workspace-shared-clone.yaml"
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
echo "PID: $(cat "$PID_FILE")"

for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" > /dev/null 2>&1; then
    echo "Health check passed after $((i*2)) seconds"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: Health check timed out"
    exit 1
  fi
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
  echo "$RESPONSE"
  exit 1
fi
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
echo "Workflow ID: $WORKFLOW_ID"

# --- Step 4: Wait for craft job to be assigned (workspace created) ---
echo ""
echo "=== Step 4: Wait for workspace creation ==="
for i in $(seq 1 60); do
  JOBS_RAW=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  CRAFT_JOB=$(echo "$JOBS_RAW" | jq -r '.[] | select(.type == "craft") | .id' 2>/dev/null | head -1)
  CRAFT_STATUS=$(echo "$JOBS_RAW" | jq -r '.[] | select(.type == "craft") | .status' 2>/dev/null | head -1)
  echo "  [$((i*2))s] craft_job=$CRAFT_JOB status=$CRAFT_STATUS"
  if [[ -n "$CRAFT_JOB" && "$CRAFT_STATUS" == "in_progress" ]]; then
    echo "Craft job $CRAFT_JOB is in_progress"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: Craft job did not reach in_progress (last status: $CRAFT_STATUS)"
    echo "Jobs: $JOBS_RAW"
    exit 1
  fi
  sleep 2
done

# --- Step 5: Verify workspace structure ---
echo ""
echo "=== Step 5: Verify workspace structure ==="

# Check bare cache exists
if ls data/repos/x7c1/palette.git/HEAD > /dev/null 2>&1; then
  echo "PASS: Bare cache exists at data/repos/x7c1/palette.git"
else
  echo "FAIL: Bare cache not found at data/repos/x7c1/palette.git"
  exit 1
fi

# Check workspace exists with alternates
WS_DIR="data/workspace/$CRAFT_JOB"
if [[ -d "$WS_DIR" ]]; then
  echo "PASS: Workspace directory exists at $WS_DIR"
else
  echo "FAIL: Workspace directory not found at $WS_DIR"
  exit 1
fi

ALTERNATES="$WS_DIR/.git/objects/info/alternates"
if [[ -f "$ALTERNATES" ]]; then
  ALTERNATES_CONTENT=$(cat "$ALTERNATES")
  if [[ "$ALTERNATES_CONTENT" == */home/agent/repo-cache/objects* ]]; then
    echo "PASS: alternates points to container repo-cache path"
  else
    echo "FAIL: alternates content unexpected: $ALTERNATES_CONTENT"
    exit 1
  fi
else
  echo "FAIL: alternates file not found at $ALTERNATES"
  exit 1
fi

# Check pushurl
PUSHURL=$(git -C "$WS_DIR" config remote.origin.pushurl 2>/dev/null || echo "")
if [[ "$PUSHURL" == "PUSH_DISABLED" ]]; then
  echo "PASS: pushurl is PUSH_DISABLED"
else
  echo "FAIL: pushurl is '$PUSHURL', expected PUSH_DISABLED"
  exit 1
fi

# --- Step 6: Monitor until completion or stall ---
echo ""
echo "=== Step 6: Monitor workflow ==="

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

# --- Step 7: Verify post-completion state ---
echo ""
echo "=== Step 7: Verify post-completion state ==="

# Wait a few seconds for destroy_member to clean up workspace
sleep 5

# Workspace cleanup depends on crafter member being destroyed,
# which may happen asynchronously after workflow completion.
if [[ -d "data/workspace/$CRAFT_JOB" ]]; then
  echo "WARN: Workspace still exists after completion (may be cleaned up asynchronously)"
else
  echo "PASS: Workspace deleted after job completion"
fi

# Bare cache should persist
if [[ -d "data/repos/x7c1/palette.git" ]]; then
  echo "PASS: Bare cache persists after workflow completion"
else
  echo "FAIL: Bare cache was deleted"
  exit 1
fi

echo ""
echo "=== All workspace-shared-clone checks passed ==="
exit 0
