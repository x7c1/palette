# Development Guide

## Build & Check

Before committing, run:

```bash
cargo fmt
cargo clippy
cargo test
```

`.cargo/config.toml` sets `-D warnings` globally, so all warnings are treated as errors in build, test, and clippy.

## Configuration

Palette resolves its configuration file as follows:

- `--config <path>` — use the specified file
- No `--config` — use `~/.config/palette/config.toml`

The repository ships a development config at `config/palette.toml`, which uses a relative `data_dir = "data"`. This means commands run with `--config config/palette.toml` operate on the local `data/` directory within the repository.

Without `--config`, commands target the user config at `~/.config/palette/`, which may contain production data.

### Data Directory Layout

All runtime data — DB, plans, workspaces, artifacts — live under `data_dir`:

- `{data_dir}/palette.db` — SQLite database
- `{data_dir}/plans/` — plan storage
- `{data_dir}/workspace/` — per-job working copies
- `{data_dir}/repos/` — bare repo caches
- `{data_dir}/artifacts/` — craft job artifacts

### Operation Models

Palette's process uniqueness is enforced by SQLite's `PRAGMA locking_mode=EXCLUSIVE` on the DB file, so the choice of `data_dir` determines how instances are shared:

- **Model A — project-local instance**: a relative `data_dir` (e.g., `data_dir = "data"`) resolves from the current working directory. Each CWD has its own DB and workspaces, and multiple instances can run concurrently. This is the default for development.
- **Model B — user-global instance**: an absolute `data_dir` (e.g., `data_dir = "/Users/alice/.local/share/palette"`) pins the instance to a single location. Starting `palette` from any CWD shares the same DB and workspaces, so only one instance exists per user.

Switch between models by editing `data_dir` in the config file.

## Running Commands Locally

Start the Orchestrator with the development config:

```bash
cargo run --bin palette -- start --config config/palette.toml
```

Admin commands also require an explicit config to avoid touching production data:

```bash
cargo run --bin palette -- admin gc --config config/palette.toml --dry-run
cargo run --bin palette -- admin reset --config config/palette.toml --dry-run
```

## Testing with the Installed Instance

The installed instance at `~/.config/palette/repo` can be switched to a development branch for testing operator skills and CLI changes end-to-end.

Switch to a branch:

```bash
scripts/switch-branch.sh <branch>
```

This fetches from origin, switches the branch, and runs `cargo build --release` in `~/.config/palette/repo`.

Restore to main after testing:

```bash
scripts/restore-main.sh
```

> **Note:** The branch must be pushed to origin before switching. Develop and push in your local clone, then use these scripts to deploy to the installed instance.

## Architecture

See [palette-design.md](palette-design.md) for the crate dependency graph, layer responsibilities, and design principles.
