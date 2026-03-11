#!/bin/bash
# Shared helpers for E2E scenario scripts.
# Source this file: source "$(dirname "$0")/e2e-helpers.sh"

STALL_THRESHOLD=12
STALL_COUNT=0
PREV_SNAPSHOT=""

# Collect state snapshot and update stall counter.
# Sets: JOBS_JSON, STATE_JSON, STALL_COUNT
collect_snapshot() {
    JOBS_JSON=$(curl -s ${BASE_URL}/jobs 2>/dev/null || echo "[]")
    STATE_JSON=$(cat data/state.json 2>/dev/null || echo "{}")

    local job_snapshot member_snapshot pane_snapshot=""
    job_snapshot=$(echo "$JOBS_JSON" | jq -r '[.[] | "\(.id):\(.status):\(.assignee // "")"] | sort | join(",")' 2>/dev/null || echo "")
    member_snapshot=$(echo "$STATE_JSON" | jq -r '[(.supervisors + .members)[] | "\(.id):\(.status)"] | sort | join(",")' 2>/dev/null || echo "")

    for pane_target in $(echo "$STATE_JSON" | jq -r '(.supervisors + .members)[] | .terminal_target' 2>/dev/null); do
        local pane_hash
        pane_hash=$(tmux capture-pane -t "$pane_target" -p 2>/dev/null | grep -v '^$' | tail -5 | md5sum | cut -d' ' -f1)
        pane_snapshot="${pane_snapshot}|${pane_hash}"
    done

    local current="${job_snapshot}|${member_snapshot}|${pane_snapshot}"
    if [ "$current" = "$PREV_SNAPSHOT" ]; then
        STALL_COUNT=$((STALL_COUNT + 1))
    else
        STALL_COUNT=0
        PREV_SNAPSHOT="$current"
    fi
}

# Print monitoring status line.
print_status() {
    local elapsed=$1
    echo "--- ${elapsed}s (stall: ${STALL_COUNT}/${STALL_THRESHOLD}) ---"

    echo "  [jobs]"
    echo "$JOBS_JSON" | jq -r '.[] | "    \(.id) \(.status) \(.assignee // "")"' 2>/dev/null

    echo "  [agents]"
    echo "$STATE_JSON" | jq -r '(.supervisors + .members)[] | "    \(.id) role=\(.role) status=\(.status)"' 2>/dev/null
}

# Check if stall threshold reached. Returns 0 if stalled.
is_stalled() {
    [ "$STALL_COUNT" -ge "$STALL_THRESHOLD" ]
}

# Common cleanup for E2E scenarios.
e2e_cleanup() {
    local port=${1:-7100}
    echo "=== Cleanup ==="
    lsof -ti:${port} | xargs -r kill 2>/dev/null || true
    docker ps -q --filter label=palette.managed=true | xargs -r docker rm -f 2>/dev/null || true
    tmux kill-session -t palette 2>/dev/null || true
    rm -f data/state.json data/palette.db data/palette.db-shm data/palette.db-wal
    docker volume ls -q --filter name=palette- | xargs -r docker volume rm 2>/dev/null || true
    mkdir -p data
}
