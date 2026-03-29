#!/usr/bin/env bash
# E2E: Workflow Suspend, Blueprint Reconciliation, and Resume
# Verify that suspending stops all containers (without removing them),
# marks workers as Suspended, that Blueprint reconciliation validates
# and applies changes during suspend, and that resuming restarts
# containers and resumes Claude Code sessions.
#
# Steps:
#   1. Reset and build
#   2. Start Palette and begin a workflow
#   3. Wait for workers to appear
#   4. Suspend the workflow
#   5. Verify: containers stopped (not removed), workers Suspended, workflow Suspended
#   6. Blueprint Reconciliation:
#      a. Validate invalid change (add child to InProgress subtree) → error
#      b. Validate valid change (add task under Pending phase) → success
#   7. Resume the workflow (with valid Blueprint change)
#   8. Verify: containers running, workers active, workflow Active
#   9. Wait for jobs to complete (including the new task from reconciliation)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/dynamic-supervisor.yaml"
BLUEPRINT_ORIG="$ROOT_DIR/tests/e2e/fixtures/dynamic-supervisor.yaml.orig"
RECONCILE_VALID="$ROOT_DIR/tests/e2e/fixtures/reconcile-valid.yaml"
RECONCILE_INVALID="$ROOT_DIR/tests/e2e/fixtures/reconcile-invalid.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
DB_FILE="data/palette.db"

cleanup() {
  # Restore original Blueprint if we backed it up
  if [[ -f "$BLUEPRINT_ORIG" ]]; then
    cp "$BLUEPRINT_ORIG" "$BLUEPRINT_PATH"
    rm -f "$BLUEPRINT_ORIG"
  fi
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

WORKFLOW_ID=$(jq -r '.workflow_id' /tmp/palette-e2e-response.json)
echo "Workflow started: $WORKFLOW_ID (HTTP $HTTP_CODE)"

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

# Wait for at least one worker to be in Working status via API.
echo "Waiting for a worker to be in Working status..."
for i in $(seq 1 30); do
  WORKING=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq '[.[] | select(.status == "working")] | length' 2>/dev/null || echo "0")
  if [[ "$WORKING" -gt 0 ]]; then
    echo "Worker in Working status after $((i*2)) seconds"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: No worker entered Working status within 60 seconds"
    curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq '.[] | {id, status}' 2>/dev/null
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# --- Step 4: Suspend workflow ---
echo ""
echo "=== Step 4: Suspend workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-suspend-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/$WORKFLOW_ID/suspend")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/$WORKFLOW_ID/suspend returned HTTP $HTTP_CODE"
  cat /tmp/palette-suspend-response.json
  exit 1
fi
echo "Suspend accepted (HTTP $HTTP_CODE)"

# Poll until workflow status is suspended.
echo "Waiting for suspend to complete..."
SEEN_SUSPENDING=false
for i in $(seq 1 60); do
  SUSPENDING_WORKFLOWS=$(curl -sf "$PALETTE_URL/workflows?status=suspending" 2>/dev/null | jq 'length' 2>/dev/null || echo "0")
  SUSPENDED_WORKFLOWS=$(curl -sf "$PALETTE_URL/workflows?status=suspended" 2>/dev/null | jq 'length' 2>/dev/null || echo "0")

  if [[ "$SUSPENDING_WORKFLOWS" -gt 0 && "$SEEN_SUSPENDING" == false ]]; then
    echo "  Workflow entered Suspending state (waiting for tasks to complete)"
    SEEN_SUSPENDING=true
  fi

  if [[ "$SUSPENDED_WORKFLOWS" -gt 0 ]]; then
    echo "Suspend complete after $((i*5)) seconds"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: Suspend did not complete within 300 seconds"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 5
done

# --- Step 5: Verify suspend ---
echo ""
echo "=== Step 5: Verify suspend ==="

PASS=true

# Check: managed containers exist but are stopped (Exited)
RUNNING_CONTAINERS=$(docker ps -q --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
ALL_CONTAINERS=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')

if [[ "$RUNNING_CONTAINERS" -eq 0 && "$ALL_CONTAINERS" -gt 0 ]]; then
  echo "PASS: Containers stopped but not removed ($ALL_CONTAINERS containers in Exited state)"
else
  echo "FAIL: Expected 0 running and >0 total containers, got running=$RUNNING_CONTAINERS total=$ALL_CONTAINERS"
  docker ps -a --filter label=palette.managed=true --format "  {{.ID}} {{.Names}} {{.Status}}" 2>/dev/null
  PASS=false
fi

# Check: all workers have Suspended status in DB
if command -v sqlite3 &>/dev/null && [[ -f "$DB_FILE" ]]; then
  NON_SUSPENDED=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM workers WHERE status_id != 6;" 2>/dev/null || echo "error")
  TOTAL_WORKERS=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM workers;" 2>/dev/null || echo "0")
  if [[ "$NON_SUSPENDED" == "0" && "$TOTAL_WORKERS" -gt 0 ]]; then
    echo "PASS: All $TOTAL_WORKERS workers are Suspended in DB"
  else
    echo "FAIL: $NON_SUSPENDED of $TOTAL_WORKERS workers are not Suspended"
    sqlite3 "$DB_FILE" "SELECT id, status_id FROM workers;" 2>/dev/null
    PASS=false
  fi
else
  echo "SKIP: sqlite3 not available for DB verification"
fi

# Check: workflow status is Suspended via API
WORKFLOW_STATUS=$(curl -sf "$PALETTE_URL/workflows?status=suspended" 2>/dev/null | jq 'length' 2>/dev/null || echo "0")
if [[ "$WORKFLOW_STATUS" -gt 0 ]]; then
  echo "PASS: Workflow status is suspended (via API)"
else
  echo "FAIL: No suspended workflows found via API"
  PASS=false
fi

# Check: suspend log messages
if grep -q "suspend complete" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Suspend complete log message found"
else
  echo "FAIL: No suspend complete log message"
  PASS=false
fi

# --- Step 6: Blueprint Reconciliation ---
echo ""
echo "=== Step 6: Blueprint Reconciliation ==="

# Back up the original Blueprint
cp "$BLUEPRINT_PATH" "$BLUEPRINT_ORIG"

# 6a: Validate invalid change — add child under phase-a (InProgress subtree)
echo "--- 6a: Validate invalid Blueprint change ---"
cp "$RECONCILE_INVALID" "$BLUEPRINT_PATH"

HTTP_CODE=$(curl -s -o /tmp/palette-validate-invalid.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/$WORKFLOW_ID/validate-blueprint")

if [[ "$HTTP_CODE" -ne 200 ]]; then
  echo "FAIL: validate-blueprint returned HTTP $HTTP_CODE (expected 200)"
  cat /tmp/palette-validate-invalid.json
  PASS=false
else
  VALID=$(jq -r '.valid' /tmp/palette-validate-invalid.json)
  ERROR_COUNT=$(jq '.errors | length' /tmp/palette-validate-invalid.json)
  if [[ "$VALID" == "false" && "$ERROR_COUNT" -gt 0 ]]; then
    echo "PASS: Invalid Blueprint correctly rejected ($ERROR_COUNT errors)"
    jq -r '.errors[] | "  \(.task_id): \(.message)"' /tmp/palette-validate-invalid.json
  else
    echo "FAIL: Expected validation to fail, got valid=$VALID errors=$ERROR_COUNT"
    cat /tmp/palette-validate-invalid.json
    PASS=false
  fi
fi

# 6b: Validate valid change — add task under phase-b (Pending)
echo "--- 6b: Validate valid Blueprint change ---"
cp "$RECONCILE_VALID" "$BLUEPRINT_PATH"

HTTP_CODE=$(curl -s -o /tmp/palette-validate-valid.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/$WORKFLOW_ID/validate-blueprint")

if [[ "$HTTP_CODE" -ne 200 ]]; then
  echo "FAIL: validate-blueprint returned HTTP $HTTP_CODE (expected 200)"
  cat /tmp/palette-validate-valid.json
  PASS=false
else
  VALID=$(jq -r '.valid' /tmp/palette-validate-valid.json)
  ADDED=$(jq '.added_tasks | length' /tmp/palette-validate-valid.json)
  if [[ "$VALID" == "true" && "$ADDED" -gt 0 ]]; then
    echo "PASS: Valid Blueprint change accepted ($ADDED tasks to add)"
    jq -r '.added_tasks[]' /tmp/palette-validate-valid.json | while read -r tid; do
      echo "  + $tid"
    done
  else
    echo "FAIL: Expected validation to pass with added tasks, got valid=$VALID added=$ADDED"
    cat /tmp/palette-validate-valid.json
    PASS=false
  fi
fi

# Leave the valid Blueprint in place for resume

# --- Step 7: Resume workflow ---
echo ""
echo "=== Step 7: Resume workflow ==="
HTTP_CODE=$(curl -s --max-time 120 -o /tmp/palette-resume-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/$WORKFLOW_ID/resume")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/$WORKFLOW_ID/resume returned HTTP $HTTP_CODE"
  cat /tmp/palette-resume-response.json
  exit 1
fi

RESUMED_COUNT=$(jq -r '.resumed_count' /tmp/palette-resume-response.json)
RECONCILED=$(jq -r '.reconciliation // empty' /tmp/palette-resume-response.json)
echo "Resumed $RESUMED_COUNT workers"
if [[ -n "$RECONCILED" ]]; then
  TASKS_CREATED=$(jq -r '.reconciliation.tasks_created' /tmp/palette-resume-response.json)
  echo "Reconciliation: $TASKS_CREATED tasks created"
  if [[ "$TASKS_CREATED" -gt 0 ]]; then
    echo "PASS: Reconciliation created new tasks"
  else
    echo "FAIL: Expected reconciliation to create tasks"
    PASS=false
  fi
fi

# Restore original Blueprint
cp "$BLUEPRINT_ORIG" "$BLUEPRINT_PATH"
rm -f "$BLUEPRINT_ORIG"

# Wait for containers to restart and Claude Code to boot
sleep 10

# --- Step 8: Verify resume ---
echo ""
echo "=== Step 8: Verify resume ==="

# Check: containers are running again
RUNNING_AFTER=$(docker ps -q --filter label=palette.managed=true 2>/dev/null | wc -l | tr -d ' ')
if [[ "$RUNNING_AFTER" -gt 0 ]]; then
  echo "PASS: $RUNNING_AFTER containers running after resume"
else
  echo "FAIL: No running containers after resume"
  docker ps -a --filter label=palette.managed=true --format "  {{.ID}} {{.Names}} {{.Status}}" 2>/dev/null
  PASS=false
fi

# Check: workflow status is active via API
ACTIVE_WORKFLOWS=$(curl -sf "$PALETTE_URL/workflows?status=active" 2>/dev/null | jq 'length' 2>/dev/null || echo "0")
if [[ "$ACTIVE_WORKFLOWS" -gt 0 ]]; then
  echo "PASS: Workflow status is active (via API)"
else
  echo "FAIL: No active workflows found via API"
  PASS=false
fi

# Check: resume log messages
if grep -q "resume complete" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Resume complete log message found"
else
  echo "FAIL: No resume complete log message"
  PASS=false
fi

# Check: Claude Code readiness after resume
if grep -q "Claude Code is ready" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Claude Code readiness detected after resume"
else
  echo "FAIL: No Claude Code readiness detected"
  PASS=false
fi

# Check: reconciliation log message
if grep -q "reconciliation complete" "$LOG_FILE" 2>/dev/null; then
  echo "PASS: Reconciliation log message found"
else
  echo "FAIL: No reconciliation log message"
  PASS=false
fi

# --- Step 9: Wait for jobs to complete (max 300 seconds) ---
echo ""
echo "=== Step 9: Wait for jobs to complete ==="
for i in $(seq 1 60); do
  WORKERS_JSON=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
  CRASHED=$(echo "$WORKERS_JSON" | jq '[.[] | select(.status == "crashed")] | length' 2>/dev/null || echo "0")

  if [[ "$CRASHED" -gt 0 ]]; then
    echo "FAIL: $CRASHED workers crashed after resume"
    echo "$WORKERS_JSON" | jq '.[] | select(.status == "crashed")' 2>/dev/null
    PASS=false
    break
  fi

  JOBS_JSON=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  TOTAL_JOBS=$(echo "$JOBS_JSON" | jq 'length' 2>/dev/null || echo "0")
  DONE_JOBS=$(echo "$JOBS_JSON" | jq '[.[] | select(.status == "done")] | length' 2>/dev/null || echo "0")

  echo "  Poll $i: $DONE_JOBS/$TOTAL_JOBS jobs done"

  if [[ "$TOTAL_JOBS" -gt 0 && "$DONE_JOBS" -eq "$TOTAL_JOBS" ]]; then
    echo "All jobs completed"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: Jobs did not all complete within 300 seconds"
    PASS=false
  fi
  sleep 5
done

# Verify the reconciled task got a job
if command -v sqlite3 &>/dev/null && [[ -f "$DB_FILE" ]]; then
  EXTRA_JOB=$(sqlite3 "$DB_FILE" "SELECT COUNT(*) FROM jobs WHERE task_id LIKE '%/extra-craft';" 2>/dev/null || echo "0")
  if [[ "$EXTRA_JOB" -gt 0 ]]; then
    echo "PASS: Job created for reconciled extra-craft task"
  else
    echo "FAIL: No job found for reconciled extra-craft task"
    PASS=false
  fi
fi

echo ""
if [[ "$PASS" == true ]]; then
  echo "=== All suspend/resume/reconciliation checks passed ==="
  scripts/reset.sh 2>&1
  exit 0
else
  echo "=== FAILED: Some checks did not pass ==="
  echo ""
  echo "--- Palette log (last 40 lines) ---"
  tail -40 "$LOG_FILE"
  exit 1
fi
