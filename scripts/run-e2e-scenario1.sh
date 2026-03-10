#!/bin/bash
# E2E Scenario 1: Parallel tasks with auto-assign (003-multi-member)
#   Load two independent work tasks from YAML via /tasks/load.
#   Orchestrator auto-spawns two members in parallel.
#   Leader handles permission prompts, reviews, and approves.
set -euo pipefail

cd "$(dirname "$0")/.."

PALETTE_PORT=7100
BASE_URL="http://127.0.0.1:${PALETTE_PORT}"

# Log output to timestamped file
LOG_DIR="data/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/scenario1-$(date +%Y%m%d-%H%M%S).log"
exec > >(tee "$LOG_FILE") 2>&1
echo "Logging to $LOG_FILE"

# Clean up previous state
echo "=== Cleanup ==="
lsof -ti:${PALETTE_PORT} | xargs -r kill 2>/dev/null || true
docker ps -q --filter label=palette.managed=true | xargs -r docker rm -f 2>/dev/null || true
tmux kill-session -t palette 2>/dev/null || true
rm -f data/state.json data/palette.db data/palette.db-shm data/palette.db-wal
docker volume ls -q --filter name=palette- | xargs -r docker volume rm 2>/dev/null || true
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
for i in $(seq 1 60); do
    if curl -sf ${BASE_URL}/tasks >/dev/null 2>&1; then
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

LEADER_PANE=$(jq -r '.leaders[0].terminal_target' data/state.json)
echo "Leader pane: $LEADER_PANE"

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "=== Loading tasks from YAML ==="
LOAD_RESP=$(curl -s -X POST ${BASE_URL}/tasks/load \
  -H "Content-Type: text/plain" \
  --data-binary @tests/fixtures/scenario1-parallel.yaml)
echo "$LOAD_RESP" | jq -r '.[] | "\(.id) \(.status)"'

echo ""
echo "=== Monitoring agents (max 8 minutes) ==="
echo "    Watch live: tmux attach -t palette"
echo ""

STALL_THRESHOLD=12  # 12 iterations x 5s = 60s without progress
STALL_COUNT=0
PREV_SNAPSHOT=""

for i in $(seq 1 96); do
    sleep 5

    # Collect current state snapshot for stall detection
    TASKS_JSON=$(curl -s ${BASE_URL}/tasks 2>/dev/null || echo "[]")
    STATE_JSON=$(cat data/state.json 2>/dev/null || echo "{}")
    TASK_SNAPSHOT=$(echo "$TASKS_JSON" | jq -r '[.[] | "\(.id):\(.status):\(.assignee // "")"] | sort | join(",")' 2>/dev/null || echo "")
    MEMBER_SNAPSHOT=$(echo "$STATE_JSON" | jq -r '[(.leaders + .members)[] | "\(.id):\(.status)"] | sort | join(",")' 2>/dev/null || echo "")
    # Capture last non-empty line from each agent's pane to detect activity
    PANE_SNAPSHOT=""
    for pane_target in $(echo "$STATE_JSON" | jq -r '(.leaders + .members)[] | .terminal_target' 2>/dev/null); do
        last_line=$(tmux capture-pane -t "$pane_target" -p 2>/dev/null | grep -v '^$' | tail -1)
        PANE_SNAPSHOT="${PANE_SNAPSHOT}|${last_line}"
    done
    CURRENT_SNAPSHOT="${TASK_SNAPSHOT}|${MEMBER_SNAPSHOT}|${PANE_SNAPSHOT}"

    if [ "$CURRENT_SNAPSHOT" = "$PREV_SNAPSHOT" ]; then
        STALL_COUNT=$((STALL_COUNT + 1))
    else
        STALL_COUNT=0
        PREV_SNAPSHOT="$CURRENT_SNAPSHOT"
    fi

    ELAPSED=$((i * 5))
    echo "--- ${ELAPSED}s (stall: ${STALL_COUNT}/${STALL_THRESHOLD}) ---"

    # Show task statuses
    echo "  [tasks]"
    echo "$TASKS_JSON" | jq -r '.[] | "    \(.id) \(.status) \(.assignee // "")"' 2>/dev/null

    # Show agent statuses
    echo "  [agents]"
    echo "$STATE_JSON" | jq -r '(.leaders + .members)[] | "    \(.id) \(.status)"' 2>/dev/null

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
    DONE_COUNT=$(echo "$TASKS_JSON" | jq '[.[] | select(.type == "work" and .status == "done")] | length' 2>/dev/null || echo 0)
    echo "  [work done: $DONE_COUNT/2]"
    if [ "$DONE_COUNT" = "2" ]; then
        echo ""
        echo "=== Both work tasks done! ==="
        break
    fi

    # Stall detection
    if [ "$STALL_COUNT" -ge "$STALL_THRESHOLD" ]; then
        echo ""
        echo "=== STALL DETECTED: no state change for ${STALL_THRESHOLD}x5s ==="
        echo "  Snapshot: $CURRENT_SNAPSHOT"

        IDLE_AGENTS=$(echo "$STATE_JSON" | jq -r '[(.leaders + .members)[] | select(.status == "idle")] | length' 2>/dev/null || echo 0)
        READY_TASKS=$(echo "$TASKS_JSON" | jq '[.[] | select(.status == "ready")] | length' 2>/dev/null || echo 0)
        echo "  Idle agents: $IDLE_AGENTS, Ready tasks: $READY_TASKS"

        if [ "$IDLE_AGENTS" -gt 0 ] && [ "$READY_TASKS" -gt 0 ]; then
            echo "  HINT: Idle agents exist but ready tasks are not being assigned"
        fi
        echo "  Aborting. Inspect with: tmux attach -t palette"
        STALL_ABORT=1
        break
    fi
    echo ""
done

echo ""
echo "=== Leader pane full capture ==="
tmux capture-pane -t "$LEADER_PANE" -p -S -500 2>&1

echo "=== Final state ==="
echo "--- tasks ---"
curl -s ${BASE_URL}/tasks 2>&1 | jq .
echo "--- review submissions ---"
for task_id in $(curl -s '${BASE_URL}/tasks?type=review' 2>/dev/null | jq -r '.[].id' 2>/dev/null); do
    echo "--- submissions for $task_id ---"
    curl -s "${BASE_URL}/reviews/$task_id/submissions" 2>&1 | jq .
done
echo "--- state.json ---"
jq . data/state.json | head -40

echo ""
echo "=== Verification ==="
WA_STATUS=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].status')
WB_STATUS=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].status')
WA_ASSIGNED=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].assigned_at')
WB_ASSIGNED=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].assigned_at')
WA_DONE=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[0].updated_at')
WB_DONE=$(curl -s ${BASE_URL}/tasks | jq -r '[.[] | select(.type == "work")] | sort_by(.created_at) | .[1].updated_at')

echo "Task A status: $WA_STATUS (expected: done)"
echo "Task B status: $WB_STATUS (expected: done)"
echo "Task A assigned_at: $WA_ASSIGNED"
echo "Task B assigned_at: $WB_ASSIGNED"
echo "Parallel check: A assigned before B done, B assigned before A done"

RESULT=PASSED
if [ "${STALL_ABORT:-0}" = "1" ]; then RESULT="FAILED (stall)"; fi
if [ "$WA_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$WB_STATUS" != "done" ]; then RESULT=FAILED; fi

echo "=== SCENARIO 1 $RESULT ==="
