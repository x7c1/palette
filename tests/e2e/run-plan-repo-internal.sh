#!/usr/bin/env bash
# E2E: Repo-inside Plan, mode I (plan 009/002)
#
# Simulates the Operator authoring a Blueprint in a host clone of palette-demo
# and starting a workflow. The Orchestrator should detect that the Blueprint's
# host git root shares its `<owner>/<repo>` with the workspace clone, copy the
# Blueprint into the workspace at the same relative path, and commit it on the
# work branch with message "chore(plan): import workflow plan".
#
# Checks performed once the craft job reaches in_progress:
# - Workspace HEAD has exactly one `chore(plan): import workflow plan` commit
# - blueprint.yaml and README.md are present in HEAD's tree at the expected rel path
# - remote.origin.pushurl is NOT PUSH_DISABLED (craft workspaces allow push)
# - Crafter container has NO `/home/agent/plans` bind mount (mode II signal
#   must be absent when mode I is in effect)
#
# Checks performed after the workflow reaches completion:
# - `workflow completed` log line appears (no stall, no crash)
# - All jobs (craft + review) reach status=done via the API
# - Workflow status is `completed` via the API
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
HOST_CLONE_DIR="/tmp/palette-demo-mode-i-e2e"
BLUEPRINT_REL_DIR="docs/plans/e2e-mode-i"
BLUEPRINT_PATH="$HOST_CLONE_DIR/$BLUEPRINT_REL_DIR/blueprint.yaml"
WORK_BRANCH="plan/2026-999-e2e-mode-i"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=24

cleanup() {
  "$SCRIPT_DIR/stop-palette.sh" 2>/dev/null || true
  rm -rf "$HOST_CLONE_DIR"
}
trap cleanup EXIT

# Run git against the workspace with alternates temporarily pointed at the
# host-side cache objects dir so object resolution works. The live alternates
# in a created workspace reference the container path
# `/home/agent/repo-cache/objects`, which is not accessible from the host.
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

# --- Step 2: Prepare host clone with an uncommitted Blueprint ---
echo ""
echo "=== Step 2: Prepare host clone with Blueprint ==="
rm -rf "$HOST_CLONE_DIR"
git clone --quiet https://github.com/x7c1/palette-demo.git "$HOST_CLONE_DIR"
mkdir -p "$HOST_CLONE_DIR/$BLUEPRINT_REL_DIR"
cat >"$HOST_CLONE_DIR/$BLUEPRINT_REL_DIR/blueprint.yaml" <<EOF
task:
  key: e2e-mode-i
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
cat >"$HOST_CLONE_DIR/$BLUEPRINT_REL_DIR/README.md" <<'EOF'
# Mode I E2E Plan

## Goal

Create `mode-i.txt` in the workspace root with the content "mode I ok" and commit it.

## Steps

- Create `mode-i.txt` at the workspace root
- Write exactly: `mode I ok`
- Stage and commit the file on the current branch

## Acceptance Criteria

- `mode-i.txt` exists at the workspace root
- File contents equal "mode I ok"
- The file is committed (not just staged)
EOF
echo "Host clone at $HOST_CLONE_DIR"
echo "Blueprint at $BLUEPRINT_PATH (uncommitted)"

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

# --- Step 5: Wait for workspace creation (craft job in_progress) ---
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

# --- Step 6: Verify mode I materialisation ---
echo ""
echo "=== Step 6: Verify mode I materialisation ==="
WS_DIR="$ROOT_DIR/data/workspace/$CRAFT_JOB"
if [[ ! -d "$WS_DIR" ]]; then
  echo "FAIL: Workspace directory not found at $WS_DIR"
  exit 1
fi

PASS=true

# (a) Exactly one plan-import commit on the work branch
IMPORT_COUNT=$(git_in_ws "$WS_DIR" log --format='%s' HEAD 2>/dev/null |
  grep -c '^chore(plan): import workflow plan$' || true)
if [[ "$IMPORT_COUNT" == "1" ]]; then
  echo "PASS: exactly 1 'chore(plan): import workflow plan' commit in HEAD"
else
  echo "FAIL: expected 1 plan-import commit, got $IMPORT_COUNT"
  git_in_ws "$WS_DIR" log --oneline -10 2>/dev/null || true
  PASS=false
fi

# (b) blueprint.yaml in HEAD tree
if git_in_ws "$WS_DIR" show "HEAD:$BLUEPRINT_REL_DIR/blueprint.yaml" >/dev/null 2>&1; then
  echo "PASS: blueprint.yaml present in HEAD tree"
else
  echo "FAIL: blueprint.yaml missing from HEAD tree at $BLUEPRINT_REL_DIR"
  PASS=false
fi

# (c) README.md in HEAD tree
if git_in_ws "$WS_DIR" show "HEAD:$BLUEPRINT_REL_DIR/README.md" >/dev/null 2>&1; then
  echo "PASS: README.md present in HEAD tree"
else
  echo "FAIL: README.md missing from HEAD tree at $BLUEPRINT_REL_DIR"
  PASS=false
fi

# (d) Craft workspace must keep origin pushurl intact
PUSHURL=$(git -C "$WS_DIR" config remote.origin.pushurl 2>/dev/null || echo "")
if [[ "$PUSHURL" != "PUSH_DISABLED" ]]; then
  echo "PASS: remote.origin.pushurl not PUSH_DISABLED (got '${PUSHURL:-<unset>}')"
else
  echo "FAIL: remote.origin.pushurl is PUSH_DISABLED"
  PASS=false
fi

# (e) Crafter container must have NO /home/agent/plans mount (mode I signal)
CRAFTER_CONTAINER=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null |
  jq -r ".[] | select(.workflow_id == \"$WORKFLOW_ID\" and .role == \"member\") | .container_id" | head -1)
if [[ -z "$CRAFTER_CONTAINER" ]]; then
  echo "FAIL: no member worker found for workflow $WORKFLOW_ID"
  curl -sf "$PALETTE_URL/workers" 2>/dev/null | jq '.'
  PASS=false
else
  PLAN_MOUNT=$(docker inspect "$CRAFTER_CONTAINER" 2>/dev/null |
    jq -r '.[0].Mounts[] | select(.Destination == "/home/agent/plans") | .Source')
  if [[ -z "$PLAN_MOUNT" ]]; then
    echo "PASS: crafter container $CRAFTER_CONTAINER has no /home/agent/plans mount"
  else
    echo "FAIL: unexpected /home/agent/plans mount on $CRAFTER_CONTAINER (source=$PLAN_MOUNT)"
    PASS=false
  fi
fi

if [[ "$PASS" != true ]]; then
  echo ""
  echo "=== FAILED in workspace-setup verification ==="
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

# Give the orchestrator a moment to mark jobs done and clean up workers.
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
echo "=== All Repo-inside Plan (mode I) checks passed ==="
exit 0
