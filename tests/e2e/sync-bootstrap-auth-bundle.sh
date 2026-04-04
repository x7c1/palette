#!/usr/bin/env bash
# Sync Claude auth artifacts from bootstrap container to local auth bundle.
set -euo pipefail

SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
OUTPUT_DIR="${PALETTE_CLAUDE_AUTH_BUNDLE_DIR:-$HOME/.config/palette/claude-auth-bundle}"
TMP_DIR="$(mktemp -d)"
STAGING_DIR="$TMP_DIR/bundle"
mkdir -p "$STAGING_DIR/.claude"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

detect_source_container() {
  if [[ -n "$SOURCE_CONTAINER" ]]; then
    return
  fi

  if docker compose ps -q claude-code >/dev/null 2>&1; then
    local cid
    cid="$(docker compose ps -q claude-code || true)"
    if [[ -n "$cid" ]]; then
      SOURCE_CONTAINER="$cid"
      return
    fi
  fi

  SOURCE_CONTAINER="$(docker ps --format '{{.Names}}' | grep -E '^palette-claude-code-1$|^claude-code$|claude-code' | head -n 1 || true)"
}

copy_if_exists() {
  local src="$1"
  local dst="$2"
  if docker exec "$SOURCE_CONTAINER" sh -lc "test -f '$src'" >/dev/null 2>&1; then
    mkdir -p "$(dirname "$dst")"
    docker cp "$SOURCE_CONTAINER:$src" "$dst"
    return 0
  fi
  return 1
}

detect_source_container
if [[ -z "$SOURCE_CONTAINER" ]]; then
  echo "FAIL: bootstrap container not found for auth sync"
  echo "HINT: run 'docker compose up -d claude-code' and complete 'claude login'"
  exit 1
fi

if ! copy_if_exists "/home/developer/.claude/.credentials.json" "$STAGING_DIR/.claude/.credentials.json"; then
  echo "FAIL: bootstrap container has no /home/developer/.claude/.credentials.json"
  echo "HINT: enter bootstrap container and run 'claude login' first"
  exit 1
fi

mkdir -p "$OUTPUT_DIR/.claude"
cp -f "$STAGING_DIR/.claude/.credentials.json" "$OUTPUT_DIR/.claude/.credentials.json"

echo "PASS: synced auth bundle from $SOURCE_CONTAINER -> $OUTPUT_DIR"
