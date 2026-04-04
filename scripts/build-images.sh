#!/usr/bin/env bash
# Build all Docker images for Palette.
# Rebuilds base first, then leader and member (which inherit from base).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CERT_SOURCE_DIR="${HOME}/.config/palette/certs"
CERT_STAGE_DIR="$ROOT_DIR/.palette-build-certs.local"

cleanup() {
  rm -rf "$CERT_STAGE_DIR"
}
trap cleanup EXIT

mkdir -p "$CERT_STAGE_DIR"
if [[ -d "$CERT_SOURCE_DIR" ]]; then
  shopt -s nullglob
  cert_files=("$CERT_SOURCE_DIR"/*.crt "$CERT_SOURCE_DIR"/*.pem)
  shopt -u nullglob
  for cert in "${cert_files[@]}"; do
    cp -f "$cert" "$CERT_STAGE_DIR/"
  done
fi

echo "Building palette-base..."
docker build -f "$ROOT_DIR/docker/Dockerfile.base" -t palette-base:latest "$ROOT_DIR"

echo "Building palette-leader..."
docker build -f "$ROOT_DIR/docker/Dockerfile.leader" -t palette-leader:latest "$ROOT_DIR"

echo "Building palette-member..."
docker build -f "$ROOT_DIR/docker/Dockerfile.member" -t palette-member:latest "$ROOT_DIR"

echo "All images built."
