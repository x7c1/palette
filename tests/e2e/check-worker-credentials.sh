#!/usr/bin/env bash
# Diagnose host credential prerequisites for worker git operations.
set -euo pipefail

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
  if command -v "$1" >/dev/null 2>&1; then
    pass "command available: $1"
  else
    fail "required command missing: $1"
  fi
}

echo "== Worker credential diagnostics =="
echo "HOME=$HOME"
REPO_DIR="${REPO_DIR:-$PWD}"
echo "REPO_DIR=$REPO_DIR"
echo

for cmd in git docker; do
  require_cmd "$cmd"
done

if [[ -f "$HOME/.config/git/config" ]]; then
  pass "git config found at ~/.config/git/config"
else
  if [[ -f "$HOME/.gitconfig" ]]; then
    fail "~/.config/git/config missing (legacy ~/.gitconfig detected)"
  else
    fail "~/.config/git/config missing"
  fi
fi

if git config -f "$HOME/.config/git/config" user.name >/dev/null 2>&1; then
  pass "git user.name is configured"
else
  fail "git user.name is missing in ~/.config/git/config"
fi

if git config -f "$HOME/.config/git/config" user.email >/dev/null 2>&1; then
  pass "git user.email is configured"
else
  fail "git user.email is missing in ~/.config/git/config"
fi

if [[ -d "$HOME/.config/gh" ]]; then
  pass "GitHub CLI config directory exists"
else
  warn "GitHub CLI config directory (~/.config/gh) not found"
fi

if [[ -d "$HOME/.ssh" ]]; then
  pass "SSH directory exists"
else
  warn "SSH directory (~/.ssh) not found"
fi

if docker info >/dev/null 2>&1; then
  pass "docker daemon is reachable"
else
  fail "docker daemon is not reachable"
fi

echo
echo "== Remote diagnosis =="

if git -C "$REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  pass "repository detected: $REPO_DIR"
else
  fail "REPO_DIR is not a git repository: $REPO_DIR"
fi

remote_kind="none"
if git -C "$REPO_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  remote_lines="$(git -C "$REPO_DIR" remote -v | awk '$3=="(fetch)" {print $1 " " $2}')"
  if [[ -z "${remote_lines:-}" ]]; then
    warn "no git remotes configured"
  else
    has_ssh=0
    has_https=0
    has_unknown=0
    while IFS= read -r line; do
      [[ -z "$line" ]] && continue
      remote_name="${line%% *}"
      remote_url="${line#* }"
      if [[ "$remote_url" =~ ^git@ ]] || [[ "$remote_url" =~ ^ssh:// ]]; then
        kind="ssh"
        has_ssh=1
      elif [[ "$remote_url" =~ ^https?:// ]]; then
        kind="https"
        has_https=1
      else
        kind="unknown"
        has_unknown=1
      fi
      echo "remote[$remote_name]=$remote_url kind=$kind"
    done <<< "$remote_lines"

    if [[ "$has_ssh" -eq 1 && "$has_https" -eq 1 ]]; then
      remote_kind="mixed"
    elif [[ "$has_ssh" -eq 1 ]]; then
      remote_kind="ssh"
    elif [[ "$has_https" -eq 1 ]]; then
      remote_kind="https"
    elif [[ "$has_unknown" -eq 1 ]]; then
      remote_kind="unknown"
    fi
  fi
fi

echo "remote_kind=$remote_kind"

git_credential_helper="$(git config -f "$HOME/.config/git/config" credential.helper 2>/dev/null || true)"

if [[ "$remote_kind" == "ssh" || "$remote_kind" == "mixed" ]]; then
  if [[ -d "$HOME/.ssh" ]]; then
    pass "ssh remote detected and ~/.ssh is available"
  else
    fail "ssh remote detected but ~/.ssh is missing"
  fi
fi

if [[ "$remote_kind" == "https" || "$remote_kind" == "mixed" ]]; then
  if [[ -n "$git_credential_helper" ]]; then
    pass "https remote detected and git credential.helper is configured ($git_credential_helper)"
  elif [[ -d "$HOME/.config/gh" ]]; then
    pass "https remote detected and ~/.config/gh exists"
  else
    fail "https remote detected but no credential.helper and no ~/.config/gh"
  fi
fi

if [[ "$remote_kind" == "unknown" ]]; then
  warn "remote kind is unknown; credential checks for transport were skipped"
fi

echo
echo "== summary =="
echo "pass=$PASS_COUNT warn=$WARN_COUNT fail=$FAIL_COUNT"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
  exit 1
fi
