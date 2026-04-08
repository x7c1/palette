# Installation Guide

## Prerequisites

| Tool | Purpose | Install |
|---|---|---|
| git | Repository clone and updates | [git-scm.com](https://git-scm.com/) |
| Rust (cargo) | Build the Palette binary | [rustup.rs](https://rustup.rs/) |
| Docker | Run Worker containers | [docs.docker.com](https://docs.docker.com/get-docker/) |
| tmux | Manage Worker terminal sessions | OS のパッケージマネージャでインストール |
| GitHub CLI (gh) | GitHub API access | [cli.github.com](https://cli.github.com/) |

Ensure Docker daemon is running and `gh auth login` has been completed.

## Quick Setup (via Claude Code)

If the Palette plugin is installed, run:

```
/palette:setup
```

This performs all steps below automatically.

## Manual Installation

### 1. Clone the Repository

```bash
mkdir -p ~/.config/palette
git clone https://github.com/x7c1/palette.git ~/.config/palette/repo
```

### 2. Build the Binary

```bash
cd ~/.config/palette/repo
cargo build --release
```

The binary is built at `~/.config/palette/repo/target/release/palette`. PATH configuration is not required — all skills reference the binary by its full path.

### 3. Build Docker Images

```bash
cd ~/.config/palette/repo
scripts/build-images.sh
```

### 4. Verify

```bash
~/.config/palette/repo/target/release/palette doctor
```

All checks should pass.

## Updating

```bash
cd ~/.config/palette/repo
git pull
cargo build --release
```

Rebuild Docker images if the Dockerfile has changed:

```bash
scripts/build-images.sh
```

## Plugin Installation

To install the Operator skills as a Claude Code plugin:

```bash
# Add the palette marketplace
/plugin marketplace add x7c1/palette

# Install the plugin
claude plugin install palette@palette
```

This makes `/palette:setup`, `/palette:doctor`, and other Operator skills available.
