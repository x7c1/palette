#!/usr/bin/env bash
# E2E Scenario 1: Task Tree Cascade
# Verify dependent tasks resolve correctly through the task tree.
# Task tree:
#   root
#   ├── step-a (composite, no depends_on)
#   │   ├── craft
#   │   └── review (depends_on: craft)
#   └── step-b (composite, depends_on: step-a)
#       ├── craft
#       └── review (depends_on: craft)
#
# Exits with 0 on success, 1 on failure. Cleans up automatically.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/task-tree-cascade.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=12  # 12 * 5s = 60s with no change → stall

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
mkdir -p data/plans
cp -r tests/e2e/fixtures/plans/* data/plans/
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
RUST_LOG=info cargo run >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"
echo "PID: $(cat "$PID_FILE")"

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

# --- Step 3: Approve Blueprint ---
echo ""
echo "=== Step 3: Approve Blueprint ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")
RESPONSE=$(cat /tmp/palette-e2e-response.json)

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  echo "Response body: $RESPONSE"
  echo "Palette log (last 20 lines):"
  tail -20 "$LOG_FILE"
  exit 1
fi
echo "Response (HTTP $HTTP_CODE): $RESPONSE"
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
TASK_COUNT=$(echo "$RESPONSE" | jq -r '.task_count')
echo "Workflow ID: $WORKFLOW_ID"
echo "Task count: $TASK_COUNT"

# root + step-a + step-a/review + step-b + step-b/review = 5
if [[ "$TASK_COUNT" -ne 5 ]]; then
  echo "FAIL: Expected task_count=5, got $TASK_COUNT"
  exit 1
fi
echo "PASS: task_count is 5"

# --- Step 4: Check initial state ---
echo ""
echo "=== Step 4: Check initial Job state ==="
JOBS=$(curl -sf "$PALETTE_URL/jobs")
echo "$JOBS" | jq -r '.[] | "\(.id)\t\(.job_type)\t\(.status)\t\(.title)"'

# step-a/craft should have a ready job
STEP_A_CRAFT_STATUS=$(echo "$JOBS" | jq -r '.[] | select(.title == "craft") | .status' | head -1)
# step-b should not have jobs yet
STEP_B_JOB_COUNT=$(echo "$JOBS" | jq '[.[] | select(.title == "craft")] | length')

if [[ "$STEP_A_CRAFT_STATUS" != "todo" ]]; then
  echo "FAIL: Expected step-a/craft job status=todo, got '$STEP_A_CRAFT_STATUS'"
  exit 1
fi
echo "PASS: step-a/craft Job is todo"

if [[ "$STEP_B_JOB_COUNT" -ne 1 ]]; then
  echo "FAIL: Expected only 1 craft job (step-a), got $STEP_B_JOB_COUNT"
  exit 1
fi
echo "PASS: only step-a/craft Job exists (step-b is pending)"

# --- Step 5: Monitor until completion or stall ---
echo ""
echo "=== Step 5: Monitoring Workflow execution ==="

prev_snapshot=""
stall_count=0
iteration=0

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  # Collect current state
  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  CONTAINERS=$(docker ps --filter label=palette.managed=true --format "{{.ID}} {{.Names}} {{.Status}}" 2>/dev/null || echo "")
  WORKERS=$(worker_summary)

  # Build snapshot for change detection
  snapshot="${JOBS}|${CONTAINERS}|${WORKERS}"

  # Print status line
  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "no jobs")
  echo "[${elapsed}s] jobs: ${job_summary} | containers: $(echo "$CONTAINERS" | wc -l | tr -d ' ') | workers: ${WORKERS:-none} | stall: ${stall_count}/${STALL_THRESHOLD}"

  # Check for stall
  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo ""
    echo "FAIL: Stall detected — no state change for $((STALL_THRESHOLD * POLL_INTERVAL)) seconds"
    echo ""
    echo "--- Last job state ---"
    echo "$JOBS" | jq -r '.[] | "  \(.id) \(.title) \(.status) \(.job_type)"' 2>/dev/null
    echo ""
    echo "--- Worker state ---"
    curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq -r '.[] | "  \(.id) role=\(.role) status=\(.status) task=\(.task_id)"' 2>/dev/null || echo "  (no workers)"
    echo ""
    echo "--- Tmux pane logs ---"
    SESSION_NAME="palette"
    if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
      for pane in $(tmux list-panes -t "$SESSION_NAME" -s -F '#{pane_id}:#{pane_title}' 2>/dev/null); do
        pane_id="${pane%%:*}"
        pane_title="${pane#*:}"
        echo "  [$pane_title ($pane_id)] last 30 lines:"
        tmux capture-pane -t "$pane_id" -p -S -30 2>/dev/null | sed 's/^/    /'
        echo ""
      done
    else
      echo "  no tmux session '$SESSION_NAME'"
    fi
    echo "--- Palette log (last 20 lines) ---"
    tail -20 "$LOG_FILE"
    exit 1
  fi

  # Check completion: workflow completed in Palette log
  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "=== Step 6: Verify final state ==="
    JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
    echo "$JOBS" | jq -r '.[] | "  \(.id) \(.title) \(.status)"' 2>/dev/null
    echo ""
    echo "=== All E2E checks passed ==="
    echo "Workflow completed successfully: step-a → step-b cascade verified."
    echo "Cleaning up..."
    scripts/reset.sh 2>&1
    exit 0
  fi
done
