#!/bin/bash
set -euo pipefail

branch="${1:?Usage: switch-branch.sh <branch>}"
cd ~/.config/palette/repo

git fetch origin
git switch "$branch"
git pull origin "$branch"
cargo build --release

echo "Switched to $branch and built successfully."
