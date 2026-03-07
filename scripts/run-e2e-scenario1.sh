#!/bin/bash
# E2E Scenario 1: Parallel tasks with auto-assign (003-multi-member)
#   Load two independent work tasks from YAML via /tasks/load.
#   Orchestrator auto-spawns two members in parallel.
#   Leader handles permission prompts, reviews, and approves.
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
echo "=== Loading tasks from YAML ==="
LOAD_RESP=$(curl -s -X POST http://127.0.0.1:7100/tasks/load \
  -H "Content-Type: text/plain" \
  --data-binary @tests/fixtures/scenario1-parallel.yaml)
echo "$LOAD_RESP" | jq -r '.[] | "\(.id) \(.status)"'

echo ""
echo "=== Monitoring agents (max 8 minutes) ==="
echo "    Watch live: tmux attach -t palette"
echo ""
for i in $(seq 1 96); do
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
WA_STATUS=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].status')
WB_STATUS=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].status')
WA_ASSIGNED=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].assigned_at')
WB_ASSIGNED=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].assigned_at')
WA_DONE=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].updated_at')
WB_DONE=$(curl -s http://127.0.0.1:7100/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].updated_at')

echo "Task A status: $WA_STATUS (expected: done)"
echo "Task B status: $WB_STATUS (expected: done)"
echo "Task A assigned_at: $WA_ASSIGNED"
echo "Task B assigned_at: $WB_ASSIGNED"
echo "Parallel check: A assigned before B done, B assigned before A done"

RESULT=PASSED
if [ "$WA_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$WB_STATUS" != "done" ]; then RESULT=FAILED; fi

echo "=== SCENARIO 1 $RESULT ==="
