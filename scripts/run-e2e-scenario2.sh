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
echo "=== Scenario 2: work -> review -> changes_requested -> work -> review -> approved -> done ==="
echo "=== Sending test message to leader ==="

MESSAGE='Execute a work-review cycle with rejection then approval. Create work task greeting-file and review task review-greeting-file. Send member-a to create /home/agent/greeting.txt with content Hi. After stop event, set work to in_review then submit review as changes_requested with summary Content should be Hello World. After rule engine resets work to in_progress, send member-a to overwrite greeting.txt with Hello World. After stop event, set work to in_review then submit review as approved with summary Content is now correct. Use curl with $PALETTE_URL.'

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

echo "=== Final state ==="
echo "--- tasks ---"
curl -s http://127.0.0.1:7100/tasks 2>&1 | jq .
echo "--- review submissions ---"
for task_id in $(curl -s 'http://127.0.0.1:7100/tasks?type=review' 2>/dev/null | jq -r '.[].id' 2>/dev/null); do
    echo "--- submissions for $task_id ---"
    curl -s "http://127.0.0.1:7100/reviews/$task_id/submissions" 2>&1 | jq .
done

echo ""
echo "=== Verification ==="
WORK_STATUS=$(curl -s http://127.0.0.1:7100/tasks | jq -r '.[] | select(.type == "work") | .status')
REVIEW_TASK_ID=$(curl -s 'http://127.0.0.1:7100/tasks?type=review' | jq -r '[.[] | select(.id | startswith("R-"))] | sort_by(.created_at) | last | .id')
ROUND_COUNT=$(curl -s "http://127.0.0.1:7100/reviews/$REVIEW_TASK_ID/submissions" | jq 'length')
ROUND1_VERDICT=$(curl -s "http://127.0.0.1:7100/reviews/$REVIEW_TASK_ID/submissions" | jq -r '.[0].verdict')
ROUND2_VERDICT=$(curl -s "http://127.0.0.1:7100/reviews/$REVIEW_TASK_ID/submissions" | jq -r '.[1].verdict')

echo "Work task status: $WORK_STATUS (expected: done)"
echo "Review submissions: $ROUND_COUNT (expected: 2)"
echo "Round 1 verdict: $ROUND1_VERDICT (expected: changes_requested)"
echo "Round 2 verdict: $ROUND2_VERDICT (expected: approved)"

RESULT=PASSED
if [ "$WORK_STATUS" != "done" ]; then RESULT=FAILED; fi
if [ "$ROUND_COUNT" != "2" ]; then RESULT=FAILED; fi
if [ "$ROUND1_VERDICT" != "changes_requested" ]; then RESULT=FAILED; fi
if [ "$ROUND2_VERDICT" != "approved" ]; then RESULT=FAILED; fi

echo "=== SCENARIO 2 $RESULT ==="
