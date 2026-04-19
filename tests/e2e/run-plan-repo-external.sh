#!/usr/bin/env bash
# E2E: Repo-outside Plan, mode II (plan 009/002)
#
# The Blueprint lives outside any clone of the target repo (e.g., a coordinator
# repo that orchestrates work on a separate repository). The Orchestrator must
# NOT commit the Blueprint into the workspace; instead it bind-mounts the
# Blueprint directory at `/home/agent/plans` (read-only) so the Crafter can
# read the Plan without touching the target repo's git history.
#
# Checks performed once the craft job reaches in_progress:
# - Workspace HEAD has NO `chore(plan): import workflow plan` commit
# - Crafter container has a `/home/agent/plans` bind mount pointing at the
#   Blueprint directory on the host, read-only
# - remote.origin.pushurl is NOT PUSH_DISABLED (craft workspaces allow push)
#
# Checks performed after the workflow reaches completion:
# - `workflow completed` log line appears
# - All jobs (craft + review) reach status=done
# - Workflow status is `completed`
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
BP_DIR="/tmp/palette-mode-ii-bp-e2e"
BLUEPRINT_PATH="$BP_DIR/blueprint.yaml"
WORK_BRANCH="plan/2026-999-e2e-mode-ii"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=24

cleanup() {
  "$SCRIPT_DIR/stop-palette.sh" 2>/dev/null || true
  rm -rf "$BP_DIR"
}
trap cleanup EXIT

git_in_ws() {
  local ws="$1"
  shift
  local alternates="$ws/.git/objects/info/alternates"
  local saved
  saved=$(cat "$alternates")
  local cache_objects="$ROOT_DIR/data/repos/x7c1/palette-demo.git/objects"
  printf '%s\n' "$cache_objects" >"$alternates"
  local rc=0
  git -C "$ws" "$@" || rc=$?
  printf '%s' "$saved" >"$alternates"
  return $rc
}

worker_summary() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null |
    jq -r '[.[] | "\(.id):\(.status)"] | join(" ")' 2>/dev/null ||
    echo ""
}

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
cargo build 2>&1

# --- Step 2: Prepare Blueprint OUTSIDE any git clone of the target repo ---
echo ""
echo "=== Step 2: Prepare Blueprint ==="
rm -rf "$BP_DIR"
mkdir -p "$BP_DIR"
cat >"$BLUEPRINT_PATH" <<EOF
task:
  key: e2e-mode-ii
  plan_path: README.md
  children:
    - key: implement
      type: craft
      plan_path: README.md
      priority: high
      repository:
        name: x7c1/palette-demo
        work_branch: $WORK_BRANCH
      children:
        - key: review
          type: review
EOF
cat >"$BP_DIR/README.md" <<'EOF'
# Mode II E2E Plan

## Goal

Create `mode-ii.txt` in the workspace root with the content "mode II ok" and commit it.

## Steps

- Create `mode-ii.txt` at the workspace root
- Write exactly: `mode II ok`
- Stage and commit the file on the current branch

## Acceptance Criteria

- `mode-ii.txt` exists at the workspace root
- File contents equal "mode II ok"
- The file is committed (not just staged)
EOF
BP_DIR_ABS=$(cd "$BP_DIR" && pwd -P)
echo "Blueprint at $BLUEPRINT_PATH (outside any palette-demo clone)"
echo "Host abs path: $BP_DIR_ABS"

# --- Step 3: Start Palette ---
echo ""
echo "=== Step 3: Start Palette ==="
RUST_LOG=info cargo run -- start >>"$LOG_FILE" 2>&1 &
echo $! >"$PID_FILE"
echo "PID: $(cat "$PID_FILE")"

for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" >/dev/null 2>&1; then
    echo "Health check passed after $((i * 2)) seconds"
    break
  fi
  if [[ $i -eq 30 ]]; then
    echo "FAIL: Health check timed out"
    tail -20 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# --- Step 4: Start workflow ---
echo ""
echo "=== Step 4: Start workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  cat /tmp/palette-e2e-response.json
  exit 1
fi

WORKFLOW_ID=$(jq -r '.workflow_id' /tmp/palette-e2e-response.json)
echo "Workflow ID: $WORKFLOW_ID"

# --- Step 5: Wait for craft job in_progress ---
echo ""
echo "=== Step 5: Wait for craft job in_progress ==="
CRAFT_JOB=""
for i in $(seq 1 60); do
  JOBS_RAW=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  CRAFT_JOB=$(echo "$JOBS_RAW" | jq -r '.[] | select(.type == "craft") | .id' 2>/dev/null | head -1)
  CRAFT_STATUS=$(echo "$JOBS_RAW" | jq -r '.[] | select(.type == "craft") | .status' 2>/dev/null | head -1)
  echo "  [$((i * 2))s] craft_job=$CRAFT_JOB status=$CRAFT_STATUS"
  if [[ -n "$CRAFT_JOB" && "$CRAFT_STATUS" == "in_progress" ]]; then
    echo "Craft job $CRAFT_JOB in_progress"
    break
  fi
  if [[ $i -eq 60 ]]; then
    echo "FAIL: Craft job did not reach in_progress (last status: $CRAFT_STATUS)"
    echo "Jobs: $JOBS_RAW"
    tail -40 "$LOG_FILE"
    exit 1
  fi
  sleep 2
done

# --- Step 6: Verify mode II state ---
echo ""
echo "=== Step 6: Verify mode II state ==="
WS_DIR="$ROOT_DIR/data/workspace/$CRAFT_JOB"
if [[ ! -d "$WS_DIR" ]]; then
  echo "FAIL: Workspace directory not found at $WS_DIR"
  exit 1
fi

PASS=true

# (a) No plan-import commit on the work branch
IMPORT_COUNT=$(git_in_ws "$WS_DIR" log --format='%s' HEAD 2>/dev/null |
  grep -c '^chore(plan): import workflow plan$' || true)
if [[ "$IMPORT_COUNT" == "0" ]]; then
  echo "PASS: no plan-import commit in HEAD (mode II does not commit the Blueprint)"
else
  echo "FAIL: expected 0 plan-import commits, got $IMPORT_COUNT"
  git_in_ws "$WS_DIR" log --oneline -10 2>/dev/null || true
  PASS=false
fi

# (b) Craft workspace must keep origin pushurl intact
PUSHURL=$(git -C "$WS_DIR" config remote.origin.pushurl 2>/dev/null || echo "")
if [[ "$PUSHURL" != "PUSH_DISABLED" ]]; then
  echo "PASS: remote.origin.pushurl not PUSH_DISABLED (got '${PUSHURL:-<unset>}')"
else
  echo "FAIL: remote.origin.pushurl is PUSH_DISABLED"
  PASS=false
fi

# (c) Crafter container has /home/agent/plans mounted read-only from BP_DIR_ABS
CRAFTER_CONTAINER=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null |
  jq -r ".[] | select(.workflow_id == \"$WORKFLOW_ID\" and .role == \"member\") | .container_id" | head -1)
if [[ -z "$CRAFTER_CONTAINER" ]]; then
  echo "FAIL: no member worker found for workflow $WORKFLOW_ID"
  curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq '.'
  PASS=false
else
  MOUNT_JSON=$(docker inspect "$CRAFTER_CONTAINER" 2>/dev/null |
    jq -c '.[0].Mounts[] | select(.Destination == "/home/agent/plans")')
  if [[ -z "$MOUNT_JSON" ]]; then
    echo "FAIL: /home/agent/plans mount missing on crafter container"
    PASS=false
  else
    MOUNT_SOURCE=$(echo "$MOUNT_JSON" | jq -r '.Source')
    MOUNT_MODE=$(echo "$MOUNT_JSON" | jq -r '.Mode')
    MOUNT_RW=$(echo "$MOUNT_JSON" | jq -r '.RW')
    # On macOS, Docker Desktop may prepend /private to /tmp paths.
    if [[ "$MOUNT_SOURCE" == "$BP_DIR_ABS" || "$MOUNT_SOURCE" == "/private$BP_DIR_ABS" ]]; then
      echo "PASS: /home/agent/plans mounted from $MOUNT_SOURCE"
    else
      echo "FAIL: /home/agent/plans source=$MOUNT_SOURCE (expected $BP_DIR_ABS)"
      PASS=false
    fi
    if [[ "$MOUNT_RW" == "false" || "$MOUNT_MODE" == *ro* ]]; then
      echo "PASS: /home/agent/plans mounted read-only (RW=$MOUNT_RW mode=$MOUNT_MODE)"
    else
      echo "FAIL: /home/agent/plans not read-only (RW=$MOUNT_RW mode=$MOUNT_MODE)"
      PASS=false
    fi
  fi
fi

if [[ "$PASS" != true ]]; then
  echo ""
  echo "=== FAILED in mode II verification ==="
  tail -40 "$LOG_FILE"
  exit 1
fi

# --- Step 7: Wait for workflow completion ---
echo ""
echo "=== Step 7: Wait for workflow completion ==="
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

  CRASHED=$(echo "$JOBS" | jq '[.[] | select(.status == "crashed" or .status == "failed")] | length' 2>/dev/null || echo "0")
  if [[ "$CRASHED" -gt 0 ]]; then
    echo "FAIL: a job entered crashed/failed status"
    echo "$JOBS" | jq '.[] | {id, type, status}'
    tail -60 "$LOG_FILE"
    exit 1
  fi

  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall detected (no progress for $((STALL_THRESHOLD * POLL_INTERVAL))s)"
    tail -60 "$LOG_FILE"
    exit 1
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    echo "Detected 'workflow completed' in log"
    break
  fi
done

# --- Step 8: Verify post-completion state ---
echo ""
echo "=== Step 8: Verify post-completion state ==="
sleep 3

JOBS_FINAL=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
TOTAL=$(echo "$JOBS_FINAL" | jq 'length')
DONE=$(echo "$JOBS_FINAL" | jq '[.[] | select(.status == "done")] | length')
if [[ "$TOTAL" -gt 0 && "$DONE" -eq "$TOTAL" ]]; then
  echo "PASS: all $TOTAL jobs in status=done"
else
  echo "FAIL: $DONE/$TOTAL jobs done"
  echo "$JOBS_FINAL" | jq '.[] | {id, type, status}'
  exit 1
fi

WORKFLOW_STATUS=$(curl -sf "$PALETTE_URL/workflows" 2>/dev/null |
  jq -r ".[] | select(.id == \"$WORKFLOW_ID\") | .status")
if [[ "$WORKFLOW_STATUS" == "completed" ]]; then
  echo "PASS: workflow status=completed"
else
  echo "FAIL: workflow status=$WORKFLOW_STATUS (expected completed)"
  curl -sf "$PALETTE_URL/workflows" 2>/dev/null | jq '.'
  exit 1
fi

echo ""
echo "=== All Repo-outside Plan (mode II) checks passed ==="
exit 0
