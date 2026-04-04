#!/usr/bin/env bash
# Minimal connectivity check for Palette's current networking assumptions.
#
# What it tests:
#   1. Host can reach a server bound to 127.0.0.1
#   2. A normal Docker container cannot reach host 127.0.0.1
#   3. A --network host container can or cannot reach host 127.0.0.1
#   4. host.docker.internal works or not on this machine
#
# This isolates the core question behind Palette's current design:
# worker containers are expected to call back to PALETTE_URL=http://127.0.0.1:PORT
# and Palette currently launches them with --network host.
set -euo pipefail

IMAGE="${IMAGE:-curlimages/curl:8.12.1}"
PORT="${PORT:-47100}"
TMP_DIR="$(mktemp -d)"
SERVER_LOG="$TMP_DIR/server.log"
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

cat > "$TMP_DIR/index.html" <<'EOF'
palette-loopback-ok
EOF

python3 -m http.server "$PORT" --bind 127.0.0.1 --directory "$TMP_DIR" >"$SERVER_LOG" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 20); do
  if curl -fsS "http://127.0.0.1:$PORT/" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo "== Host check =="
HOST_OK=0
if run_probe "host -> http://127.0.0.1:$PORT" \
  curl -fsS "http://127.0.0.1:$PORT/"; then
  HOST_OK=1
fi

echo
echo "== Container checks =="
BRIDGE_OK=0
HOST_NETWORK_OK=0
HOST_INTERNAL_OK=0

if run_probe "bridge container -> http://127.0.0.1:$PORT (expected to fail)" \
  docker run --rm "$IMAGE" -fsS "http://127.0.0.1:$PORT/"; then
  BRIDGE_OK=1
fi

if run_probe "host-network container -> http://127.0.0.1:$PORT" \
  docker run --rm --network host "$IMAGE" -fsS "http://127.0.0.1:$PORT/"; then
  HOST_NETWORK_OK=1
fi

if run_probe "bridge container -> http://host.docker.internal:$PORT" \
  docker run --rm "$IMAGE" -fsS "http://host.docker.internal:$PORT/"; then
  HOST_INTERNAL_OK=1
fi

echo
echo "== Summary =="
echo "host_localhost=$HOST_OK"
echo "bridge_to_127001=$BRIDGE_OK"
echo "host_network_to_127001=$HOST_NETWORK_OK"
echo "host_docker_internal=$HOST_INTERNAL_OK"

echo
echo "== Interpretation =="
if [[ "$HOST_OK" -ne 1 ]]; then
  echo "Host cannot reach its own 127.0.0.1 test server. The local setup is broken."
  exit 1
fi

if [[ "$BRIDGE_OK" -eq 0 ]]; then
  echo "A normal container cannot use 127.0.0.1 to reach the host. This is expected."
else
  echo "A normal container reached 127.0.0.1 on the host. That is unusual; inspect your Docker setup."
fi

if [[ "$HOST_NETWORK_OK" -eq 1 ]]; then
  echo "A --network host container reached host 127.0.0.1."
  echo "Palette's current Linux-style networking assumption may work on this machine."
else
  echo "A --network host container could not reach host 127.0.0.1."
  echo "Palette's current worker callback design will not work here as-is."
fi

if [[ "$HOST_INTERNAL_OK" -eq 1 ]]; then
  echo "host.docker.internal is available as an alternative host route."
  echo "This is useful for diagnosis, but Palette currently assumes 127.0.0.1."
else
  echo "host.docker.internal is not available or the host service was unreachable through it."
fi
