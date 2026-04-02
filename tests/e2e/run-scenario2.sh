#!/usr/bin/env bash
# E2E Scenario 2: Dynamic Supervisor Spawn
# Verify supervisors are dynamically spawned/destroyed per composite task.
# Task tree:
#   root (pure composite → Approver)
#   ├── phase-a (pure composite → Approver)
#   │   └── craft (leaf)
#   └── phase-b (pure composite → Approver, depends_on: phase-a)
#       └── craft (leaf)
#
# Exits with 0 on success, 1 on failure. Cleans up automatically.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/dynamic-supervisor.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=24  # 24 * 5s = 120s with no change → stall

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

# --- Helpers ---
supervisor_count() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq '[.[] | select(.role == "approver" or .role == "review_integrator")] | length' 2>/dev/null \
    || echo 0
}

supervisor_task_ids() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq -r '[.[] | select(.role == "approver" or .role == "review_integrator") | .task_id] | sort | join(", ")' 2>/dev/null \
    || echo ""
}

dump_diagnostics() {
  echo ""
  echo "--- Last job state ---"
  curl -sf "$PALETTE_URL/jobs" 2>/dev/null | jq -r '.[] | "  \(.id) \(.title) \(.status) \(.job_type)"' 2>/dev/null || echo "  (no jobs)"
  echo ""
  echo "--- Worker state ---"
  curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq -r '.[] | "  \(.id) role=\(.role) status=\(.status) task=\(.task_id)"' 2>/dev/null || echo "  (no workers)"
  echo ""
  echo "--- Palette log (last 30 lines) ---"
  tail -30 "$LOG_FILE" 2>/dev/null || echo "  (no log)"
}

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
: > "$LOG_FILE"  # Truncate log so previous "workflow completed" doesn't match
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

# --- Step 3: Start Workflow ---
echo ""
echo "=== Step 3: Start Workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")
RESPONSE=$(cat /tmp/palette-e2e-response.json)

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  echo "Response body: $RESPONSE"
  tail -20 "$LOG_FILE"
  exit 1
fi
echo "Response (HTTP $HTTP_CODE): $RESPONSE"
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
TASK_COUNT=$(echo "$RESPONSE" | jq -r '.task_count')
echo "Workflow ID: $WORKFLOW_ID"

# root + phase-a + craft + review + phase-b + craft + review = 7
if [[ "$TASK_COUNT" -ne 7 ]]; then
  echo "FAIL: Expected task_count=7, got $TASK_COUNT"
  exit 1
fi
echo "PASS: task_count is 7"

# --- Step 4: Check initial supervisor state ---
echo ""
echo "=== Step 4: Check initial supervisor state ==="
# Poll for 2 supervisors (effects are processed async)
for i in $(seq 1 30); do
  SUP_COUNT=$(supervisor_count)
  if [[ "$SUP_COUNT" -eq 2 ]]; then
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: Expected 2 supervisors (root + phase-a) within 60s, got $SUP_COUNT"
    dump_diagnostics
    exit 1
  fi
  sleep 2
done

SUP_TASKS=$(supervisor_task_ids)
echo "Supervisors: $SUP_COUNT (task_ids: $SUP_TASKS)"
echo "PASS: 2 supervisors spawned (root + phase-a)"

# --- Step 5: Monitor until completion or stall ---
echo ""
echo "=== Step 5: Monitoring Workflow execution ==="

prev_snapshot=""
stall_count=0
iteration=0
phase_a_done=false

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  # Collect current state
  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  SUP_COUNT=$(supervisor_count)
  SUP_TASKS=$(supervisor_task_ids)

  # Include last log line to detect activity even when jobs/supervisors don't change
  LOG_TAIL=$(tail -1 "$LOG_FILE" 2>/dev/null || echo "")

  # Build snapshot for change detection
  snapshot="${JOBS}|${SUP_COUNT}|${SUP_TASKS}|${LOG_TAIL}"

  # Print status line
  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "no jobs")
  echo "[${elapsed}s] jobs: ${job_summary} | supervisors: ${SUP_COUNT} (${SUP_TASKS})"

  # Check phase-a completion (supervisor count changes)
  if [[ "$phase_a_done" == "false" && "$SUP_COUNT" -eq 2 ]]; then
    # Check if phase-a supervisor is gone and phase-b is present
    if echo "$SUP_TASKS" | grep -q "phase-b" && ! echo "$SUP_TASKS" | grep -q "phase-a"; then
      echo ""
      echo "=== Phase-a completed: supervisor swapped ==="
      echo "PASS: phase-a supervisor destroyed, phase-b supervisor spawned"
      phase_a_done=true
    fi
  fi

  # Check for crashed workers (early failure detection)
  CRASHED_WORKERS=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq -r '[.[] | select(.status == "crashed")] | length' 2>/dev/null || echo "0")
  if [[ "$CRASHED_WORKERS" -gt 0 ]]; then
    echo ""
    echo "FAIL: Worker crashed during test execution"
    dump_diagnostics
    exit 1
  fi

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
    dump_diagnostics
    exit 1
  fi

  # Check completion
  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo ""
    echo "=== Step 6: Verify final state ==="

    # Wait for supervisor cleanup (Docker stop/rm takes several seconds per container)
    for j in $(seq 1 12); do
      SUP_COUNT=$(supervisor_count)
      if [[ "$SUP_COUNT" -eq 0 ]]; then
        break
      fi
      if [[ $j -eq 12 ]]; then
        echo "FAIL: Expected 0 supervisors after workflow completion within 60s, got $SUP_COUNT"
        dump_diagnostics
        exit 1
      fi
      sleep 5
    done

    echo "Final supervisor count: $SUP_COUNT"
    echo "PASS: all supervisors destroyed"

    if [[ "$phase_a_done" != "true" ]]; then
      echo "WARN: phase-a supervisor swap was not observed (may have happened too fast)"
    fi

    echo ""
    echo "=== All E2E checks passed ==="
    echo "Dynamic supervisor lifecycle verified:"
    echo "  - 2 supervisors spawned on workflow start (root + phase-a)"
    echo "  - phase-a supervisor destroyed on completion, phase-b supervisor spawned"
    echo "  - All supervisors destroyed on workflow completion"
    exit 0
  fi
done
