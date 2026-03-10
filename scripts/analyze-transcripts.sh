#!/bin/bash
# Analyze saved transcripts from E2E scenario tests.
# Usage: scripts/analyze-transcripts.sh [transcript_dir]
#   If no dir given, uses the most recent transcripts-* directory.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -n "${1:-}" ]; then
    TRANSCRIPT_DIR="$1"
else
    TRANSCRIPT_DIR=$(ls -dt data/logs/transcripts-* 2>/dev/null | head -1)
fi

if [ -z "$TRANSCRIPT_DIR" ] || [ ! -d "$TRANSCRIPT_DIR" ]; then
    echo "No transcript directory found."
    exit 1
fi

echo "=== Analyzing transcripts in $TRANSCRIPT_DIR ==="
echo ""

# Deduplicate: transcripts are in a shared volume, so all containers have the same files.
# Use the first container's copy for each unique session ID.
declare -A SEEN_SESSIONS

for container_dir in "$TRANSCRIPT_DIR"/palette-*/; do
    container=$(basename "$container_dir")
    for jsonl in "$container_dir"/projects/-home-agent/*.jsonl; do
        [ -f "$jsonl" ] || continue
        session_id=$(basename "$jsonl" .jsonl)

        # Skip if we already processed this session
        if [ "${SEEN_SESSIONS[$session_id]:-}" = "1" ]; then
            continue
        fi
        SEEN_SESSIONS[$session_id]=1

        echo "=== Session: $session_id ==="
        echo "--- File: $jsonl ---"
        echo "Lines: $(wc -l < "$jsonl")"
        echo ""

        # Show user messages (task instructions)
        echo "[User messages]"
        jq -r 'select(.type == "user") | .message.content // .message | if type == "array" then .[] | select(.type == "text") | .text else . end' "$jsonl" 2>/dev/null | head -20
        echo ""

        # Show assistant text responses
        echo "[Assistant responses]"
        jq -r 'select(.type == "assistant") | .message.content[]? | select(.type == "text") | .text' "$jsonl" 2>/dev/null | head -40
        echo ""

        # Show tool calls
        echo "[Tool calls]"
        jq -r 'select(.type == "assistant") | .message.content[]? | select(.type == "tool_use") | "  \(.name): \(.input | tostring | .[0:120])"' "$jsonl" 2>/dev/null
        echo ""

        # Show tool results (errors only)
        echo "[Tool errors]"
        jq -r 'select(.type == "tool_result" and .is_error == true) | "  ERROR: \(.content | tostring | .[0:200])"' "$jsonl" 2>/dev/null
        echo ""

        echo "---"
        echo ""
    done
done

echo "=== Analysis complete ==="
