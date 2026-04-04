#!/usr/bin/env bash
# Migrate legacy ~/.gitconfig to XDG path ~/.config/git/config.
set -euo pipefail

SOURCE="${SOURCE:-$HOME/.gitconfig}"
TARGET_DIR="${TARGET_DIR:-$HOME/.config/git}"
TARGET="${TARGET:-$TARGET_DIR/config}"
BACKUP_DIR="${BACKUP_DIR:-$TARGET_DIR/backups}"
OVERWRITE="${OVERWRITE:-0}"

pass() {
  echo "PASS: $*"
}

fail() {
  echo "FAIL: $*"
  exit 1
}

warn() {
  echo "WARN: $*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "required command not found: $1"
  fi
}

timestamp() {
  date +"%Y%m%d-%H%M%S"
}

echo "== migrate gitconfig to XDG =="
echo "source=$SOURCE"
echo "target=$TARGET"
echo "overwrite=$OVERWRITE"

require_cmd git

if [[ ! -f "$SOURCE" ]]; then
  fail "source file not found: $SOURCE"
fi

mkdir -p "$TARGET_DIR"
mkdir -p "$BACKUP_DIR"

if [[ -f "$TARGET" ]]; then
  if cmp -s "$SOURCE" "$TARGET"; then
    pass "target already matches source; no changes needed"
  else
    if [[ "$OVERWRITE" != "1" ]]; then
      fail "target exists and differs: $TARGET (set OVERWRITE=1 to replace)"
    fi
    backup_path="$BACKUP_DIR/config.$(timestamp).bak"
    cp -p "$TARGET" "$backup_path"
    pass "backed up existing target to $backup_path"
    cp -p "$SOURCE" "$TARGET"
    pass "replaced target with source"
  fi
else
  cp -p "$SOURCE" "$TARGET"
  pass "copied source to target"
fi

if git config -f "$TARGET" user.name >/dev/null 2>&1; then
  pass "user.name present in $TARGET"
else
  warn "user.name missing in $TARGET"
fi

if git config -f "$TARGET" user.email >/dev/null 2>&1; then
  pass "user.email present in $TARGET"
else
  warn "user.email missing in $TARGET"
fi

echo
echo "Next:"
echo "  tests/e2e/check-worker-credentials.sh"
