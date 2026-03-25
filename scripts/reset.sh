#!/usr/bin/env bash
# Reset palette to a clean state: stop containers, kill tmux session, remove data files.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

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

# Stop and remove managed containers (identified by Docker label)
container_ids=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null || true)
for cid in $container_ids; do
  if [[ -n "$cid" ]]; then
    echo "stopping container ${cid:0:12}..."
    docker rm -f "$cid" &>/dev/null || true
  fi
done

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
for f in "$DB_FILE" "${DB_FILE}-wal" "${DB_FILE}-shm"; do
  if [[ -f "$f" ]]; then
    echo "removing $(basename "$f")..."
    rm "$f"
  fi
done

echo "reset complete."
