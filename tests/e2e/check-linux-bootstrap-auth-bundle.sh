#!/usr/bin/env bash
# Verify Linux bootstrap auth bundle can be discovered and propagated.
#
# Expected flow:
#  1. Run bootstrap container and complete `claude login` manually once.
#  2. Re-run this script to collect auth artifacts and verify mount propagation.
set -euo pipefail

SOURCE_CONTAINER="${SOURCE_CONTAINER:-}"
TARGET_IMAGE="${TARGET_IMAGE:-}"
REQUIRE_AUTH="${REQUIRE_AUTH:-1}"
TMP_DIR="$(mktemp -d)"
BUNDLE_DIR="$TMP_DIR/bundle"
mkdir -p "$BUNDLE_DIR"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

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

  # Prefer compose-managed bootstrap container when present.
  if docker compose ps -q claude-code >/dev/null 2>&1; then
    local cid
    cid="$(docker compose ps -q claude-code || true)"
    if [[ -n "$cid" ]]; then
      SOURCE_CONTAINER="$cid"
      return
    fi
  fi

  # Fallback by name match for manually started containers.
  local by_name
  by_name="$(docker ps -a --format '{{.Names}}' | grep -E '^claude-code$|claude-code' | head -n 1 || true)"
  if [[ -n "$by_name" ]]; then
    SOURCE_CONTAINER="$by_name"
  fi
}

container_has_path() {
  local path="$1"
  docker exec "$SOURCE_CONTAINER" sh -lc "test -e '$path'" >/dev/null 2>&1
}

copy_if_exists() {
  local src="$1"
  local dst="$2"
  if container_has_path "$src"; then
    mkdir -p "$(dirname "$dst")"
    docker cp "$SOURCE_CONTAINER:$src" "$dst"
    echo "FOUND: $src"
    return 0
  fi
  echo "MISS:  $src"
  return 1
}

require_cmd docker

detect_source_container
if [[ -z "$SOURCE_CONTAINER" ]]; then
  echo "FAIL: bootstrap source container not found."
  echo "HINT: run 'docker compose up -d claude-code' and complete 'claude login' once."
  exit 1
fi

if [[ -z "$TARGET_IMAGE" ]]; then
  TARGET_IMAGE="$(docker inspect -f '{{.Config.Image}}' "$SOURCE_CONTAINER" 2>/dev/null || true)"
fi

if [[ -z "$TARGET_IMAGE" ]]; then
  echo "FAIL: could not determine TARGET_IMAGE from source container '$SOURCE_CONTAINER'."
  echo "HINT: set TARGET_IMAGE explicitly."
  exit 1
fi

echo "== Bootstrap source =="
echo "container=$SOURCE_CONTAINER"
echo "target_image=$TARGET_IMAGE"
echo

echo "== Discover auth artifacts in bootstrap container =="
auth_count=0

if copy_if_exists "/home/developer/.claude/.credentials.json" "$BUNDLE_DIR/.claude/.credentials.json"; then
  auth_count=$((auth_count + 1))
fi
if copy_if_exists "/home/developer/.claude/settings.json" "$BUNDLE_DIR/.claude/settings.json"; then
  auth_count=$((auth_count + 1))
fi
if copy_if_exists "/home/developer/.claude/CLAUDE.md" "$BUNDLE_DIR/.claude/CLAUDE.md"; then
  :
fi
if copy_if_exists "/home/developer/.claude.json" "$BUNDLE_DIR/.claude.json"; then
  :
fi

echo
echo "== Bundle summary =="
echo "bundle_dir=$BUNDLE_DIR"
echo "auth_markers=$auth_count"
find "$BUNDLE_DIR" -type f | sed "s#^$BUNDLE_DIR#  .#"

if [[ "$REQUIRE_AUTH" == "1" && "$auth_count" -eq 0 ]]; then
  echo
  echo "FAIL: no auth markers found in bootstrap container."
  echo "HINT: enter the container and complete 'claude login' first."
  exit 1
fi

echo
echo "== Propagation check =="
docker run --rm \
  -v "$BUNDLE_DIR:/tmp/bootstrap-bundle:ro" \
  "$TARGET_IMAGE" \
  sh -lc '
    set -eu
    test -d /tmp/bootstrap-bundle
    ls -la /tmp/bootstrap-bundle >/dev/null
    if [ -f /tmp/bootstrap-bundle/.claude/.credentials.json ]; then
      echo "PASS: credentials file mount visible in target container"
    else
      echo "INFO: credentials file not present in exported bundle"
    fi
  '

echo "PASS: bootstrap bundle export and mount propagation succeeded"
