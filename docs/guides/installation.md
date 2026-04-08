# Installation Guide

## Prerequisites

| Tool | Purpose | Install |
|---|---|---|
| git | Repository clone and updates | [git-scm.com](https://git-scm.com/) |
| Rust (cargo) | Build the Palette binary | [rustup.rs](https://rustup.rs/) |
| Docker | Run Worker containers | [docs.docker.com](https://docs.docker.com/get-docker/) |
| tmux | Manage Worker terminal sessions | [github.com/tmux/tmux](https://github.com/tmux/tmux) |
| GitHub CLI (gh) | GitHub API access | [cli.github.com](https://cli.github.com/) |

Ensure Docker daemon is running and `gh auth login` has been completed.

## Plugin Installation

Operator skills (`/palette:setup`, `/palette:doctor` など) を Claude Code で使うには、プラグインをインストールする:

```bash
# marketplace を追加
/plugin marketplace add x7c1/palette

# プラグインをインストール
claude plugin install palette@palette
```

## Quick Setup (via Claude Code)

プラグインのインストール後、以下を実行するだけでセットアップが完了する:

```
/palette:setup
```

これにより以下の Manual Installation の手順がすべて自動で実行される。

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
