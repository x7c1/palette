#!/usr/bin/env bash
# Reset palette to a clean state: stop containers, kill tmux session, remove data files.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DB_FILE="$ROOT_DIR/data/palette.db"
SESSION_NAME="palette"

collect_protected_pids() {
  # Keep current process lineage alive so reset invoked from a test script
  # does not terminate itself.
  local current="$$"
  local out=" $current "
  while [[ "$current" -gt 1 ]]; do
    local parent
    parent="$(ps -o ppid= -p "$current" 2>/dev/null | tr -d ' ' || true)"
    if [[ -z "$parent" || "$parent" -le 1 ]]; then
      break
    fi
    out="${out}${parent} "
    current="$parent"
  done
  echo "$out"
}

stop_other_e2e_scripts() {
  local protected
  protected="$(collect_protected_pids)"
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    local pid
    pid="$(echo "$line" | awk '{print $1}')"
    local cmd
    cmd="$(echo "$line" | cut -d' ' -f2-)"
    [[ -z "$pid" ]] && continue
    if [[ "$protected" == *" $pid "* ]]; then
      continue
    fi
    if [[ "$cmd" == *"/tests/e2e/run-"*".sh"* ]]; then
      echo "stopping other e2e script (PID $pid)..."
      kill "$pid" 2>/dev/null || true
    fi
  done < <(ps -axo pid=,command= 2>/dev/null || true)
}

# Stop other running E2E scripts first to avoid parallel runs fighting
# over shared pid/log/data paths.
stop_other_e2e_scripts

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

# Kill tmux sessions (main + any stale test sessions)
for sess in $(tmux ls -F '#{session_name}' 2>/dev/null | grep "^palette" || true); do
  echo "killing tmux session '$sess'..."
  tmux kill-session -t "$sess" 2>/dev/null || true
done

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
