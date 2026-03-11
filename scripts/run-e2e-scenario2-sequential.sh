#!/bin/bash
# E2E Scenario 2: Sequential jobs with DAG dependency (003-multi-member)
#   Load jobs from YAML where W-B depends on W-A.
#   Orchestrator assigns W-A first. After W-A is done, W-B becomes assignable.
#   Verifies B's assigned_at is after A's done timestamp.
set -euo pipefail

cd "$(dirname "$0")/.."
source scripts/e2e-helpers.sh

PALETTE_PORT=7100
BASE_URL="http://127.0.0.1:${PALETTE_PORT}"

# Log output to timestamped file
LOG_DIR="data/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/scenario2-$(date +%Y%m%d-%H%M%S).log"
exec > >(tee "$LOG_FILE") 2>&1
echo "Logging to $LOG_FILE"

# Clean up previous state
e2e_cleanup $PALETTE_PORT

echo "=== Building ==="
cargo build 2>&1 | tail -3

echo "=== Starting palette ==="
cargo run 2>&1 &
PALETTE_PID=$!

cleanup() {
    echo "=== Stopping palette (PID=$PALETTE_PID) ==="
    kill $PALETTE_PID 2>/dev/null || true
    wait $PALETTE_PID 2>/dev/null || true
    docker ps -q --filter label=palette.managed=true | xargs -r docker rm -f 2>/dev/null || true
    tmux kill-session -t palette 2>/dev/null || true
}
trap cleanup EXIT

echo "Waiting for server to start..."
for i in $(seq 1 60); do
    if curl -sf ${BASE_URL}/jobs >/dev/null 2>&1; then
        echo "  server ready (${i}s)"
        break
    fi
    if ! kill -0 $PALETTE_PID 2>/dev/null; then
        echo "ERROR: palette exited unexpectedly"
        exit 1
    fi
    sleep 1
done

echo ""
echo "=== Palette running (PID=$PALETTE_PID) ==="

LEADER_PANE=$(jq -r '.supervisors[0].terminal_target' data/state.json)
echo "Leader pane: $LEADER_PANE"

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "=== Submitting blueprint ==="
SUBMIT_RESP=$(curl -s -X POST ${BASE_URL}/blueprints/submit \
  -H "Content-Type: text/plain" \
  --data-binary @tests/fixtures/scenario2-sequential.yaml)
TASK_ID=$(echo "$SUBMIT_RESP" | jq -r '.task_id')
TASK_ID_ENCODED=$(printf '%s' "$TASK_ID" | jq -sRr @uri)
echo "Blueprint submitted: $TASK_ID"

echo ""
echo "=== Loading blueprint (W-B depends on W-A) ==="
LOAD_RESP=$(curl -s -X POST ${BASE_URL}/blueprints/${TASK_ID_ENCODED}/load)
echo "$LOAD_RESP" | jq -r '.[] | "\(.id) \(.status)"'

echo ""
echo "=== Monitoring agents (max 10 minutes) ==="
echo "    Watch live: tmux attach -t palette"
echo ""

for i in $(seq 1 120); do
    sleep 5

    collect_snapshot

    ELAPSED=$((i * 5))
    print_status $ELAPSED

    echo "  [leader pane]"
    tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -2

    # Show dynamically spawned member panes
    MEMBER_COUNT=$(echo "$STATE_JSON" | jq -r '.members | length' 2>/dev/null || echo 0)
    if [ "$MEMBER_COUNT" -gt 0 ]; then
        for j in $(seq 0 $((MEMBER_COUNT - 1))); do
            MID=$(echo "$STATE_JSON" | jq -r ".members[$j].id" 2>/dev/null)
            MPANE=$(echo "$STATE_JSON" | jq -r ".members[$j].terminal_target" 2>/dev/null)
            echo "  [$MID pane]"
            tmux capture-pane -t "$MPANE" -p 2>&1 | grep -v '^$' | tail -2
        done
    fi

    # Check completion
    DONE_COUNT=$(echo "$JOBS_JSON" | jq '[.[] | select(.type == "craft" and .status == "done")] | length' 2>/dev/null || echo 0)
    echo "  [craft done: $DONE_COUNT/2]"
    if [ "$DONE_COUNT" = "2" ]; then
        echo ""
        echo "=== Both craft jobs done! ==="
        break
    fi

    if is_stalled; then
        echo ""
        echo "=== STALL DETECTED ==="
        STALL_ABORT=1
        break
    fi
    echo ""
done

echo ""
echo "=== Leader pane full capture ==="
tmux capture-pane -t "$LEADER_PANE" -p -S -500 2>&1

echo "=== Final state ==="
echo "--- jobs ---"
curl -s ${BASE_URL}/jobs 2>&1 | jq .
echo "--- review submissions ---"
for job_id in $(curl -s '${BASE_URL}/jobs?type=review' 2>/dev/null | jq -r '.[].id' 2>/dev/null); do
    echo "--- submissions for $job_id ---"
    curl -s "${BASE_URL}/reviews/$job_id/submissions" 2>&1 | jq .
done
echo "--- state.json ---"
jq . data/state.json | head -40

echo ""
echo "=== Verification ==="
WA_DONE_AT=$(curl -s ${BASE_URL}/jobs | jq -r '.[] | select(.id == "W-A") | .updated_at')
WB_ASSIGNED_AT=$(curl -s ${BASE_URL}/jobs | jq -r '.[] | select(.id == "W-B") | .assigned_at')
WA_STATUS=$(curl -s ${BASE_URL}/jobs | jq -r '.[] | select(.id == "W-A") | .status')
WB_STATUS=$(curl -s ${BASE_URL}/jobs | jq -r '.[] | select(.id == "W-B") | .status')

echo "Job A status: $WA_STATUS (expected: done)"
echo "Job B status: $WB_STATUS (expected: done)"
echo "Job A done at:    $WA_DONE_AT"
echo "Job B assigned at: $WB_ASSIGNED_AT"
echo "Sequential check: B assigned_at should be after A done_at"

RESULT=PASSED
if [ "${STALL_ABORT:-0}" = "1" ]; then RESULT="FAILED (stall)"; fi
if [ "$WA_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$WB_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$WA_STATUS" = "done" ] && [ "$WB_STATUS" = "done" ] && [[ "$WB_ASSIGNED_AT" < "$WA_DONE_AT" ]]; then
    echo "FAIL: B was assigned before A was done"
    RESULT=FAILED
fi

echo "=== SCENARIO 2 $RESULT ==="
