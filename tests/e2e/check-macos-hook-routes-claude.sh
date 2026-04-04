#!/usr/bin/env bash
# Verify Claude Code real hook delivery from a Linux container to host on macOS.
#
# This script runs `claude -p` inside an already logged-in bootstrap container,
# using hooks that target host.docker.internal. It validates real hook traffic.
set -euo pipefail

PORT="${PORT:-47111}"
SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
TMP_DIR="$(mktemp -d)"
HITS_FILE="$TMP_DIR/hits.log"
SERVER_LOG="$TMP_DIR/server.log"
SERVER_PID=""
FAIL_COUNT=0
WARN_COUNT=0

cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

warn() {
  echo "WARN: $*"
  WARN_COUNT=$((WARN_COUNT + 1))
}

info() {
  echo "INFO: $*"
}

pass() {
  echo "PASS: $*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "FAIL: required command not found: $1"
    exit 1
  fi
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
require_cmd curl
require_cmd python3

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
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length else b""
        with open(hits, "a", encoding="utf-8") as f:
            f.write(self.path + "\n")
            if body:
                try:
                    f.write(body.decode("utf-8", errors="replace") + "\n")
                except Exception:
                    f.write("<decode-error>\n")
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, fmt, *args):
        return

HTTPServer(("127.0.0.1", port), H).serve_forever()
PY

touch "$HITS_FILE"
PORT="$PORT" HITS_FILE="$HITS_FILE" python3 "$TMP_DIR/server.py" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 20); do
  if curl -fsS -X POST -H "Content-Type: application/json" \
    -d '{"probe":"host"}' "http://127.0.0.1:$PORT/hooks/session-start" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

echo "== Real Claude hook probe =="
echo "source_container=$SOURCE_CONTAINER"
echo "port=$PORT"

docker exec "$SOURCE_CONTAINER" sh -lc "
  set -e
  test -x /home/developer/.local/bin/claude
  TMP=\$(mktemp -d)
  cat > \"\$TMP/settings-primary.json\" <<'JSON'
{
  \"permissions\": {
    \"allow\": [
      \"Read(/home/developer/**)\",
      \"Read(/projects/**)\",
      \"Glob(/home/developer/**)\",
      \"Glob(/projects/**)\"
    ]
  },
  \"hooks\": {
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

  cat > \"\$TMP/settings-command-stop.json\" <<'JSON'
{
  \"permissions\": {
    \"allow\": [
      \"Read(/home/developer/**)\",
      \"Read(/projects/**)\",
      \"Glob(/home/developer/**)\",
      \"Glob(/projects/**)\"
    ]
  },
  \"hooks\": {
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
            \"command\": \"curl -sf -X POST -H 'Content-Type: application/json' -d @- 'http://host.docker.internal:${PORT}/hooks/stop-command' || true\"
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

  /home/developer/.local/bin/claude -p \
    --permission-mode default \
    --settings \"\$TMP/settings-primary.json\" \
    \"Return exactly: hook-probe-ok\" >/tmp/claude-hook-probe.out 2>/tmp/claude-hook-probe.err || true

  # Best-effort second run that tends to raise permission prompts and notification hooks.
  /home/developer/.local/bin/claude -p \
    --permission-mode default \
    --settings \"\$TMP/settings-primary.json\" \
    \"Use Bash to run: pwd\" >/tmp/claude-hook-probe-tool.out 2>/tmp/claude-hook-probe-tool.err || true

  # Fallback run without -p. Some hook types are only emitted in interactive lifecycle.
  printf 'Return exactly: interactive-hook-probe\n/exit\n' | \
    /home/developer/.local/bin/claude \
      --permission-mode default \
      --settings \"\$TMP/settings-primary.json\" \
      >/tmp/claude-hook-probe-interactive.out 2>/tmp/claude-hook-probe-interactive.err || true

  # Control run: Stop as command hook for event-emission/policy split.
  /home/developer/.local/bin/claude -p \
    --permission-mode default \
    --settings \"\$TMP/settings-command-stop.json\" \
    \"Return exactly: hook-probe-stop-command\" >/tmp/claude-hook-probe-stop-command.out 2>/tmp/claude-hook-probe-stop-command.err || true

  # Permission-prompt candidate: non-print mode with a tool request, killed after timeout.
  if command -v timeout >/dev/null 2>&1; then
    timeout 20s /home/developer/.local/bin/claude \
      --permission-mode default \
      --settings \"\$TMP/settings-primary.json\" \
      \"Use Bash to run: pwd\" >/tmp/claude-hook-probe-permission.out 2>/tmp/claude-hook-probe-permission.err || true
  else
    /home/developer/.local/bin/claude -p \
      --permission-mode default \
      --settings \"\$TMP/settings-primary.json\" \
      \"Use Bash to run: pwd\" >/tmp/claude-hook-probe-permission.out 2>/tmp/claude-hook-probe-permission.err || true
  fi

  rm -rf \"\$TMP\"
"

sleep 1

session_hits="$(grep -c '^/hooks/session-start' "$HITS_FILE" || true)"
stop_hits="$(grep -c '^/hooks/stop' "$HITS_FILE" || true)"
stop_command_hits="$(grep -c '^/hooks/stop-command' "$HITS_FILE" || true)"
notification_hits="$(grep -c '^/hooks/notification' "$HITS_FILE" || true)"
notification_any_hits="$(grep -c '^/hooks/notification-any' "$HITS_FILE" || true)"

echo
echo "== Hit summary =="
echo "session_start_hits=$session_hits"
echo "stop_hits=$stop_hits"
echo "stop_command_hits=$stop_command_hits"
echo "notification_hits=$notification_hits"
echo "notification_any_hits=$notification_any_hits"

if [[ "$session_hits" -ge 1 ]]; then
  pass "SessionStart hook reached host via host.docker.internal"
else
  fail "SessionStart hook did not reach host"
fi

if [[ "$stop_hits" -ge 1 ]]; then
  pass "Stop hook reached host via host.docker.internal"
else
  fail "Stop hook did not reach host"
fi

if [[ "$stop_command_hits" -ge 1 ]]; then
  pass "Stop(command) hook reached host via host.docker.internal"
else
  warn "Stop(command) hook was not observed; Stop event emission may be absent in this mode"
fi

if [[ "$notification_hits" -ge 1 ]]; then
  pass "Notification hook reached host via host.docker.internal"
else
  info "Notification hook was not observed in this run (optional signal)"
fi

if [[ "$notification_any_hits" -ge 1 ]]; then
  pass "Notification(any) hook observed (non-matcher route)"
else
  info "No Notification(any) hooks observed; Claude notifications are optional in this probe"
fi

echo
echo "== Result =="
echo "warn=$WARN_COUNT fail=$FAIL_COUNT"
if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
fi
