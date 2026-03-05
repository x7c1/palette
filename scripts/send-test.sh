#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Get pane targets from state.json
LEADER_PANE=$(jq -r '.leaders[0].tmux_target' data/state.json)
MEMBER_PANE=$(jq -r '.members[0].tmux_target' data/state.json)
echo "Leader pane: $LEADER_PANE"
echo "Member pane: $MEMBER_PANE"

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
    echo ""
done

echo "=== Final state ==="
echo "--- tasks ---"
curl -s http://127.0.0.1:7100/tasks 2>&1 | jq .
echo "--- events (last 20 lines) ---"
curl -s http://127.0.0.1:7100/events 2>&1 | jq . | tail -20

echo ""
echo "=== Done ==="
