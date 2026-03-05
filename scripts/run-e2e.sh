#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Clean up previous state
docker rm -f palette-leader palette-member-a 2>/dev/null || true
tmux kill-session -t palette 2>/dev/null || true
rm -f data/state.json data/palette.db
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

sleep 8

if ! kill -0 $PALETTE_PID 2>/dev/null; then
    echo "ERROR: palette exited unexpectedly"
    exit 1
fi

echo ""
echo "=== Palette running (PID=$PALETTE_PID) ==="

echo "--- tmux windows ---"
tmux list-windows -t palette 2>&1

echo "--- containers ---"
docker ps --filter label=palette.managed=true --format '{{.Names}} {{.Status}}' 2>&1

echo ""
echo "--- leader pane (last 10 lines) ---"
tmux capture-pane -t palette:leader -p 2>&1 | grep -v '^$' | tail -10

echo ""
echo "--- member pane (last 10 lines) ---"
tmux capture-pane -t palette:member-a -p 2>&1 | grep -v '^$' | tail -10

echo ""
echo "=== Sending test message to leader ==="
curl -s -X POST http://127.0.0.1:7100/send \
  -H "Content-Type: application/json" \
  -d '{"member_id":"leader-1","message":"You are a leader agent. Create a work task titled hello-world and assign it to member-a. Then send member-a the instruction: create a file called hello.txt with the content Hello World."}' | python3 -m json.tool 2>&1 || echo "(sent)"

echo ""
echo "=== Waiting 30 seconds for agents to work ==="
for i in $(seq 1 6); do
    sleep 5
    echo "--- ${i}0s: leader pane ---"
    tmux capture-pane -t palette:leader -p 2>&1 | grep -v '^$' | tail -5
    echo "--- ${i}0s: member pane ---"
    tmux capture-pane -t palette:member-a -p 2>&1 | grep -v '^$' | tail -5
    echo "--- events ---"
    curl -s http://127.0.0.1:7100/events 2>&1 | python3 -m json.tool 2>&1 | tail -10
    echo ""
done

echo "=== Final state ==="
echo "--- tasks ---"
curl -s http://127.0.0.1:7100/tasks 2>&1 | python3 -m json.tool 2>&1
echo "--- state.json ---"
cat data/state.json | python3 -m json.tool 2>&1 | head -40

echo ""
echo "=== Done ==="
