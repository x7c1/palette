#!/bin/bash
# E2E Scenario 1: Single work-review cycle (happy path)
#   Leader creates work + review tasks, sends member-a an instruction,
#   handles permission prompt, then approves the review → work task becomes done.
set -euo pipefail

cd "$(dirname "$0")/.."

# Clean up previous state
echo "=== Cleanup ==="
lsof -ti:7100 | xargs -r kill 2>/dev/null || true
docker rm -f palette-leader palette-member-a 2>/dev/null || true
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
    docker rm -f palette-leader palette-member-a 2>/dev/null || true
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

# Get pane targets from state.json
LEADER_PANE=$(jq -r '.leaders[0].tmux_target' data/state.json)
MEMBER_PANE=$(jq -r '.members[0].tmux_target' data/state.json)
echo "Leader pane: $LEADER_PANE"
echo "Member pane: $MEMBER_PANE"

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "=== Waiting for Claude Code to initialize (10s) ==="
sleep 10

echo "--- leader pane ---"
tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -5
echo "--- member pane ---"
tmux capture-pane -t "$MEMBER_PANE" -p 2>&1 | grep -v '^$' | tail -5

echo ""
echo "=== Sending test message to leader ==="

MESSAGE='Execute a complete work-review cycle: (1) Create a work task titled hello-world, (2) Create a review task titled review-hello-world with depends_on the work task, (3) Send member-a the instruction to create /home/agent/hello.txt with content Hello World, (4) When member-a completes (stop event) update the work task to in_review, (5) Submit the review as approved with verdict approved and summary File created correctly, (6) Update the work task to done. Use the palette-api agent for all API calls.'

curl -s -X POST http://127.0.0.1:7100/send \
  -H "Content-Type: application/json" \
  -d "$(jq -n --arg msg "$MESSAGE" '{"member_id":"leader-1","message":$msg}')" || echo "(sent)"

echo ""
echo "=== Monitoring agents (max 5 minutes) ==="
for i in $(seq 1 60); do
    sleep 5
    echo "--- ${i}x5s: leader pane ---"
    tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -5
    echo "--- ${i}x5s: member pane ---"
    tmux capture-pane -t "$MEMBER_PANE" -p 2>&1 | grep -v '^$' | tail -5

    # Check if work task reached "done"
    if curl -s http://127.0.0.1:7100/tasks 2>/dev/null | jq -e '.[] | select(.type == "work" and .status == "done")' >/dev/null 2>&1; then
        echo "=== Work task done ==="
        break
    fi
    echo ""
done

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
echo "=== Done ==="
