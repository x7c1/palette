#!/usr/bin/env bash
# E2E: Branch collision detection at workflow-start (plan 009/002)
#
# When two Blueprints claim the same `(repo, work_branch)` pair and the first
# Workflow is still active, POST /workflows/start for the second must be
# rejected with HTTP 400 and reason=workflow/branch_in_use — no second
# workspace may be set up.
#
# Steps:
# 1. Start Palette
# 2. Start Workflow 1 (blueprint 1, expects HTTP 200)
# 3. Immediately start Workflow 2 (blueprint 2, same (repo, work_branch))
# 4. Assert: HTTP 400, code=input_validation_failed, errors[].reason=workflow/work_branch_in_use
# 5. Assert: only Workflow 1 appears in the /workflows list (no row created for 2)
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
BP_DIR_1="/tmp/palette-branch-collision-e2e-1"
BP_DIR_2="/tmp/palette-branch-collision-e2e-2"
BLUEPRINT_1="$BP_DIR_1/blueprint.yaml"
BLUEPRINT_2="$BP_DIR_2/blueprint.yaml"
SHARED_BRANCH="plan/2026-999-e2e-branch-collision"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"

cleanup() {
  "$SCRIPT_DIR/stop-palette.sh" 2>/dev/null || true
  rm -rf "$BP_DIR_1" "$BP_DIR_2"
}
trap cleanup EXIT

write_blueprint() {
  local dir="$1"
  local key="$2"
  mkdir -p "$dir"
  cat >"$dir/blueprint.yaml" <<EOF
task:
  key: $key
  plan_path: README.md
  children:
    - key: implement
      type: craft
      plan_path: README.md
      priority: high
      repository:
        name: x7c1/palette-demo
        work_branch: $SHARED_BRANCH
      children:
        - key: review
          type: review
EOF
  cat >"$dir/README.md" <<'EOF'
# Branch collision E2E stub

## Goal

Placeholder plan — this workflow should be rejected by the branch collision
check before any Crafter runs, so the task description is never read.
EOF
}

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
rm -f "$LOG_FILE"
cargo build 2>&1

# --- Step 2: Prepare two Blueprints sharing the same (repo, work_branch) ---
echo ""
echo "=== Step 2: Prepare Blueprints ==="
rm -rf "$BP_DIR_1" "$BP_DIR_2"
write_blueprint "$BP_DIR_1" "e2e-collision-one"
write_blueprint "$BP_DIR_2" "e2e-collision-two"
echo "Blueprint 1 at $BLUEPRINT_1"
echo "Blueprint 2 at $BLUEPRINT_2"
echo "Shared work branch: $SHARED_BRANCH"

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

# --- Step 4: Start Workflow 1 (must succeed) ---
echo ""
echo "=== Step 4: Start Workflow 1 (expect HTTP 200) ==="
HTTP_CODE=$(curl -s -o /tmp/palette-collision-1.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_1\"}")

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: Workflow 1 POST returned HTTP $HTTP_CODE (expected 2xx)"
  cat /tmp/palette-collision-1.json
  exit 1
fi
WORKFLOW_1_ID=$(jq -r '.workflow_id' /tmp/palette-collision-1.json)
echo "Workflow 1 started: $WORKFLOW_1_ID"

# --- Step 5: Start Workflow 2 (must be rejected) ---
echo ""
echo "=== Step 5: Start Workflow 2 (expect HTTP 400 branch_in_use) ==="
HTTP_CODE=$(curl -s -o /tmp/palette-collision-2.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_2\"}")

PASS=true
if [[ "$HTTP_CODE" != "400" ]]; then
  echo "FAIL: Workflow 2 POST returned HTTP $HTTP_CODE (expected 400)"
  cat /tmp/palette-collision-2.json
  PASS=false
fi

RESPONSE_CODE=$(jq -r '.code' /tmp/palette-collision-2.json 2>/dev/null || echo "")
if [[ "$RESPONSE_CODE" == "input_validation_failed" ]]; then
  echo "PASS: response code=input_validation_failed"
else
  echo "FAIL: response code=$RESPONSE_CODE (expected input_validation_failed)"
  cat /tmp/palette-collision-2.json
  PASS=false
fi

REASON=$(jq -r '.errors[0].reason' /tmp/palette-collision-2.json 2>/dev/null || echo "")
if [[ "$REASON" == "workflow/work_branch_in_use" ]]; then
  echo "PASS: errors[0].reason=workflow/work_branch_in_use"
else
  echo "FAIL: errors[0].reason=$REASON (expected workflow/work_branch_in_use)"
  cat /tmp/palette-collision-2.json
  PASS=false
fi

HINT=$(jq -r '.errors[0].hint' /tmp/palette-collision-2.json 2>/dev/null || echo "")
if [[ "$HINT" == *"$SHARED_BRANCH"* ]]; then
  echo "PASS: errors[0].hint contains branch name ($HINT)"
else
  echo "FAIL: errors[0].hint=$HINT (expected to include $SHARED_BRANCH)"
  PASS=false
fi

# --- Step 6: Verify no row for Workflow 2 ---
echo ""
echo "=== Step 6: Verify no row created for Workflow 2 ==="
WF_COUNT=$(curl -sf "$PALETTE_URL/workflows" 2>/dev/null | jq 'length')
if [[ "$WF_COUNT" == "1" ]]; then
  echo "PASS: only 1 workflow row exists"
else
  echo "FAIL: expected 1 workflow row, got $WF_COUNT"
  curl -sf "$PALETTE_URL/workflows" 2>/dev/null | jq '.'
  PASS=false
fi

WF_1_FOUND=$(curl -sf "$PALETTE_URL/workflows" 2>/dev/null |
  jq -r ".[] | select(.id == \"$WORKFLOW_1_ID\") | .id")
if [[ "$WF_1_FOUND" == "$WORKFLOW_1_ID" ]]; then
  echo "PASS: workflow 1 present in list"
else
  echo "FAIL: workflow 1 ($WORKFLOW_1_ID) missing from list"
  PASS=false
fi

echo ""
if [[ "$PASS" == true ]]; then
  echo "=== All branch-collision checks passed ==="
  exit 0
else
  echo "=== FAILED ==="
  tail -40 "$LOG_FILE"
  exit 1
fi
