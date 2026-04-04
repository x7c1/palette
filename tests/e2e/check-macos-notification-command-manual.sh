#!/usr/bin/env bash
# Manual harness: verify Claude hook event emission without relying on HTTP transport.
#
# It records SessionStart/PreToolUse/Notification/Stop via command hooks
# to a file inside the bootstrap container, while also keeping optional HTTP hooks.
set -euo pipefail

SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
EVENT_LOG_IN_CONTAINER="${EVENT_LOG_IN_CONTAINER:-/tmp/palette-hook-events-manual.log}"
PORT="${PORT:-47114}"
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
  cid="$(docker compose ps -q claude-code 2>/dev/null || true)"
  if [[ -n "$cid" ]]; then
    SOURCE_CONTAINER="$cid"
    return
  fi
  SOURCE_CONTAINER="$(docker ps --format '{{.ID}} {{.Names}}' | awk '$2 ~ /claude-code/ {print $1; exit}' || true)"
}

require_cmd docker
require_cmd python3
require_cmd curl

detect_source_container
if [[ -z "$SOURCE_CONTAINER" ]]; then
  echo "FAIL: bootstrap container not found"
  echo "HINT: run docker compose up -d claude-code and complete claude login first"
  exit 1
fi

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

echo "== Manual hook emission probe =="
echo "container=$SOURCE_CONTAINER"
echo "container_event_log=$EVENT_LOG_IN_CONTAINER"
echo "host_receiver=http://127.0.0.1:$PORT"
echo
echo "Inside Claude:"
echo "1) send: Use Bash to run exactly: uname -a"
echo "2) wait for permission prompt"
echo "3) keep the session open 10 seconds"
echo "4) try both deny and allow (if possible)"
echo "5) type /exit"
echo

docker exec -it "$SOURCE_CONTAINER" sh -lc "
set -e
TMP=\$(mktemp -d)
: > '$EVENT_LOG_IN_CONTAINER'
cat > \"\$TMP/settings.json\" <<'JSON'
{
  \"hooks\": {
    \"SessionStart\": [
      {
        \"hooks\": [
          { \"type\": \"command\", \"command\": \"echo session_start >> $EVENT_LOG_IN_CONTAINER\" },
          { \"type\": \"command\", \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/session-start' || true\" }
        ]
      }
    ],
    \"PreToolUse\": [
      {
        \"matcher\": \"Bash\",
        \"hooks\": [
          { \"type\": \"command\", \"command\": \"echo pretool_bash >> $EVENT_LOG_IN_CONTAINER\" },
          { \"type\": \"command\", \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/pretool' || true\" }
        ]
      }
    ],
    \"Notification\": [
      {
        \"matcher\": \"permission_prompt\",
        \"hooks\": [
          { \"type\": \"http\", \"url\": \"http://host.docker.internal:${PORT}/hooks/notification-permission\" }
        ]
      }
    ],
    \"Stop\": [
      {
        \"hooks\": [
          { \"type\": \"command\", \"command\": \"echo stop >> $EVENT_LOG_IN_CONTAINER\" },
          { \"type\": \"command\", \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/stop-command' || true\" }
        ]
      }
    ]
  }
}
JSON
/home/developer/.local/bin/claude --debug hooks --permission-mode default --settings \"\$TMP/settings.json\"
rm -rf \"\$TMP\"
"

sleep 1

echo
echo "== Container event log =="
docker exec "$SOURCE_CONTAINER" sh -lc "cat '$EVENT_LOG_IN_CONTAINER' 2>/dev/null || true"

echo
echo "== Host receiver paths =="
grep '^/hooks/' "$HITS_FILE" || true

session_hits="$(grep -c '^/hooks/session-start' "$HITS_FILE" || true)"
pretool_hits="$(grep -c '^/hooks/pretool' "$HITS_FILE" || true)"
notification_permission_hits="$(grep -c '^/hooks/notification-permission' "$HITS_FILE" || true)"
notification_any_hits="$(grep -c '^/hooks/notification-any' "$HITS_FILE" || true)"
stop_hits="$(grep -c '^/hooks/stop-command' "$HITS_FILE" || true)"

echo
echo "== Summary =="
echo "session_start_hits=$session_hits"
echo "pretool_hits=$pretool_hits"
echo "notification_permission_hits=$notification_permission_hits"
echo "notification_any_hits=$notification_any_hits"
echo "stop_hits=$stop_hits"
