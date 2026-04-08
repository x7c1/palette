#!/usr/bin/env bash
# Fail-fast check for docker images required by palette E2E workflows.
set -euo pipefail

required_images=(
  "palette-supervisor:latest"
  "palette-member:latest"
)

if ! command -v docker >/dev/null 2>&1; then
  echo "FAIL: required command missing: docker"
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "FAIL: docker daemon is not reachable"
  exit 1
fi

missing=0
for image in "${required_images[@]}"; do
  if docker image inspect "$image" >/dev/null 2>&1; then
    echo "PASS: docker image available: $image"
  else
    echo "FAIL: required docker image missing: $image (run ./scripts/build-images.sh)"
    missing=1
    continue
  fi

  if docker run --rm --entrypoint sh "$image" -lc "command -v claude >/dev/null 2>&1"; then
    echo "PASS: claude executable available in image: $image"
  else
    echo "FAIL: claude executable missing in image: $image (run ./scripts/build-images.sh)"
    missing=1
  fi
done

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi
