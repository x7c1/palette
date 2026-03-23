#!/usr/bin/env bash
# Reset palette to a clean state: stop containers, kill tmux session, remove data files.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

STATE_FILE="$ROOT_DIR/data/state.json"
DB_FILE="$ROOT_DIR/data/palette.db"
SESSION_NAME="palette"

# Stop Palette process if running
port_pid=$(lsof -ti :7100 2>/dev/null || true)
if [[ -n "$port_pid" ]]; then
  echo "stopping Palette process (PID $port_pid)..."
  kill "$port_pid" 2>/dev/null || true
  sleep 1
  kill -0 "$port_pid" 2>/dev/null && kill -9 "$port_pid" 2>/dev/null || true
fi

# Stop and remove containers listed in state.json
if [[ -f "$STATE_FILE" ]]; then
  container_ids=$(jq -r '(.supervisors + .members)[] | .container_id' "$STATE_FILE" 2>/dev/null || true)
  for cid in $container_ids; do
    if [[ -n "$cid" ]] && docker inspect "$cid" &>/dev/null; then
      echo "stopping container ${cid:0:12}..."
      docker rm -f "$cid" &>/dev/null || true
    fi
  done
fi

# Kill tmux session
if tmux has-session -t "$SESSION_NAME" 2>/dev/null; then
  echo "killing tmux session '$SESSION_NAME'..."
  tmux kill-session -t "$SESSION_NAME"
fi

# Remove plans directory (including git history from previous runs)
PLANS_DIR="$ROOT_DIR/data/plans"
if [[ -d "$PLANS_DIR" ]]; then
  echo "removing plans directory..."
  rm -rf "$PLANS_DIR"
fi

# Remove data files (including SQLite WAL/SHM)
for f in "$STATE_FILE" "$DB_FILE" "${DB_FILE}-wal" "${DB_FILE}-shm"; do
  if [[ -f "$f" ]]; then
    echo "removing $(basename "$f")..."
    rm "$f"
  fi
done

echo "reset complete."
