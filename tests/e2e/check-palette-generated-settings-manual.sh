#!/usr/bin/env bash
# Manual probe using the same hook registration path as palette:
# template expansion -> /home/agent/.claude/settings.json in container.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
WORKER_ID="${WORKER_ID:-worker-a}"
PORT="${PORT:-47118}"
TMP_DIR="$(mktemp -d)"
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
  command -v "$1" >/dev/null 2>&1 || { echo "FAIL: missing command: $1"; exit 1; }
}

detect_source_container() {
  if [[ -n "$SOURCE_CONTAINER" ]]; then
    return
  fi
  local cid
  cid="$(docker compose -f "$ROOT_DIR/docker-compose.yml" ps -q claude-code 2>/dev/null || true)"
  if [[ -n "$cid" ]]; then
    SOURCE_CONTAINER="$cid"
    return
  fi
  SOURCE_CONTAINER="$(docker ps --format '{{.ID}} {{.Names}}' | awk '$2 ~ /claude-code/ {print $1; exit}' || true)"
}

require_cmd docker
require_cmd python3
require_cmd sed

detect_source_container
if [[ -z "$SOURCE_CONTAINER" ]]; then
  echo "FAIL: bootstrap container not found"
  echo "HINT: run docker compose up -d claude-code and complete claude login first"
  exit 1
fi

TEMPLATE="$ROOT_DIR/config/hooks/worker-settings.json"
if [[ ! -f "$TEMPLATE" ]]; then
  echo "FAIL: template not found: $TEMPLATE"
  exit 1
fi

SESSION_URL="http://host.docker.internal:${PORT}/hooks/session-start?worker_id=${WORKER_ID}"
STOP_URL="http://host.docker.internal:${PORT}/hooks/stop?worker_id=${WORKER_ID}"
NOTIF_URL="http://host.docker.internal:${PORT}/hooks/notification?worker_id=${WORKER_ID}"

SETTINGS="$TMP_DIR/settings.json"
sed -e "s#{{PALETTE_SESSION_START_URL}}#${SESSION_URL}#g" \
    -e "s#{{PALETTE_STOP_URL}}#${STOP_URL}#g" \
    -e "s#{{PALETTE_NOTIFICATION_URL}}#${NOTIF_URL}#g" \
    "$TEMPLATE" > "$SETTINGS"

cat > "$TMP_DIR/server.py" <<'PY'
from http.server import BaseHTTPRequestHandler, HTTPServer
import os

hits = os.environ["HITS_FILE"]
port = int(os.environ["PORT"])

class H(BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n) if n else b""
        with open(hits, "a", encoding="utf-8") as f:
            f.write(self.path + "\n")
            if body:
                f.write(body.decode("utf-8", errors="replace") + "\n")
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")
    def log_message(self, fmt, *args):
        return

HTTPServer(("127.0.0.1", port), H).serve_forever()
PY

touch "$HITS_FILE"
PORT="$PORT" HITS_FILE="$HITS_FILE" python3 "$TMP_DIR/server.py" >/dev/null 2>&1 &
SERVER_PID=$!

echo "== Palette-generated settings manual probe =="
echo "container=$SOURCE_CONTAINER"
echo "template=$TEMPLATE"
echo "worker_id=$WORKER_ID"
echo "receiver=http://127.0.0.1:$PORT"
echo
echo "Claude will start now with the generated settings."
echo "Do this in Claude:"
echo "1) Send: Use Bash to run exactly: uname -a"
echo "2) If permission prompt appears, wait ~10s, then try deny/allow"
echo "3) Then /exit"
echo

docker cp "$SETTINGS" "$SOURCE_CONTAINER:/tmp/palette-generated-settings.json"
docker exec -it "$SOURCE_CONTAINER" sh -lc '
set -e
cp /tmp/palette-generated-settings.json /home/developer/.claude/settings.json
/home/developer/.local/bin/claude --debug hooks --permission-mode default
'

sleep 1
session_hits="$(grep -c '^/hooks/session-start' "$HITS_FILE" || true)"
stop_hits="$(grep -c '^/hooks/stop' "$HITS_FILE" || true)"
notification_hits="$(grep -c '^/hooks/notification' "$HITS_FILE" || true)"

echo
echo "== Hook summary =="
echo "session_start_hits=$session_hits"
echo "stop_hits=$stop_hits"
echo "notification_hits=$notification_hits"
echo "-- raw paths --"
grep '^/hooks/' "$HITS_FILE" || true
