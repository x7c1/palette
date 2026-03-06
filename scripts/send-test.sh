#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Get pane targets from state.json
LEADER_PANE=$(jq -r '.leaders[0].tmux_target' data/state.json)
MEMBER_PANE=$(jq -r '.members[0].tmux_target' data/state.json)
echo "Leader pane: $LEADER_PANE"
echo "Member pane: $MEMBER_PANE"

MESSAGE='Execute a complete work-review cycle: (1) Create a work task titled hello-world, (2) Create a review task titled review-hello-world with depends_on the work task, (3) Send member-a the instruction to create /home/agent/hello.txt with content Hello World, (4) When member-a completes (stop event) update the work task to in_review, (5) Submit the review as approved with verdict approved and summary File created correctly, (6) Update the work task to done. Use the palette-api agent for all API calls.'

echo "=== Sending test message to leader ==="
curl -s -X POST http://127.0.0.1:7100/send \
  -H "Content-Type: application/json" \
  -d "$(jq -n --arg msg "$MESSAGE" '{"member_id":"leader-1","message":$msg}')" || echo "(sent)"

echo ""
echo "=== Monitoring agents (90 seconds) ==="
for i in $(seq 1 18); do
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
echo "--- review submissions ---"
for task_id in $(curl -s 'http://127.0.0.1:7100/tasks?type=review' 2>/dev/null | jq -r '.[].id' 2>/dev/null); do
    echo "--- submissions for $task_id ---"
    curl -s "http://127.0.0.1:7100/reviews/$task_id/submissions" 2>&1 | jq .
done
echo "--- events (last 30 lines) ---"
curl -s http://127.0.0.1:7100/events 2>&1 | jq . | tail -30

echo ""
echo "=== Done ==="
