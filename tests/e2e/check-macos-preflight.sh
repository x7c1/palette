#!/usr/bin/env bash
# Preflight diagnostics for macOS E2E runs.
# This script is intentionally lightweight and fail-fast for hard blockers.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

pass() {
  echo "PASS: $*"
  PASS_COUNT=$((PASS_COUNT + 1))
}

fail() {
  echo "FAIL: $*"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

warn() {
  echo "WARN: $*"
  WARN_COUNT=$((WARN_COUNT + 1))
}

require_cmd() {
  local cmd="$1"
  if command -v "$cmd" >/dev/null 2>&1; then
    pass "command available: $cmd"
  else
    fail "required command missing: $cmd"
  fi
}

echo "== macOS E2E preflight =="
echo "OS: $(uname -s)"
echo "Arch: $(uname -m)"
echo

for cmd in bash jq sqlite3 tmux docker curl lsof; do
  require_cmd "$cmd"
done

if [[ "${BASH_VERSINFO[0]:-0}" -ge 4 ]]; then
  pass "bash version is >= 4 (${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]})"
else
  fail "bash version is < 4 (${BASH_VERSINFO[0]:-unknown}); install newer bash for scripts using associative arrays"
fi

DOCKER_REACHABLE=0
if command -v docker >/dev/null 2>&1; then
  if docker info >/dev/null 2>&1; then
    pass "docker daemon is reachable"
    DOCKER_REACHABLE=1
  else
    fail "docker daemon is not reachable"
  fi
fi

if [[ -S /var/run/docker.sock ]]; then
  pass "host docker socket exists: /var/run/docker.sock"
else
  fail "host docker socket missing: /var/run/docker.sock"
fi

MEMBER_CONTAINER="${MEMBER_CONTAINER:-member}"
if [[ "$DOCKER_REACHABLE" -eq 1 ]]; then
  if docker ps --format '{{.Names}}' | grep -Fx "$MEMBER_CONTAINER" >/dev/null 2>&1; then
    if docker exec "$MEMBER_CONTAINER" sh -lc 'test -S /var/run/docker.sock'; then
      pass "container '$MEMBER_CONTAINER' has /var/run/docker.sock"
    else
      fail "container '$MEMBER_CONTAINER' is running but /var/run/docker.sock is unavailable"
    fi
  else
    warn "container '$MEMBER_CONTAINER' is not running; skipped container docker.sock check"
  fi
else
  warn "docker daemon unreachable; skipped container docker.sock check"
fi

if [[ "${RUN_LOOPBACK_CHECK:-0}" == "1" ]]; then
  if "$SCRIPT_DIR/check-macos-loopback.sh"; then
    pass "loopback check passed"
  else
    fail "loopback check failed"
  fi
else
  echo "INFO: skipping loopback check (set RUN_LOOPBACK_CHECK=1 to enable)"
fi

echo
echo "== preflight summary =="
echo "pass=$PASS_COUNT warn=$WARN_COUNT fail=$FAIL_COUNT"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
fi
