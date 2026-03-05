#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Clean up previous state
echo "=== Cleanup ==="
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
curl -s -X POST http://127.0.0.1:7100/send \
  -H "Content-Type: application/json" \
  -d '{"member_id":"leader-1","message":"You are a leader agent. Create a work task titled hello-world and assign it to member-a. Then send member-a the instruction: create a file called hello.txt with the content Hello World."}' || echo "(sent)"

echo ""
echo "=== Monitoring agents (60 seconds) ==="
for i in $(seq 1 12); do
    sleep 5
    echo "--- ${i}x5s: leader pane ---"
    tmux capture-pane -t "$LEADER_PANE" -p 2>&1 | grep -v '^$' | tail -5
    echo "--- ${i}x5s: member pane ---"
    tmux capture-pane -t "$MEMBER_PANE" -p 2>&1 | grep -v '^$' | tail -5
    echo "--- events ---"
    curl -s http://127.0.0.1:7100/events 2>&1 | jq . | tail -10
    echo ""
done

echo "=== Final state ==="
echo "--- tasks ---"
curl -s http://127.0.0.1:7100/tasks 2>&1 | jq .
echo "--- state.json ---"
jq . data/state.json | head -40

echo ""
echo "=== Done ==="
