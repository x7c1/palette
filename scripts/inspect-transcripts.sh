#!/usr/bin/env bash
# Inspect worker transcripts to find tool calls (especially Bash commands).
# Usage: ./scripts/inspect-transcripts.sh [worker-name-pattern]
#
# Examples:
#   ./scripts/inspect-transcripts.sh          # all workers
#   ./scripts/inspect-transcripts.sh member-3  # specific worker
set -euo pipefail

PATTERN="${1:-}"
TRANSCRIPT_DIR="data/transcripts"

if [[ ! -d "$TRANSCRIPT_DIR" ]]; then
  echo "No transcripts directory found at $TRANSCRIPT_DIR"
  exit 1
fi

for worker_dir in "$TRANSCRIPT_DIR"/*/; do
  worker_name="$(basename "$worker_dir")"
  if [[ -n "$PATTERN" ]] && [[ "$worker_name" != *"$PATTERN"* ]]; then
    continue
  fi

  echo "=== $worker_name ==="
  for jsonl in "$worker_dir"*/*.jsonl; do
    [[ -f "$jsonl" ]] || continue
    echo "  Transcript: $(basename "$jsonl")"

    # Extract tool_use calls from assistant messages
    jq -r '
      select(.type == "assistant")
      | .message.content[]?
      | select(.type == "tool_use")
      | if .name == "Bash" then
          "  [Bash] \(.input.command)"
        elif .name == "Read" then
          "  [Read] \(.input.file_path)"
        elif .name == "Write" then
          "  [Write] \(.input.file_path)"
        elif .name == "Edit" then
          "  [Edit] \(.input.file_path)"
        elif .name == "Glob" then
          "  [Glob] \(.input.pattern)"
        elif .name == "Grep" then
          "  [Grep] \(.input.pattern)"
        elif .name == "Agent" then
          "  [Agent] \(.input.prompt[:80])"
        else
          "  [\(.name)] \(.input | keys | join(", "))"
        end
    ' "$jsonl" 2>/dev/null || true

    # Count permission prompts
    prompts=$(jq -r 'select(.type == "user") | .message.content // "" | select(test("permission")) | "  [PERMISSION_PROMPT]"' "$jsonl" 2>/dev/null | wc -l)
    if [[ "$prompts" -gt 0 ]]; then
      echo "  Permission prompts: $prompts"
    fi
    echo ""
  done
done
