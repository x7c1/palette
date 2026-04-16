#!/bin/bash
set -euo pipefail

cd ~/.config/palette/repo

git switch main
git pull origin main
cargo build --release

echo "Restored to main and built successfully."
