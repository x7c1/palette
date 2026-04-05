#!/usr/bin/env bash
# Manual harness to verify Claude Notification hook delivery on macOS.
#
# Usage:
#   tests/e2e/manual/check-macos-notification-manual.sh
#
# Flow:
#   1) Starts a local hook receiver on 127.0.0.1:$PORT.
#   2) Launches interactive Claude in bootstrap container with notification hooks
#      targeting host.docker.internal.
#   3) You trigger a permission prompt manually and then exit Claude.
#   4) Script prints hook hit summary.
set -euo pipefail

PORT="${PORT:-47113}"
SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
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

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "FAIL: missing command: $1"; exit 1; }
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

echo "== Manual notification probe =="
echo "container=$SOURCE_CONTAINER"
echo "receiver=http://127.0.0.1:$PORT"
echo
echo "Claude will start interactively now."
echo "Inside Claude, trigger a permission prompt (example: 'Use Bash to run: pwd'),"
echo "observe prompt, then type /exit."
echo

docker exec -it "$SOURCE_CONTAINER" sh -lc "
set -e
TMP=\$(mktemp -d)
cat > \"\$TMP/settings.json\" <<'JSON'
{
  \"hooks\": {
    \"PreToolUse\": [
      {
        \"matcher\": \"Bash\",
        \"hooks\": [
          {
            \"type\": \"command\",
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/pretool' || true\"
          }
        ]
      }
    ],
    \"SessionStart\": [
      {
        \"hooks\": [
          {
            \"type\": \"command\",
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/session-start' || true\"
          }
        ]
      }
    ],
    \"Stop\": [
      {
        \"hooks\": [
          {
            \"type\": \"command\",
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/stop' || true\"
          }
        ]
      }
    ],
    \"Notification\": [
      {
        \"matcher\": \"permission_prompt\",
        \"hooks\": [
          {
            \"type\": \"command\",
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/notification' || true\"
          }
        ]
      },
      {
        \"hooks\": [
          {
            \"type\": \"command\",
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/notification-any' || true\"
          }
        ]
      }
    ]
  }
}
JSON
/home/developer/.local/bin/claude --permission-mode default --settings \"\$TMP/settings.json\"
rm -rf \"\$TMP\"
"

sleep 1
pretool_hits="$(grep -c '^/hooks/pretool' "$HITS_FILE" || true)"
session_hits="$(grep -c '^/hooks/session-start' "$HITS_FILE" || true)"
stop_hits="$(grep -c '^/hooks/stop' "$HITS_FILE" || true)"
notification_hits="$(grep -c '^/hooks/notification' "$HITS_FILE" || true)"
notification_any_hits="$(grep -c '^/hooks/notification-any' "$HITS_FILE" || true)"

echo
echo "== Hook summary =="
echo "pretool_hits=$pretool_hits"
echo "session_start_hits=$session_hits"
echo "stop_hits=$stop_hits"
echo "notification_hits=$notification_hits"
echo "notification_any_hits=$notification_any_hits"
echo "-- raw paths --"
grep '^/hooks/' "$HITS_FILE" || true

if [[ "$notification_hits" -ge 1 || "$notification_any_hits" -ge 1 ]]; then
  echo "PASS: Notification hook observed"
else
  echo "WARN: Notification hook not observed in this manual run"
fi
