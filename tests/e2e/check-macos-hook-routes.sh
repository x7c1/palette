#!/usr/bin/env bash
# Diagnose callback reachability for hook endpoints from Docker containers.
#
# This script validates transport reachability for command-style hooks:
# - SessionStart
# - Stop
# - Notification
#
# It does not execute Claude Code itself; it isolates network path behavior.
set -euo pipefail

IMAGE="${IMAGE:-curlimages/curl:8.12.1}"
PORT="${PORT:-47110}"
TMP_DIR="$(mktemp -d)"
SERVER_LOG="$TMP_DIR/server.log"
HITS_FILE="$TMP_DIR/hits.log"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "FAIL: required command not found: $1"
    exit 1
  fi
}

run_probe() {
  local name="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    echo "PASS: $name"
    return 0
  fi
  echo "FAIL: $name"
  return 1
}

require_cmd docker
require_cmd curl
require_cmd python3

cat > "$TMP_DIR/server.py" <<'PY'
from http.server import BaseHTTPRequestHandler, HTTPServer
import os

hits_file = os.environ["HITS_FILE"]
port = int(os.environ["PORT"])

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        _ = self.rfile.read(length) if length > 0 else b""
        with open(hits_file, "a", encoding="utf-8") as f:
            f.write(self.path + "\n")
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, fmt, *args):
        return

HTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY

touch "$HITS_FILE"
PORT="$PORT" HITS_FILE="$HITS_FILE" python3 "$TMP_DIR/server.py" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 20); do
  if curl -fsS -X POST -H "Content-Type: application/json" \
    -d '{"probe":"host"}' "http://127.0.0.1:$PORT/hooks/session-start" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo "== Host baseline =="
baseline_ok=0
for _ in $(seq 1 10); do
  if curl -fsS -X POST -H "Content-Type: application/json" \
    -d '{"session_id":"host-baseline"}' \
    "http://127.0.0.1:$PORT/hooks/session-start" >/dev/null 2>&1; then
    baseline_ok=1
    break
  fi
  sleep 0.2
done
if [[ "$baseline_ok" -eq 1 ]]; then
  echo "PASS: host -> session-start endpoint"
else
  echo "FAIL: host -> session-start endpoint"
  exit 1
fi

echo
echo "== Container probes (host.docker.internal) =="

# SessionStart command-style hook emulation.
run_probe "bridge container command-hook -> /hooks/session-start via host.docker.internal" \
  docker run --rm "$IMAGE" sh -lc \
  "echo '{\"session_id\":\"worker-1\",\"source\":\"command\"}' | \
   curl -fsS -X POST -H 'Content-Type: application/json' -d @- \
   http://host.docker.internal:$PORT/hooks/session-start"

# Stop / Notification command-style hook emulation.
run_probe "bridge container command-hook -> /hooks/stop via host.docker.internal" \
  docker run --rm "$IMAGE" sh -lc \
  "echo '{\"session_id\":\"worker-1\"}' | \
   curl -fsS -X POST -H 'Content-Type: application/json' -d @- \
   http://host.docker.internal:$PORT/hooks/stop"

run_probe "bridge container command-hook -> /hooks/notification via host.docker.internal" \
  docker run --rm "$IMAGE" sh -lc \
  "echo '{\"notification_type\":\"permission_prompt\"}' | \
   curl -fsS -X POST -H 'Content-Type: application/json' -d @- \
   http://host.docker.internal:$PORT/hooks/notification"

echo
echo "== Container probes (127.0.0.1 reference) =="
if docker run --rm "$IMAGE" -fsS -X POST -H "Content-Type: application/json" \
  -d '{"session_id":"worker-1"}' "http://127.0.0.1:$PORT/hooks/stop" >/dev/null 2>&1; then
  echo "WARN: bridge container reached host 127.0.0.1 unexpectedly"
else
  echo "PASS: bridge container -> host 127.0.0.1 failed (expected)"
fi

echo
echo "== Server hit summary =="
session_hits=$(grep -c '^/hooks/session-start' "$HITS_FILE" || true)
stop_hits=$(grep -c '^/hooks/stop' "$HITS_FILE" || true)
notification_hits=$(grep -c '^/hooks/notification' "$HITS_FILE" || true)
echo "session_start_hits=$session_hits"
echo "stop_hits=$stop_hits"
echo "notification_hits=$notification_hits"

if [[ "$session_hits" -lt 1 || "$stop_hits" -lt 1 || "$notification_hits" -lt 1 ]]; then
  echo "FAIL: one or more hook endpoints were not reached from container probes"
  exit 1
fi

echo "PASS: command-style hook routes are reachable via host.docker.internal"
