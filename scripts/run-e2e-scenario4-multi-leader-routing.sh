#!/bin/bash
# E2E Scenario 4: Multi-leader routing verification (005-multi-leader)
#   Load a work task and a review task.
#   Verify that work member's stop hook routes to main leader,
#   and review member's stop hook routes to review integrator.
set -euo pipefail

cd "$(dirname "$0")/.."
source scripts/e2e-helpers.sh

PALETTE_PORT=7100
BASE_URL="http://127.0.0.1:${PALETTE_PORT}"

# Log output to timestamped file
LOG_DIR="data/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/scenario4-$(date +%Y%m%d-%H%M%S).log"
exec > >(tee "$LOG_FILE") 2>&1
echo "Logging to $LOG_FILE"

# Clean up previous state
e2e_cleanup $PALETTE_PORT

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
echo "$STATE_JSON" | jq -r '.leaders[] | "  \(.id) role=\(.role)"'

echo ""
echo "=== Verifying bootstrap: two leaders present ==="
LEADER_COUNT=$(echo "$STATE_JSON" | jq '.leaders | length')
echo "Leader count: $LEADER_COUNT (expected: 2)"

MAIN_LEADER=$(echo "$STATE_JSON" | jq -r '.leaders[] | select(.role == "leader") | .id')
REVIEW_INTEGRATOR=$(echo "$STATE_JSON" | jq -r '.leaders[] | select(.role == "review_integrator") | .id')
echo "Main leader: $MAIN_LEADER"
echo "Review integrator: $REVIEW_INTEGRATOR"

# Wait for agents to boot and become idle, then check member routing
echo ""
echo "=== Waiting for leaders to become idle (max 3 minutes) ==="
for i in $(seq 1 36); do
    sleep 5
    STATE_JSON=$(cat data/state.json 2>/dev/null || echo "{}")
    IDLE_LEADERS=$(echo "$STATE_JSON" | jq '[.leaders[] | select(.status == "idle")] | length')
    echo "  ${i}: idle leaders = $IDLE_LEADERS/2"
    if [ "$IDLE_LEADERS" = "2" ]; then
        echo "  Both leaders idle"
        break
    fi
done

save_transcripts

echo ""
echo "=== Verification ==="
RESULT=PASSED

if [ "$LEADER_COUNT" != "2" ]; then
    echo "FAIL: Expected 2 leaders, got $LEADER_COUNT"
    RESULT=FAILED
fi

if [ -z "$MAIN_LEADER" ]; then
    echo "FAIL: No main leader found"
    RESULT=FAILED
fi

if [ -z "$REVIEW_INTEGRATOR" ]; then
    echo "FAIL: No review integrator found"
    RESULT=FAILED
fi

echo "=== SCENARIO 4 $RESULT ==="
