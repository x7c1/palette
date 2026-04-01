#!/bin/bash
# PreToolUse hook for Bash: block compound cd commands.
# Chaining `cd` with other commands triggers Claude Code's
# "bare repository attack" security check, which requires manual approval
# that cannot be auto-allowed via settings.json.
set -euo pipefail

command=$(jq -r '.tool_input.command // empty')
[[ -z "$command" ]] && exit 0

stripped=$(echo "$command" | sed "s/'[^']*'//g; s/\"[^\"]*\"//g")

pattern='(^|[^[:alnum:]])cd[[:space:]]+[^[:space:]].*[;&|]+.+'
if [[ $stripped =~ $pattern ]]; then
    cat >&2 << 'EOF'
BLOCKED: Chaining commands after `cd` is not allowed.

Run `cd` as a separate command first, then run the next command separately.
EOF
    exit 2
fi
