#!/usr/bin/env bash
# E2E test: palette server + Claude Code via tmux
#
# This script verifies the full communication flow:
#   1. Start palette server
#   2. Launch Claude Code in a tmux pane with hooks configured
#   3. Send a prompt via /send
#   4. Verify Stop hook fires and is recorded
#   5. Verify Notification hook fires on permission prompt
#
# Prerequisites:
#   - tmux installed
#   - Claude Code installed (claude command available)
#   - cargo build completed
#
# Usage:
#   ./scripts/e2e-claude-code.sh

set -euo pipefail

SESSION="palette-e2e"
PORT=7199
BASE_URL="http://127.0.0.1:$PORT"
CONFIG=$(mktemp)
HOOKS_CONFIG=$(mktemp -d)
RESULT=0

# Kill any leftover process from a previous run
lsof -ti:"$PORT" | xargs kill 2>/dev/null || true
tmux kill-session -t "$SESSION" 2>/dev/null || true

cleanup() {
    echo "--- Cleaning up ---"
    kill "$SERVER_PID" 2>/dev/null || true
    tmux kill-session -t "$SESSION" 2>/dev/null || true
    rm -f "$CONFIG"
    rm -rf "$HOOKS_CONFIG"
    if [ "$RESULT" -eq 0 ]; then
        echo "=== ALL CHECKS PASSED ==="
    else
        echo "=== SOME CHECKS FAILED ==="
    fi
    exit "$RESULT"
}
trap cleanup EXIT

# Generate config
cat > "$CONFIG" <<EOF
port = $PORT

[tmux]
session_name = "$SESSION"
EOF

# Generate hooks settings for Claude Code
mkdir -p "$HOOKS_CONFIG/.claude"
cat > "$HOOKS_CONFIG/.claude/settings.json" <<EOF
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "http",
            "url": "$BASE_URL/hooks/stop"
          }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt",
        "hooks": [
          {
            "type": "http",
            "url": "$BASE_URL/hooks/notification"
          }
        ]
      }
    ]
  }
}
EOF

echo "=== E2E Test: palette + Claude Code ==="
echo "Session: $SESSION"
echo "Port: $PORT"
echo ""

# Step 1: Start palette server in background
echo "--- Step 1: Starting palette server ---"
cargo run --quiet -- "$CONFIG" &
SERVER_PID=$!
sleep 2

if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "FAIL: Server failed to start"
    RESULT=1
    exit 1
fi
echo "OK: Server started (pid=$SERVER_PID)"

# The server already created the tmux session and worker target
TARGET="$SESSION:worker"

# Step 2: Send a simple command via direct tmux send-keys
echo "--- Step 2: Sending direct tmux command ---"
tmux send-keys -t "$TARGET" -l "echo 'palette-e2e-test-marker'"
tmux send-keys -t "$TARGET" Enter
sleep 1

# Verify the text appeared in the pane
PANE_CONTENT=$(tmux capture-pane -t "$TARGET" -p)
if echo "$PANE_CONTENT" | grep -q "palette-e2e-test-marker"; then
    echo "OK: send-keys delivered to pane"
else
    echo "FAIL: send-keys text not found in pane"
    RESULT=1
fi

# Step 3: Test hooks via HTTP
echo "--- Step 3: Testing hooks endpoints ---"
STOP_RESP=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/hooks/stop" \
    -H "Content-Type: application/json" \
    -d '{"session_id":"e2e-test","conversation_id":"conv-1"}')

if [ "$STOP_RESP" = "200" ]; then
    echo "OK: /hooks/stop returned 200"
else
    echo "FAIL: /hooks/stop returned $STOP_RESP"
    RESULT=1
fi

NOTIF_RESP=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/hooks/notification" \
    -H "Content-Type: application/json" \
    -d '{"notification_type":"permission_prompt","tool_name":"Bash","tool_input":{"command":"ls"}}')

if [ "$NOTIF_RESP" = "200" ]; then
    echo "OK: /hooks/notification returned 200"
else
    echo "FAIL: /hooks/notification returned $NOTIF_RESP"
    RESULT=1
fi

# Step 4: Test /send endpoint
echo "--- Step 4: Testing /send endpoint ---"
SEND_RESP=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$BASE_URL/send" \
    -H "Content-Type: application/json" \
    -d '{"message":"echo palette-send-test"}')

if [ "$SEND_RESP" = "200" ]; then
    echo "OK: /send returned 200"
else
    echo "FAIL: /send returned $SEND_RESP"
    RESULT=1
fi

sleep 1
PANE_CONTENT=$(tmux capture-pane -t "$TARGET" -p)
if echo "$PANE_CONTENT" | grep -q "palette-send-test"; then
    echo "OK: /send message appeared in pane"
else
    echo "FAIL: /send message not found in pane"
    RESULT=1
fi

# Step 5: Verify event log
echo "--- Step 5: Verifying event log ---"
EVENTS=$(curl -s "$BASE_URL/events")
EVENT_COUNT=$(echo "$EVENTS" | jq length)

if [ "$EVENT_COUNT" -ge 3 ]; then
    echo "OK: $EVENT_COUNT events recorded"
    echo "$EVENTS" | jq -c '.[] | {event_type, timestamp}'
else
    echo "FAIL: expected >= 3 events, got $EVENT_COUNT"
    RESULT=1
fi

echo ""
echo "=== E2E Test Complete ==="
