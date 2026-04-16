#!/bin/bash
set -euo pipefail

timeout="${1:-30}"
for i in $(seq 1 "$timeout"); do
  if curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:7100/health 2>/dev/null | grep -q 200; then
    echo "Orchestrator started successfully."
    exit 0
  fi
  sleep 1
done
echo "Timed out waiting for Orchestrator to start."
exit 1
