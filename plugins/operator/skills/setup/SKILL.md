---
name: setup
description: Install and set up Palette on the local machine. Clones the repository, builds the binary, and builds Docker images.
user_invocable: true
---

# /palette:setup

Install Palette on this machine. Run each step below in order. If any step fails, stop and show the error to the Operator.

## Step 1: Check Prerequisites

Run `~/.config/palette/repo/target/release/palette doctor` if the binary already exists. Otherwise, check manually:

```bash
git --version
cargo --version
docker info
tmux -V
gh auth status
```

If any command fails, tell the Operator which tool is missing and how to install it, then stop.

## Step 2: Clone or Update Repository

```bash
if [ -d ~/.config/palette/repo ]; then
  cd ~/.config/palette/repo && git pull
else
  mkdir -p ~/.config/palette
  git clone https://github.com/x7c1/palette.git ~/.config/palette/repo
fi
```

## Step 3: Build Binary

```bash
cd ~/.config/palette/repo && cargo build --release
```

This produces `~/.config/palette/repo/target/release/palette`.

## Step 4: Build Docker Images

```bash
cd ~/.config/palette/repo && scripts/build-images.sh
```

## Step 5: Verify

```bash
~/.config/palette/repo/target/release/palette doctor
```

Show the results to the Operator. If all checks pass, report success.
