#!/bin/bash

# Tests for guard-cd-chain.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HOOK="$SCRIPT_DIR/guard-cd-chain.sh"

pass=0
fail=0

run_hook() {
    local command=$1
    jq -n --arg cmd "$command" '{tool_input:{command:$cmd}}' | "$HOOK" >/dev/null 2>&1
}

expect_blocked() {
    local label=$1 command=$2
    if run_hook "$command"; then
        echo "FAIL (expected block): $label"
        ((fail += 1))
    else
        echo "pass: $label"
        ((pass += 1))
    fi
}

expect_allowed() {
    local label=$1 command=$2
    if run_hook "$command"; then
        echo "pass: $label"
        ((pass += 1))
    else
        echo "FAIL (expected allow): $label"
        ((fail += 1))
    fi
}

# --- Should block ---
expect_blocked "cd && command" "cd /path && git status"
expect_blocked "cd && non-git" "cd /path && npm test"
expect_blocked "cd ; command" "cd /path ; make build"
expect_blocked "cd || command" "cd /path || echo fail"
expect_blocked "cd && chain" "cd /repo && git add . && git commit -m 'msg'"
expect_blocked "cd workspace && git diff" "cd /home/agent/workspace && git diff"

# --- Should allow ---
expect_allowed "cd alone" "cd /some/path"
expect_allowed "normal git command" "git status"
expect_allowed "normal npm command" "npm test"
expect_allowed "cd in quoted string" "echo 'cd /path && git status'"
expect_allowed "cd in double-quoted string" "echo \"cd /path && git status\""
expect_allowed "empty command" ""
expect_allowed "git add && git commit" "git add . && git commit -m 'msg'"

echo ""
echo "Results: $pass passed, $fail failed"
[[ $fail -eq 0 ]]
