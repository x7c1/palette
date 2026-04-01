#!/usr/bin/env bash
# Stop the Palette process only. Does NOT delete data files (DB, logs).
# To fully clean up, run scripts/reset.sh separately.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PID_FILE="data/palette.pid"

kill_pid() {
  local pid=$1
  if kill -0 "$pid" 2>/dev/null; then
    echo "stopping Palette (PID $pid)..."
    kill "$pid" 2>/dev/null || true
    for i in $(seq 1 5); do
      kill -0 "$pid" 2>/dev/null || return 0
      sleep 1
    done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
  fi
}

# Kill by PID file
if [[ -f "$PID_FILE" ]]; then
  kill_pid "$(cat "$PID_FILE")"
  rm -f "$PID_FILE"
fi

# Also kill any process listening on port 7100 (in case PID file was stale/missing)
port_pid=$(lsof -ti :7100 2>/dev/null || true)
if [[ -n "$port_pid" ]]; then
  echo "found process on port 7100 (PID $port_pid), stopping..."
  kill_pid "$port_pid"
fi

# Stop and remove managed containers (same logic as reset.sh)
container_ids=$(docker ps -aq --filter label=palette.managed=true 2>/dev/null || true)
for cid in $container_ids; do
  if [[ -n "$cid" ]]; then
    echo "stopping container ${cid:0:12}..."
    docker rm -f "$cid" &>/dev/null || true
  fi
done
