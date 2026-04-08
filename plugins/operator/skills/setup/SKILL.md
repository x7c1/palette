---
name: setup
description: Install and set up Palette on the local machine. Clones the repository, builds the binary, and builds Docker images.
user-invocable: true
---

# /palette:setup

Install Palette on this machine. Run each step below in order. If any step fails, stop and show the error to the Operator.

## Step 1: Check Prerequisites

Run `~/.config/palette/repo/target/release/palette doctor` if the binary already exists. Otherwise, check manually:

```bash
git --version
cargo --version
docker version
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

## Step 3: Sync User Config

The user config lives at `~/.config/palette/config.toml`. The bundled default is at `~/.config/palette/repo/config/palette.toml`.

- If the user config does not exist, copy the bundled default to create it
- If the user config already exists, compare it with the bundled default. If the bundled default contains new fields or sections that are missing from the user config, add them to the user config with their default values. Do not overwrite fields the Operator has already customized

## Step 4: Build Binary

```bash
cd ~/.config/palette/repo && cargo build --release
```

This produces `~/.config/palette/repo/target/release/palette`.

## Step 5: Build Docker Images

```bash
cd ~/.config/palette/repo && scripts/build-images.sh
```

## Step 6: Verify

```bash
~/.config/palette/repo/target/release/palette doctor
```

Show the results to the Operator. If all checks pass, report success.
