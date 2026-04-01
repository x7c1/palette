#!/usr/bin/env bash
# Build all Docker images for Palette.
# Rebuilds base first, then leader and member (which inherit from base).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Building palette-base..."
docker build -f "$ROOT_DIR/docker/Dockerfile.base" -t palette-base:latest "$ROOT_DIR"

echo "Building palette-leader..."
docker build -f "$ROOT_DIR/docker/Dockerfile.leader" -t palette-leader:latest "$ROOT_DIR"

echo "Building palette-member..."
docker build -f "$ROOT_DIR/docker/Dockerfile.member" -t palette-member:latest "$ROOT_DIR"

echo "All images built."
