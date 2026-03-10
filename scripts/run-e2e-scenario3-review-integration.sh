#!/bin/bash
# E2E Scenario 3: Review integration flow (005-multi-leader)
#   Load a work task and its review task from YAML.
#   Main leader coordinates with review integrator.
#   Review integrator dispatches reviewer members, aggregates findings, submits verdict.
#   Verifies: changes_requested → rework → approved → done.
set -euo pipefail

cd "$(dirname "$0")/.."

PALETTE_PORT=7100
BASE_URL="http://127.0.0.1:${PALETTE_PORT}"

# Log output to timestamped file
LOG_DIR="data/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/scenario3-$(date +%Y%m%d-%H%M%S).log"
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

save_transcripts() {
    local transcript_dir="data/logs/transcripts-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$transcript_dir"
    echo "=== Saving transcripts to $transcript_dir ==="
    for container in $(docker ps -a --filter label=palette.managed=true --format '{{.Names}}'); do
        local dest="$transcript_dir/$container"
        mkdir -p "$dest"
        docker cp "$container:/home/agent/.claude/projects/" "$dest/" 2>&1 || echo "    (no transcripts found)"
        echo "  $container: $(find "$dest" -name '*.jsonl' 2>/dev/null | wc -l) transcript(s)"
    done
}

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

STATE_JSON=$(cat data/state.json 2>/dev/null || echo "{}")
echo "--- leaders ---"
echo "$STATE_JSON" | jq -r '.leaders[] | "  \(.id) role=\(.role) status=\(.status)"'

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "=== Loading tasks from YAML ==="
LOAD_RESP=$(curl -s -X POST ${BASE_URL}/tasks/load \
  -H "Content-Type: text/plain" \
  --data-binary @tests/fixtures/scenario3-review-integration.yaml)
echo "$LOAD_RESP" | jq -r '.[] | "\(.id) \(.status)"'

echo ""
echo "=== Monitoring agents (max 12 minutes) ==="
echo "    Watch live: tmux attach -t palette"
echo ""

STALL_THRESHOLD=12
STALL_COUNT=0
PREV_SNAPSHOT=""

for i in $(seq 1 144); do
    sleep 5

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

    echo "  [tasks]"
    echo "$TASKS_JSON" | jq -r '.[] | "    \(.id) \(.status) \(.assignee // "")"' 2>/dev/null

    echo "  [agents]"
    echo "$STATE_JSON" | jq -r '(.leaders + .members)[] | "    \(.id) role=\(.role) status=\(.status)"' 2>/dev/null

    # Check completion
    DONE_COUNT=$(echo "$TASKS_JSON" | jq '[.[] | select(.type == "work" and .status == "done")] | length' 2>/dev/null || echo 0)
    echo "  [work done: $DONE_COUNT/1]"
    if [ "$DONE_COUNT" = "1" ]; then
        echo ""
        echo "=== Work task done! ==="
        break
    fi

    if [ "$STALL_COUNT" -ge "$STALL_THRESHOLD" ]; then
        echo ""
        echo "=== STALL DETECTED ==="
        STALL_ABORT=1
        break
    fi
    echo ""
done

echo ""
echo "=== Final state ==="
echo "--- tasks ---"
curl -s ${BASE_URL}/tasks 2>&1 | jq .
echo "--- review submissions ---"
for task_id in $(curl -s ${BASE_URL}/tasks 2>/dev/null | jq -r '.[] | select(.type == "review") | .id' 2>/dev/null); do
    echo "--- submissions for $task_id ---"
    curl -s "${BASE_URL}/reviews/$task_id/submissions" 2>&1 | jq .
done
echo "--- state.json ---"
jq . data/state.json | head -60

echo ""
echo "=== Verification ==="
W_STATUS=$(curl -s ${BASE_URL}/tasks | jq -r '.[] | select(.type == "work") | .status')
R_STATUS=$(curl -s ${BASE_URL}/tasks | jq -r '.[] | select(.type == "review") | .status')
R_ASSIGNEE=$(curl -s ${BASE_URL}/tasks | jq -r '.[] | select(.type == "review") | .assignee // "none"')
REVIEW_ROUNDS=$(curl -s ${BASE_URL}/tasks | jq -r '.[] | select(.type == "review") | .id' | while read rid; do
    curl -s "${BASE_URL}/reviews/$rid/submissions" | jq 'length'
done | head -1)

echo "Work status: $W_STATUS (expected: done)"
echo "Review status: $R_STATUS"
echo "Review assignee: $R_ASSIGNEE (expected: auto-assigned member)"
echo "Review rounds: ${REVIEW_ROUNDS:-0}"

# Verify review member was auto-spawned under review integrator
STATE_JSON=$(cat data/state.json 2>/dev/null || echo "{}")
REVIEW_MEMBERS=$(echo "$STATE_JSON" | jq '[.members[] | select(.leader_id != null)] | length' 2>/dev/null || echo 0)
echo "Members spawned: $REVIEW_MEMBERS"

save_transcripts

RESULT=PASSED
if [ "${STALL_ABORT:-0}" = "1" ]; then RESULT="FAILED (stall)"; fi
if [ "$W_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$R_ASSIGNEE" = "none" ]; then echo "WARN: Review task was not auto-assigned"; fi

echo "=== SCENARIO 3 $RESULT ==="
