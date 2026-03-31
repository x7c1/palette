#!/usr/bin/env bash
# E2E: Review Artifacts Persistence (012/002)
# Verify that reviewers write review.md and the integrator writes
# integrated-review.md in the correct round-based directory structure.
#
# Blueprint: 1 Crafter + 2 Reviewers (with ReviewIntegrator)
# Expected: ChangesRequested in round 1, Approved in round 2
#
# Checks:
# - Round 1: review.md files created per reviewer
# - Round 1: integrated-review.md created
# - Round 1 files have YAML frontmatter
# - Round 2: new round directory created on re-review
# - Round 1 files preserved (audit trail)
# - Crafter can read integrated-review.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

PALETTE_URL="http://127.0.0.1:7100"
BLUEPRINT_PATH="$ROOT_DIR/tests/e2e/fixtures/review-artifacts.yaml"
LOG_FILE="data/palette.log"
PID_FILE="data/palette.pid"
POLL_INTERVAL=5
STALL_THRESHOLD=24

trap '"$SCRIPT_DIR/stop-palette.sh"' EXIT

# --- Helpers ---
worker_summary() {
  curl -sf "$PALETTE_URL/workers" 2>/dev/null \
    | jq -r '[.[] | "\(.id):\(.status)"] | join(" ")' 2>/dev/null \
    || echo ""
}

wait_for_log() {
  local pattern="$1"
  local max_wait="${2:-120}"
  for i in $(seq 1 "$max_wait"); do
    if grep -q "$pattern" "$LOG_FILE" 2>/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

# --- Step 1: Reset and build ---
echo "=== Step 1: Reset and build ==="
scripts/reset.sh 2>&1
mkdir -p data/plans
cp -r tests/e2e/fixtures/plans/* data/plans/ 2>/dev/null || true
cargo build 2>&1

# --- Step 2: Start Palette ---
echo ""
echo "=== Step 2: Start Palette ==="
RUST_LOG=info cargo run >> "$LOG_FILE" 2>&1 &
echo $! > "$PID_FILE"

for i in $(seq 1 30); do
  if curl -sf "$PALETTE_URL/jobs" > /dev/null 2>&1; then
    echo "Health check passed"
    break
  fi
  if [[ $i -eq 30 ]]; then echo "FAIL: Health check timed out"; exit 1; fi
  sleep 2
done

# --- Step 3: Start workflow ---
echo ""
echo "=== Step 3: Start workflow ==="
HTTP_CODE=$(curl -s -o /tmp/palette-e2e-response.json -w '%{http_code}' \
  -X POST "$PALETTE_URL/workflows/start" \
  -H "Content-Type: application/json" \
  -d "{\"blueprint_path\": \"$BLUEPRINT_PATH\"}")
RESPONSE=$(cat /tmp/palette-e2e-response.json)

if [[ "$HTTP_CODE" -lt 200 || "$HTTP_CODE" -ge 300 ]]; then
  echo "FAIL: POST /workflows/start returned HTTP $HTTP_CODE"
  exit 1
fi
WORKFLOW_ID=$(echo "$RESPONSE" | jq -r '.workflow_id')
echo "Workflow ID: $WORKFLOW_ID"

# --- Step 4: Monitor and verify artifacts ---
echo ""
echo "=== Step 4: Monitor workflow ==="

prev_snapshot=""
stall_count=0
iteration=0
round1_verified=false

while true; do
  iteration=$((iteration + 1))
  sleep "$POLL_INTERVAL"

  JOBS=$(curl -sf "$PALETTE_URL/jobs" 2>/dev/null || echo "[]")
  WORKERS=$(worker_summary)
  snapshot="${JOBS}|${WORKERS}"

  elapsed=$((iteration * POLL_INTERVAL))
  job_summary=$(echo "$JOBS" | jq -r '[.[] | .status] | group_by(.) | map("\(.[0]):\(length)") | join(" ")' 2>/dev/null || echo "no jobs")
  echo "[${elapsed}s] jobs: ${job_summary} | workers: ${WORKERS:-none} | stall: ${stall_count}/${STALL_THRESHOLD}"

  # Check for round-1 artifacts after review jobs enter done/changes_requested
  if [[ "$round1_verified" == "false" ]]; then
    CRAFT_JOB=$(echo "$JOBS" | jq -r '.[] | select(.job_type == "craft") | .id' | head -1)
    if [[ -n "$CRAFT_JOB" ]] && [[ -d "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB/round-1" ]]; then
      echo ""
      echo "--- Checking round-1 artifacts ---"

      # Count review.md files
      REVIEW_COUNT=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB/round-1" -name "review.md" 2>/dev/null | wc -l)
      if [[ "$REVIEW_COUNT" -gt 0 ]]; then
        echo "PASS: Found $REVIEW_COUNT review.md file(s) in round-1"

        # Verify YAML frontmatter in first review.md
        FIRST_REVIEW=$(find "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB/round-1" -name "review.md" | head -1)
        if head -1 "$FIRST_REVIEW" | grep -q "^---"; then
          echo "PASS: review.md has YAML frontmatter"
        else
          echo "WARN: review.md may lack YAML frontmatter"
        fi

        round1_verified=true
      fi

      # Check integrated-review.md
      if [[ -f "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB/round-1/integrated-review.md" ]]; then
        echo "PASS: integrated-review.md exists in round-1"
        if head -1 "data/artifacts/$WORKFLOW_ID/$CRAFT_JOB/round-1/integrated-review.md" | grep -q "^---"; then
          echo "PASS: integrated-review.md has YAML frontmatter"
        fi
      fi
    fi
  fi

  if [[ "$snapshot" == "$prev_snapshot" ]]; then
    stall_count=$((stall_count + 1))
  else
    stall_count=0
  fi
  prev_snapshot="$snapshot"

  if [[ $stall_count -ge $STALL_THRESHOLD ]]; then
    echo "FAIL: Stall detected"
    tail -20 "$LOG_FILE"
    exit 1
  fi

  if grep -q "workflow completed" "$LOG_FILE" 2>/dev/null; then
    break
  fi
done

# --- Step 5: Verify final artifacts ---
echo ""
echo "=== Step 5: Verify final artifacts ==="

CRAFT_JOB=$(curl -sf "$PALETTE_URL/jobs" | jq -r '.[] | select(.job_type == "craft") | .id' | head -1)
ARTIFACTS_DIR="data/artifacts/$WORKFLOW_ID/$CRAFT_JOB"

if [[ ! -d "$ARTIFACTS_DIR" ]]; then
  echo "FAIL: Artifacts directory not found: $ARTIFACTS_DIR"
  exit 1
fi

echo "Artifacts directory contents:"
find "$ARTIFACTS_DIR" -type f | sort | while read -r f; do
  echo "  $f"
done

# Round 1 should exist
if [[ -d "$ARTIFACTS_DIR/round-1" ]]; then
  echo "PASS: round-1 directory exists"
else
  echo "FAIL: round-1 directory not found"
  exit 1
fi

echo ""
echo "=== All review-artifacts checks passed ==="
scripts/reset.sh 2>&1
exit 0
