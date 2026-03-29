#!/usr/bin/env bash
# Debug script for suspend/resume E2E investigation.
# Run while Palette is active to capture worker and pane states.
set -euo pipefail

PALETTE_URL="http://127.0.0.1:7100"

echo "=== Workers ==="
curl -sf "$PALETTE_URL/workers" | jq '.[] | {id, status, terminal_target}' 2>/dev/null || echo "API unavailable"

echo ""
echo "=== Worker Session IDs ==="
curl -sf "$PALETTE_URL/workers" | jq '.[] | {id, session_id}' 2>/dev/null || echo "API unavailable"

echo ""
echo "=== Workflows ==="
curl -sf "$PALETTE_URL/workflows" | jq '.' 2>/dev/null || echo "API unavailable"

echo ""
echo "=== Jobs ==="
curl -sf "$PALETTE_URL/jobs" | jq '.[] | {id, status, assignee_id}' 2>/dev/null || echo "API unavailable"

echo ""
echo "=== Pane Captures ==="
WORKERS=$(curl -sf "$PALETTE_URL/workers" 2>/dev/null || echo "[]")
echo "$WORKERS" | jq -r '.[] | "\(.id) \(.terminal_target)"' 2>/dev/null | while read -r wid target; do
  echo ""
  echo "--- $wid ($target) ---"
  tmux capture-pane -t "$target" -p 2>/dev/null | tail -20 || echo "(pane not found)"
done

echo ""
echo "=== Docker Containers ==="
docker ps -a --filter label=palette.managed=true --format "{{.ID}} {{.Names}} {{.Status}}" 2>/dev/null || echo "docker unavailable"
