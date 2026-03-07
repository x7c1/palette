#!/bin/bash
# E2E Scenario 2: Sequential tasks with DAG dependency (003-multi-member)
#   Load tasks from YAML where W-B depends on W-A.
#   Orchestrator assigns W-A first. After W-A is done, W-B becomes assignable.
#   Verifies B's assigned_at is after A's done timestamp.
set -euo pipefail

cd "$(dirname "$0")/.."

# Clean up previous state
echo "=== Cleanup ==="
lsof -ti:7100 | xargs -r kill 2>/dev/null || true
docker ps -q --filter label=palette.managed=true | xargs -r docker rm -f 2>/dev/null || true
tmux kill-session -t palette 2>/dev/null || true
rm -f data/state.json data/palette.db data/palette.db-shm data/palette.db-wal
mkdir -p data

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
sleep 10

if ! kill -0 $PALETTE_PID 2>/dev/null; then
    echo "ERROR: palette exited unexpectedly"
    exit 1
fi

echo ""
echo "=== Palette running (PID=$PALETTE_PID) ==="

LEADER_PANE=$(jq -r '.leaders[0].tmux_target' data/state.json)
echo "Leader pane: $LEADER_PANE"

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "=== Waiting for Claude Code to initialize (10s) ==="
sleep 10

echo "--- leader pane ---"
tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -5

echo ""
echo "=== Loading tasks from YAML (W-B depends on W-A) ==="
LOAD_RESP=$(curl -s -X POST http://127.0.0.1:7100/tasks/load \
  -H "Content-Type: text/plain" \
  --data-binary @tests/fixtures/scenario2-sequential.yaml)
echo "$LOAD_RESP" | jq -r '.[] | "\(.id) \(.status)"'

echo ""
echo "=== Monitoring agents (max 10 minutes) ==="
echo "    Watch live: tmux attach -t palette"
echo ""
for i in $(seq 1 120); do
    sleep 5
    echo "--- ${i}x5s ---"

    echo "  [leader]"
    tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -3

    # Show dynamically spawned member panes
    MEMBER_COUNT=$(jq -r '.members | length' data/state.json 2>/dev/null || echo 0)
    if [ "$MEMBER_COUNT" -gt 0 ]; then
        for j in $(seq 0 $((MEMBER_COUNT - 1))); do
            MID=$(jq -r ".members[$j].id" data/state.json 2>/dev/null)
            MPANE=$(jq -r ".members[$j].tmux_target" data/state.json 2>/dev/null)
            MSTATUS=$(jq -r ".members[$j].status" data/state.json 2>/dev/null)
            echo "  [$MID ($MSTATUS)]"
            tmux capture-pane -t "$MPANE" -p 2>&1 | grep -v '^$' | tail -3
        done
    fi

    echo "  [containers]"
    docker ps --filter label=palette.managed=true --format '  {{.Names}} ({{.Status}})' 2>&1

    # Check if both work tasks reached "done"
    DONE_COUNT=$(curl -s http://127.0.0.1:7100/tasks 2>/dev/null | jq '[.[] | select(.type == "work" and .status == "done")] | length' 2>/dev/null || echo 0)
    echo "  [work done: $DONE_COUNT/2]"
    if [ "$DONE_COUNT" = "2" ]; then
        echo ""
        echo "=== Both work tasks done! ==="
        break
    fi
    echo ""
done

echo ""
echo "=== Leader pane full capture ==="
tmux capture-pane -t "$LEADER_PANE" -p -S -500 2>&1

echo "=== Final state ==="
echo "--- tasks ---"
curl -s http://127.0.0.1:7100/tasks 2>&1 | jq .
echo "--- review submissions ---"
for task_id in $(curl -s 'http://127.0.0.1:7100/tasks?type=review' 2>/dev/null | jq -r '.[].id' 2>/dev/null); do
    echo "--- submissions for $task_id ---"
    curl -s "http://127.0.0.1:7100/reviews/$task_id/submissions" 2>&1 | jq .
done
echo "--- state.json ---"
jq . data/state.json | head -40

echo ""
echo "=== Verification ==="
WA_DONE_AT=$(curl -s http://127.0.0.1:7100/tasks | jq -r '.[] | select(.id == "W-A") | .updated_at')
WB_ASSIGNED_AT=$(curl -s http://127.0.0.1:7100/tasks | jq -r '.[] | select(.id == "W-B") | .assigned_at')
WA_STATUS=$(curl -s http://127.0.0.1:7100/tasks | jq -r '.[] | select(.id == "W-A") | .status')
WB_STATUS=$(curl -s http://127.0.0.1:7100/tasks | jq -r '.[] | select(.id == "W-B") | .status')

echo "Task A status: $WA_STATUS (expected: done)"
echo "Task B status: $WB_STATUS (expected: done)"
echo "Task A done at:    $WA_DONE_AT"
echo "Task B assigned at: $WB_ASSIGNED_AT"
echo "Sequential check: B assigned_at should be after A done_at"

RESULT=PASSED
if [ "$WA_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$WB_STATUS" != "done" ]; then RESULT=FAILED; fi
if [[ "$WB_ASSIGNED_AT" < "$WA_DONE_AT" ]]; then
    echo "FAIL: B was assigned before A was done"
    RESULT=FAILED
fi

echo "=== SCENARIO 2 $RESULT ==="
